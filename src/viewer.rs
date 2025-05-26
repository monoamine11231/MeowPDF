use core::f32;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, RwLock, RwLockReadGuard},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use nix::pty::Winsize;

use crate::{threads::renderer::*, Config, Image, CONFIG, TERMINAL_SIZE};

pub struct Viewer {
    scale: f32,
    page_first: usize,  /* The first page in the view */
    page_view: usize,   /* The page in the middle */
    offset: (f32, f32), /* Offset is given in page size units ≈ pixels */
    max_page_width: f32,
    cumulative_heights: Vec<f32>,

    pub images: HashMap<usize, Arc<RwLock<Image>>>,
    invalidated: HashMap<usize, ()>,
    scheduled4render: HashMap<usize, ()>,
    memory_used: usize,
    last_rendered: VecDeque<usize>,

    sender_rerender: Sender<()>,
}

impl Viewer {
    pub fn new() -> (Self, Receiver<()>) {
        let config: &Config = CONFIG.get().unwrap();
        let (sender_rerender, receiver_rerender) = unbounded::<()>();
        (
            Self {
                scale: config.viewer.scale_default,
                page_first: 0,
                page_view: 0,
                offset: (0.0f32, 0.0f32),
                max_page_width: -f32::INFINITY,
                cumulative_heights: Vec::new(),
                images: HashMap::new(),
                invalidated: HashMap::new(),
                scheduled4render: HashMap::new(),
                memory_used: 0,
                last_rendered: VecDeque::new(),
                sender_rerender: sender_rerender,
            },
            receiver_rerender,
        )
    }

    pub fn update_metadata(
        &mut self,
        max_page_width: f32,
        cumulative_heights: &Vec<f32>,
    ) {
        self.max_page_width = max_page_width;
        self.cumulative_heights = cumulative_heights.clone();
    }

    pub fn invalidate_registry(&mut self) {
        self.invalidated.clear();
        self.scheduled4render.clear();
        for k in self.images.keys() {
            self.invalidated.insert(*k, ());
        }
    }

    fn offset2page(&self, offset: f32) -> usize {
        /* Update page index by performing binary search */
        let res: Result<usize, usize> = self.cumulative_heights.binary_search_by(|x: &f32| {
            x.partial_cmp(&offset)
                .expect("NaN value found in cumulative height vector")
        });

        let index: usize = match res {
            Ok(x) => x,
            Err(x) => x,
        };

        index
    }

    pub fn bound_viewer(&mut self) {
        let config: &Config = CONFIG.get().unwrap();

        let terminal_size_lock: RwLockReadGuard<Winsize> =
            TERMINAL_SIZE.get().unwrap().read().unwrap();

        self.scale = f32::max(self.scale, config.viewer.scale_min);
        self.offset.0 = f32::max(
            self.offset.0,
            f32::min(
                0.0f32,
                terminal_size_lock.ws_xpixel as f32 - self.max_page_width * self.scale,
            ),
        );
        self.offset.0 = f32::min(
            self.offset.0,
            f32::max(
                0.0f32,
                terminal_size_lock.ws_xpixel as f32 - self.max_page_width * self.scale,
            ),
        );
        let max_yoffset: f32 = f32::max(
            -10.0f32,
            self.cumulative_heights
                .get(self.cumulative_heights.len() - 1)
                .unwrap_or(&0.0f32)
                - terminal_size_lock.ws_ypixel as f32 / self.scale,
        );
        self.offset.1 = f32::max(self.offset.1, -10.0f32);
        self.offset.1 = f32::min(self.offset.1, max_yoffset);

        self.page_first = self.offset2page(self.offset.1);
        self.page_view = self.offset2page(
            self.offset.1 + terminal_size_lock.ws_ypixel as f32 * 0.5 / self.scale,
        );
        let mut min_page: usize = 0;
        if self.cumulative_heights.len() > 0 {
            min_page = self.cumulative_heights.len() - 1;
        }
        self.page_view = usize::min(self.page_view, min_page);
    }

    pub fn center_viewer(&mut self) {
        let terminal_size_lock: RwLockReadGuard<Winsize> =
            TERMINAL_SIZE.get().unwrap().read().unwrap();

        self.offset.0 = terminal_size_lock.ws_xpixel as f32 * 0.5
            - self.max_page_width * self.scale * 0.5;
    }

    pub fn scroll(&mut self, amount: (f32, f32)) {
        self.offset.0 += amount.0;
        self.offset.1 += amount.1;
        self.bound_viewer();
    }

    pub fn pages(&self) -> usize {
        return self.cumulative_heights.len();
    }

    pub fn jump(&mut self, page: usize) -> Result<(), String> {
        if page >= self.cumulative_heights.len() {
            Err("Given page number is larger than the number of pages")?;
        }

        self.page_first = usize::min(page, self.cumulative_heights.len() - 1);

        if page == 0 {
            self.offset.1 = 0.0f32;
        } else {
            self.offset.1 = self.cumulative_heights[self.page_first - 1];
        }
        self.bound_viewer();

        Ok(())
    }

    pub fn scale(&mut self, scale: f32) {
        self.scale += scale;
        self.bound_viewer();
    }

    pub fn get_scale(&self) -> f32 {
        return self.scale;
    }

    pub fn page_first(&self) -> usize {
        return self.page_first;
    }

