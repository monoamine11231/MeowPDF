use core::f32;
use std::{
    sync::{atomic::Ordering, Arc, RwLock},
    thread::{self, JoinHandle},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use mupdf::{Colorspace, Document, Error, Matrix, Page, Pixmap};

use crate::{
    config::Config,
    drivers::priority_channel::{unbounded_priority, PriorityReceiver, PrioritySender},
    globals::{CONFIG, RUNNING},
    image::Image,
};

#[derive(Copy, Clone, PartialEq)]
pub enum RendererAction {
    Load,
    Display(usize),
    ToggleInverse,
    ToggleAlpha,
}

#[derive(Clone)]
pub enum RendererResult {
    PageMetadata {
        max_page_width: f32,
        cumulative_heights: Vec<f32>,
    },
    Image {
        page: usize,
        data: Option<Arc<RwLock<Image>>>,
    },
}

struct RendererInnerState<'a> {
    pub config: &'a Config,

    pub file: String,
    pub document: Document,

    pub cache: Vec<Page>,

    pub alpha: bool,
    pub inverse: bool,

    pub cs: Colorspace,
    pub ctm: Matrix,
}

impl<'a> RendererInnerState<'a> {
    pub fn new(file: String) -> Result<Self, String> {
        let document = Document::open(&file)
            .map_err(|x| format!("Could not open the given PDF file: {}", x))?;

        let config = CONFIG.get().unwrap();
        let inner_state: RendererInnerState<'a> = Self {
            config,
            file,
            document,
            cache: Vec::new(),
            alpha: false,
            inverse: false,
            cs: Colorspace::device_rgb(),
            ctm: Matrix::new_scale(
                config.viewer.render_precision as f32,
                config.viewer.render_precision as f32,
            ),
        };

        Ok(inner_state)
    }

    pub fn load(&mut self) -> Result<RendererResult, String> {
        let mut max_page_width = -f32::INFINITY;
        let mut cumulative_heights = Vec::new();

        self.document = Document::open(&self.file)
            .map_err(|x| format!("Could not open the given PDF file: {}", x))?;

        if !self.document.is_pdf() {
            Err("The given PDF file is not a PDF!".to_string())?;
        }
        self.cache.clear();

        let page_count: i32 = self
            .document
            .page_count()
            .map_err(|x| format!("Could not extract the number of pages: {}", x))?;

        for i in 0..page_count {
            let page: Page = self
                .document
                .load_page(i)
                .map_err(|x| format!("Could not load page {}: {}", i, x))?;

            let bounds = page
                .bounds()
                .map_err(|x| format!("Could not get bounds for page {}: {}", i, x))?;

            let width: f32 = bounds.width();
            let height: f32 = bounds.height();

            self.cache.push(page);
            max_page_width = f32::max(max_page_width, width);
            cumulative_heights.push(
                cumulative_heights.last().unwrap_or(&0.0f32)
                    + height
                    + self.config.viewer.margin_bottom,
            );
        }

        Ok(RendererResult::PageMetadata {
            max_page_width,
            cumulative_heights,
        })
    }
}

pub struct Renderer {
    thread_render: Option<JoinHandle<Result<(), String>>>,

    priority_client_sender: PrioritySender<RendererAction, 2>,
    priority_server_receiver: PriorityReceiver<RendererAction, 2>,

    general_client_receiver: Receiver<RendererAction>,
    general_server_sender: Sender<RendererAction>,

    result_server_sender: PrioritySender<RendererResult, 2>,
}

impl Renderer {
    pub fn new() -> (Self, PriorityReceiver<RendererResult, 2>) {
        let (priority_client_sender, priority_server_receiver) =
            unbounded_priority::<RendererAction, 2>();

        let (general_server_sender, general_client_receiver) =
            unbounded::<RendererAction>();

        let (result_server_sender, result_client_receiver) =
            unbounded_priority::<RendererResult, 2>();

        (
            Self {
                thread_render: None,

                priority_client_sender,
                priority_server_receiver,

                general_client_receiver,
                general_server_sender,

                result_server_sender,
            },
            result_client_receiver,
        )
    }

