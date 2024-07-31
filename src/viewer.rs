use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    thread::{self, JoinHandle},
};

use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use mupdf::{Colorspace, Document, Error, Matrix, Page, Pixmap};
use nix::pty::Winsize;

use crate::{
    drivers::broadcast::UnboundedBroadcast, Config, Image, CONFIG, TERMINAL_SIZE,
};

pub struct ViewerOffset {
    scale: f32,
    page_first: usize,  /* The first page in the view */
    page_view: usize,   /* The page in the middle */
    offset: (f32, f32), /* Offset is given in page size units ≈ pixels */
    max_page_width: f32,
    cumulative_heights: Vec<f32>,
}

impl ViewerOffset {
    pub fn new(max_page_width: f32, cumulative_heights: Vec<f32>) -> Self {
        let config: &Config = CONFIG.get().unwrap();
        Self {
            scale: config.viewer.scale_default,
            page_first: 0,
            page_view: 0,
            offset: (0.0f32, 0.0f32),
            max_page_width: max_page_width,
            cumulative_heights: cumulative_heights,
        }
    }

    fn offset2page(&self, offset: f32) -> usize {
        /* Update page index by performing binary search */
        let res = self.cumulative_heights.binary_search_by(|x: &f32| {
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
            self.cumulative_heights[self.cumulative_heights.len() - 1]
                - terminal_size_lock.ws_ypixel as f32 / self.scale,
        );
        self.offset.1 = f32::max(self.offset.1, -10.0f32);
        self.offset.1 = f32::min(self.offset.1, max_yoffset);

        self.page_first = self.offset2page(self.offset.1);
        self.page_view = self.offset2page(
            self.offset.1 + terminal_size_lock.ws_ypixel as f32 * 0.5 / self.scale,
        );
        self.page_view = usize::min(self.page_view, self.cumulative_heights.len() - 1);
    }

    pub fn center_viewer(&mut self) {
        let terminal_size_lock: RwLockReadGuard<Winsize> =
            TERMINAL_SIZE.get().unwrap().read().unwrap();

        self.offset.0 =
            terminal_size_lock.ws_xpixel as f32 * 0.5 - self.max_page_width * self.scale * 0.5;
    }

    pub fn scroll(&mut self, amount: (f32, f32)) {
        self.offset.0 += amount.0;
        self.offset.1 += amount.1;
        self.bound_viewer();
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
}

pub struct Viewer {
    pub file: String,
    pub thread_render: Option<JoinHandle<Result<(), String>>>,

    pub reload_initiator: Option<Sender<()>>,
    reload_informer_broadcast: Option<UnboundedBroadcast<()>>,
    pub reload_informer_global: Option<Receiver<()>>,
    reload_informer_internal: Option<Receiver<()>>,
    render_scheduler: Option<Sender<usize>>, /* Page index as input */

    image_channel: Option<Receiver<(usize, Option<Arc<RwLock<Image>>>)>>,
    pub rerender_instructor: Option<Receiver<()>>,
    pub rerender_initializer: Option<Sender<()>>,
    pub offset: Arc<RwLock<ViewerOffset>>,
    pub images: HashMap<usize, Arc<RwLock<Image>>>,
    invalidated: HashMap<usize, ()>,
    scheduled4render: HashMap<usize, ()>,
    memory_used: usize,
    last_rendered: VecDeque<usize>,
}

impl Viewer {
    pub fn new(file: &str) -> Result<Self, String> {
        let viewer: Viewer = Self {
            file: file.to_string(),
            thread_render: None,
            reload_initiator: None,
            reload_informer_broadcast: None,
            reload_informer_global: None,
            reload_informer_internal: None,
            render_scheduler: None,
            image_channel: None,
            rerender_instructor: None,
            rerender_initializer: None,
            offset: Arc::new(RwLock::new(ViewerOffset::new(0f32, Vec::new()))),
            images: HashMap::new(),
            invalidated: HashMap::new(),
            scheduled4render: HashMap::new(),
            memory_used: 0,
            last_rendered: VecDeque::new(),
        };
        Ok(viewer)
    }

    pub fn run(&mut self) -> Result<(), String> {
        let (sender_render_init, receive_render_init) = unbounded::<()>();
        let (sender_display, receive_display) = unbounded::<usize>();
        let (sender_image, receive_image) =
            unbounded::<(usize, Option<Arc<RwLock<Image>>>)>();
        let (sender_rerender, receive_rerender) = unbounded::<()>();
        let mut reload_informer_broadcast: UnboundedBroadcast<()> =
            UnboundedBroadcast::new();

        self.reload_initiator = Some(sender_render_init);
        self.reload_informer_global = Some(reload_informer_broadcast.subscribe());
        self.reload_informer_internal = Some(reload_informer_broadcast.subscribe());
        self.reload_informer_broadcast = Some(reload_informer_broadcast);
        self.render_scheduler = Some(sender_display.clone()); /* Will be used later */
        self.image_channel = Some(receive_image);
        self.rerender_instructor = Some(receive_rerender);
        self.rerender_initializer = Some(sender_rerender.clone());

        /* ======================== Thread rendering PDF pages ======================= */
        let offset: Arc<RwLock<ViewerOffset>> = self.offset.clone();
        let file: String = self.file.clone();
        let sender_rerender1: Sender<()> = sender_rerender.clone();
        let informer_broadcast: UnboundedBroadcast<()> =
            self.reload_informer_broadcast.clone().unwrap();
        let thread_render: JoinHandle<Result<(), String>> = thread::spawn(move || {
            let config: &Config = CONFIG.get().unwrap();
            let mut document: Document;
            let mut cache: Vec<Page> = Vec::new();

            let cs: Colorspace = Colorspace::device_rgb();
            let ctm: Matrix = Matrix::new_scale(
                config.viewer.render_precision as f32,
                config.viewer.render_precision as f32,
            );

            macro_rules! reload_document {
                () => {
                    let mut max_page_width: f32 = -f32::INFINITY;
                    let mut cumulative_heights: Vec<f32> = Vec::new();

                    document = Document::open(&file).map_err(|x| {
                        format!("Could not open the given PDF file: {}", x)
                    })?;

                    if !document.is_pdf() {
                        Err("The given PDF file is not a PDF!".to_string())?;
                    }
                    cache.clear();

                    let page_count: i32 = document.page_count().map_err(|x| {
                        format!("Could not extract the number of pages: {}", x)
                    })?;

                    for i in 0..page_count {
                        let page: Page = document
                            .load_page(i)
                            .map_err(|x| format!("Could not load page {}: {}", i, x))?;

                        let bounds = page.bounds().map_err(|x| {
                            format!("Could not get bounds for page {}: {}", i, x)
                        })?;

                        let width: f32 = bounds.width();
                        let height: f32 = bounds.height();

                        cache.push(page);
                        max_page_width = f32::max(max_page_width, width);
                        cumulative_heights.push(
                            cumulative_heights.last().unwrap_or(&0.0f32)
                                + height
                                + config.viewer.margin_bottom,
                        );
                    }
                    {
                        let mut offset_lock: RwLockWriteGuard<ViewerOffset> =
                            offset.write().unwrap();
                        offset_lock.max_page_width = max_page_width;
                        offset_lock.cumulative_heights = cumulative_heights.to_owned();
                        /* Important to recalculate the bounds e.g when a page is removed
                         * in the PDF file update */
                        offset_lock.bound_viewer();
                    }
                    /* Notify that the document has been rendered */
                    let _ = informer_broadcast.send(());
                };
            }
            reload_document!();

            loop {
                select_biased! {
                    recv(receive_render_init) -> _ => {
                        reload_document!();
                    },
                    recv(receive_display) -> page => {
                        /* Do not raise exceptions. This can become a reality when the
                        * document has been modified and there are less pages than in the
                        * old version. The page should be removed in that case from the
                        * hash map on the other side. This is done by sending `None` */
                        if page.is_err() {
                            continue;
                        }

                        let page_unwrap: usize = page.unwrap();
                        if cache.get(page_unwrap).is_none() {
                            /* Ignore send errors since they occur only when trying to
                             * close the application */
                            let _ = sender_image.send((page_unwrap, None));
                            continue;
                        }

                        /* Load the image */
                        let data: Result<Pixmap, Error> =
                            cache[page_unwrap].to_pixmap(&ctm, &cs, 0.0f32, false);

                        if data.is_err() {
                            continue;
                        }

                        let res: Result<Image, String> = Image::new(&data.unwrap());
                        if res.is_err() {
                            continue;
                        }

                        let image: Image = res.unwrap();
                        /* Ignore send errors since they occur only when trying to
                        * close the application */
                        let _ = sender_image
                            .send((page_unwrap, Some(Arc::new(RwLock::new(image)))));
                        let _ = sender_rerender1.send(());
                    }
                }
            }
        });

        /* ======================== Check and move the threads ======================= */
        if thread_render.is_finished() {
            thread_render.join().unwrap()?;
        } else {
            self.thread_render = Some(thread_render);
        }

        Ok(())
    }

    /* ================================ Miscellaneous ================================ */

    /* Displays the pages based on the internal state of the offset.
     * Calculates how many pages should be rendered based on the terminal size
     */
    pub fn display_pages(&mut self) -> Result<Vec<usize>, String> {
        /* Track what images have been actually displayed on the screen to
         * later check if there occured errors during the display */
        let mut displayed: Vec<usize> = Vec::new();
        let config: &Config = CONFIG.get().unwrap();
        let offset_lock: RwLockReadGuard<ViewerOffset> = self.offset.read().unwrap();

        macro_rules! load_or_display {
            ($page:expr, $offset:expr, $scale:expr, $preload:expr) => {
                if (!self.images.contains_key(&$page)
                    || self.invalidated.contains_key(&$page))
                    && !self.scheduled4render.contains_key(&$page)
                {
                    let res = self.render_scheduler.as_ref().unwrap().send($page);
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
                                offset_lock.offset().0 as i32,
                                $offset as i32,
                                $scale as f64,
                            )
                            .unwrap();

                        if has_displayed {
                            displayed.push($page);
                        }
                    }
                }
            };
        }

        macro_rules! remove_image {
            ($page:expr) => {
                self.memory_used -= self.images[&$page].read().unwrap().size();
                self.images.remove(&$page);
            };
        }

        let image_channel: &mut Receiver<(usize, Option<Arc<RwLock<Image>>>)> =
            self.image_channel.as_mut().unwrap();

        /* Fetch all the images and put them into the hashmap.
         * Delete old ones if data limit has been exceeded */
        while let Ok((page, image)) = image_channel.try_recv() {
            /* Receiving a none implies that that image should be removed from the list */
            if image.is_none() {
                remove_image!(page);
                continue;
            }

            self.invalidated.remove(&page);

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

        let reload: bool = self
            .reload_informer_internal
            .clone()
            .unwrap()
            .try_recv()
            .is_ok();

        if reload {
            for k in self.images.keys() {
                self.invalidated.insert(*k, ());
            }
        }

        /* The index of the first rendered page */
        let mut page_index: usize = offset_lock.page_first();
        if offset_lock.cumulative_heights.len() <= page_index {
            return Ok(displayed);
        }

        /* Offset inside target page */
        let mut page_offset: f32 = offset_lock.offset().1;
        page_offset -= if page_index == 0 {
            0.0f32
        } else {
            offset_lock.cumulative_heights[page_index - 1]
        };

        /* Preload 1 page before the first displayed page to avoid flickering pages */
        if page_index > 0 {
            load_or_display!(page_index - 1, 0, 0, true);
        }

        /* Display the first page which is special because of extra offset calculation */
        load_or_display!(
            page_index,
            -page_offset * offset_lock.get_scale(),
            offset_lock.get_scale(),
            false
        );
        let mut displayed_offset: f32 = (offset_lock
            .page_height(page_index)
            .map_err(|x| format!("Could not retrieve page height: {}", x))?
            - page_offset)
            * offset_lock.get_scale();

        page_index += 1;

        /* Display the other pages that fit inside the viewpoint of the terminal */
        while displayed_offset
            < TERMINAL_SIZE.get().unwrap().read().unwrap().ws_ypixel as f32
            && page_index < offset_lock.cumulative_heights.len()
        {
            load_or_display!(
                page_index,
                displayed_offset,
                offset_lock.get_scale(),
                false
            );
            displayed_offset += offset_lock
                .page_height(page_index)
                .map_err(|x: String| format!("Could not retrieve page height: {}", x))?
                * offset_lock.get_scale();
            page_index += 1;
        }

        /* Preload N pages after the last displayed page */
        for _ in 0..usize::min(
            config.viewer.pages_preloaded,
            offset_lock.cumulative_heights.len() - page_index,
        ) {
            load_or_display!(page_index, 0, 0, true);
            page_index += 1;
        }

        Ok(displayed)
    }

    pub fn schedule_transfer(&mut self, page: usize) {
        let sender_rerender: Sender<()> = self.rerender_initializer.clone().unwrap();
        let image: Arc<RwLock<Image>> = self.images[&page].clone();
        let _ = image.read().unwrap().transfer();
        let _ = sender_rerender.send(());
    }
}