    #[allow(dead_code)]
    pub fn page_view(&self) -> usize {
        return self.page_view;
    }

    pub fn offset(&self) -> (f32, f32) {
        return self.offset;
    }

    /* ============================= Calculation methods ============================= */
    pub fn page_height(&self, page: usize) -> Result<f32, String> {
        let page_prev_height: f32;
        if page > 0 {
            page_prev_height = *self.cumulative_heights.get(page - 1).unwrap_or(&0.0f32);
        } else {
            page_prev_height = 0.0f32;
        }

        let page_height: f32;
        page_height = *self.cumulative_heights.get(page).ok_or(format!(
            "Wrong page index provided when retrieving page height, index: {}",
            page
        ))? - page_prev_height;

        Ok(page_height)
    }

    /* ================================ Miscellaneous ================================ */


    pub fn handle_image(&mut self, page: usize, image: Option<Arc<RwLock<Image>>>) {
        macro_rules! remove_image {
            ($page:expr) => {
                self.memory_used -= self.images[&$page].read().unwrap().size();
                self.images.remove(&$page);
                self.invalidated.remove(&$page);
            };
        }

        let config: &Config = CONFIG.get().unwrap();

        if image.is_none() {
            remove_image!(page);
            return;
        }

        if self.invalidated.contains_key(&page) {
            remove_image!(page);
        }

        let image_unwrapped: Arc<RwLock<Image>> = image.unwrap();
        self.memory_used += image_unwrapped.read().unwrap().size();
        self.last_rendered.push_back(page);

        self.images.insert(page, image_unwrapped);
        self.scheduled4render.remove(&page);

        while self.memory_used >= config.viewer.memory_limit {
            let page2remove: usize = self.last_rendered.pop_front().unwrap();
            if !self.images.contains_key(&page2remove) {
                continue;
            }
            remove_image!(page2remove);
        }
    }

    /* Displays the pages based on the internal state of the offset.
     * Calculates how many pages should be rendered based on the terminal size
     */
    pub fn display_pages(&mut self, renderer: &Renderer) -> Result<Vec<usize>, String> {
        /* Track what images have been actually displayed on the screen to
         * later check if there occured errors during the display */
        let mut displayed: Vec<usize> = Vec::new();
        let config: &Config = CONFIG.get().unwrap();

        macro_rules! load_or_display {
            ($page:expr, $offset:expr, $scale:expr, $preload:expr) => {{
                if (!self.images.contains_key(&$page)
                    || self.invalidated.contains_key(&$page))
                    && !self.scheduled4render.contains_key(&$page)
                {
                    let res =
                        renderer.send_action(RendererAction::Display($page));
                    if res.is_ok() {
                        self.scheduled4render.insert($page, ());
                    }
                }

                if self.images.contains_key(&$page) {
                    if $preload {
                        self.images[&$page].read().unwrap().check().unwrap();
                        displayed.push($page);
                    } else {
                        let has_displayed: bool = self.images[&$page]
                            .read()
                            .unwrap()
                            .display(
                                self.offset().0 as i32,
                                $offset as i32,
                                $scale as f64,
                            )
                            .unwrap();

                        if has_displayed {
                            displayed.push($page);
                        }
                    }
                }
            }};
        }

        /* The index of the first rendered page */
        let mut page_index: usize = self.page_first();
        if self.cumulative_heights.len() <= page_index {
            return Ok(displayed);
        }

        /* Preload N pages before the first displayed page to avoid flickering pages */
        for i in 0..usize::min(config.viewer.pages_preloaded, page_index) {
            load_or_display!(page_index - 1 - i, 0, 0, true);
        }

        /* Offset inside target page */
        let mut page_offset: f32 = self.offset().1;
        page_offset -= if page_index == 0 {
            0.0f32
        } else {
            self.cumulative_heights[page_index - 1]
        };

        /* Display the first page which is special because of extra offset calculation */
        load_or_display!(
            page_index,
            -page_offset * self.get_scale(),
            self.get_scale(),
            false
        );
        let mut displayed_offset: f32 = (self
            .page_height(page_index)
            .map_err(|x| format!("Could not retrieve page height: {}", x))?
            - page_offset)
            * self.get_scale();

        page_index += 1;

        /* Display the other pages that fit inside the viewpoint of the terminal */
        while displayed_offset
            < TERMINAL_SIZE.get().unwrap().read().unwrap().ws_ypixel as f32
            && page_index < self.cumulative_heights.len()
        {
            load_or_display!(page_index, displayed_offset, self.get_scale(), false);
            displayed_offset += self
                .page_height(page_index)
                .map_err(|x: String| format!("Could not retrieve page height: {}", x))?
                * self.get_scale();
            page_index += 1;
        }

        /* Preload N pages after the last displayed page */
        for _ in 0..usize::min(
            config.viewer.pages_preloaded,
            self.cumulative_heights.len() - page_index,
        ) {
            load_or_display!(page_index, 0, 0, true);
            page_index += 1;
        }

        Ok(displayed)
    }

    pub fn schedule_transfer(&mut self, page: usize) {
        let image: Arc<RwLock<Image>> = self.images[&page].clone();
        let _ = image.read().unwrap().transfer();
        let _ = self.sender_rerender.send(());
    }
}