    pub fn run(&mut self, file_input: &str) -> Result<(), String> {
        let file_string = file_input.to_owned();
        let priority_server_receiver = self.priority_server_receiver.clone();
        let general_server_sender = self.general_server_sender.clone();
        let result_server_sender = self.result_server_sender.clone();

        let thread_render: JoinHandle<Result<(), String>> =
            thread::spawn(move || {
                let priority_server_receiver = priority_server_receiver;
                let general_server_sender = general_server_sender;
                let result_server_sender = result_server_sender;

                let mut state = RendererInnerState::new(file_string)?;

                let mut sel = priority_server_receiver.construct_biased_select();

                while RUNNING.load(Ordering::Acquire) {
                    let action = priority_server_receiver
                        .recv_priority(sel.ready())
                        .map_err(|x| format!("Could not receive from client: {}", x))?;

                    match action {
                        RendererAction::Display(_) => (),
                        _ => {
                            general_server_sender.try_send(action).map_err(|x| {
                                format!("Could not send action to client: {}", x)
                            })?;
                        }
                    }

                    match action {
                        RendererAction::Load => {
                            priority_server_receiver.clear_priority(0);
                            let result = state.load()?;

                            // Clear the scheduled pages for rendering
                            priority_server_receiver.clear_priority(1);

                            result_server_sender.try_send_priority(result, 0).map_err(
                                |x| format!("Could not send results to client: {}", x),
                            )?;
                        }
                        RendererAction::ToggleAlpha => {
                            state.alpha = !state.alpha;

                            // Clear the scheduled pages for rendering
                            priority_server_receiver.clear_priority(1);
                        }
                        RendererAction::ToggleInverse => {
                            state.inverse = !state.inverse;

                            // Clear the scheduled pages for rendering
                            priority_server_receiver.clear_priority(1);
                        }
                        RendererAction::Display(page) => {
                            if state.cache.get(page).is_none() {
                                // Sending `None` as data signals that it should be
                                // removed from the registry
                                result_server_sender
                                    .try_send_priority(
                                        RendererResult::Image { page, data: None },
                                        1,
                                    )
                                    .map_err(|x| {
                                        format!("Could not send result to client: {}", x)
                                    })?;
                                continue;
                            }

                            /* Load the image */
                            let data: Result<Pixmap, Error> = state.cache[page]
                                .to_pixmap(&state.ctm, &state.cs, state.alpha, false);

                            if data.is_err() {
                                continue;
                            }

                            let mut data_unwrapped: Pixmap = data.unwrap();
                            let n: usize = data_unwrapped.n() as usize;
                            if state.inverse {
                                for pixel in data_unwrapped.samples_mut().chunks_mut(n) {
                                    pixel[0] = 255 - pixel[0];
                                    pixel[1] = 255 - pixel[1];
                                    pixel[2] = 255 - pixel[2];
                                }
                            }

                            let res: Result<Image, String> = Image::new(&data_unwrapped);
                            if res.is_err() {
                                continue;
                            }

                            let image: Image = res.unwrap();
                            result_server_sender
                                .try_send_priority(
                                    RendererResult::Image {
                                        page,
                                        data: Some(Arc::new(RwLock::new(image))),
                                    },
                                    1,
                                )
                                .map_err(|x| {
                                    format!("Could not send results to client: {}", x)
                                })?;
                        }
                    };
                }

                Ok(())
            });

        /* ======================== Check and move the threads ======================= */
        if thread_render.is_finished() {
            thread_render.join().unwrap()?;
        } else {
            self.thread_render = Some(thread_render);
        }

        Ok(())
    }

    pub fn send_action(&self, action: RendererAction) -> Result<(), String> {
        match action {
            RendererAction::Load
            | RendererAction::ToggleAlpha
            | RendererAction::ToggleInverse => {
                Err("Cannot wait for Load, Alpha and Inverse".to_string())?
            }
            RendererAction::Display(_) => self
                .priority_client_sender
                .try_send_priority(action, 0)
                .map_err(|x| format!("Could not send action to renderer: {}", x))?,
        }

        Ok(())
    }

    pub fn send_and_confirm_action(&self, action: RendererAction) -> Result<(), String> {
        /* Sends an action to the renderer thread and waits until the thread confirms
         * that the action has been accepted and scheduled */
        match action {
            RendererAction::Load
            | RendererAction::ToggleAlpha
            | RendererAction::ToggleInverse => self
                .priority_client_sender
                .try_send_priority(action, 0)
                .map_err(|x| format!("Could not send action to renderer: {}", x))?,
            RendererAction::Display(_) => Err("Cannot wait for display".to_string())?,
        }

        let result = self
            .general_client_receiver
            .recv()
            .map_err(|x| format!("Could not receive action from renderer: {}", x))?;

        if result != action {
            return Err("Sent and received action from renderer don't match!".to_owned());
        }

        Ok(())
    }
}
