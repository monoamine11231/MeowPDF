use core::f32;
use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{Arc, RwLock},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use crossterm::{
    cursor::{MoveToRow, RestorePosition, SavePosition},
    event::MouseEvent,
    execute,
    style::{Print, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use mupdf::Link;

use crate::{threads::renderer::*, Image, CONFIG, TERMINAL_SIZE};

#[derive(Clone, Copy, Debug)]
pub struct DisplayRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub trait Point {
    fn x(&self) -> i32;
    fn y(&self) -> i32;
}

pub trait Rect {
    fn x(&self) -> i32;
    fn y(&self) -> i32;
    fn width(&self) -> i32;
    fn height(&self) -> i32;
}

fn rect_point_intersect<R: Rect, P: Point>(rect: &R, point: &P) -> bool {
    let rect_x = rect.x();
    let rect_y = rect.y();
    let rect_width = rect.width();
    let rect_height = rect.height();

    let point_x = point.x();
    let point_y = point.y();

    let cr1 = point_x >= rect_x && point_x <= (rect_x + rect_width);
    let cr2 = point_y >= rect_y && point_y <= (rect_y + rect_height);

    cr1 && cr2
}

impl Point for MouseEvent {
    fn x(&self) -> i32 {
        self.column as i32
    }

    fn y(&self) -> i32 {
        self.row as i32
    }
}

impl Point for (i32, i32) {
    fn x(&self) -> i32 {
        self.0
    }

    fn y(&self) -> i32 {
        self.1
    }
}

impl Rect for DisplayRect {
    fn x(&self) -> i32 {
        self.x
    }

    fn y(&self) -> i32 {
        self.y
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }
}

impl Rect for mupdf::rect::Rect {
    fn x(&self) -> i32 {
        self.x0 as i32
    }

    fn y(&self) -> i32 {
        self.y0 as i32
    }

    fn width(&self) -> i32 {
        (self.x1 - self.x0) as i32
    }

    fn height(&self) -> i32 {
        (self.y1 - self.y0) as i32
    }
}

pub struct Viewer {
    scale: f32,
    page_first: usize,  /* The first page in the view */
    page_view: usize,   /* The page in the middle */
    offset: (f32, f32), /* Offset is given in page size units ≈ pixels */

    max_width: f32,
    cumulative_heights: Vec<f32>,
    widths: Vec<f32>,
    links: Vec<Vec<Link>>,

    pub images: HashMap<usize, Arc<RwLock<Image>>>,
    invalidated: HashMap<usize, ()>,
    scheduled4render: HashMap<usize, ()>,
    memory_used: usize,
    last_rendered: VecDeque<usize>,

    sender_rerender: Sender<()>,
}

impl Viewer {
    pub fn new() -> (Self, Receiver<()>) {
        let (sender_rerender, receiver_rerender) = unbounded::<()>();
        (
            Self {
                scale: 1.0,
                page_first: 0,
                page_view: 0,
                offset: (0.0f32, 0.0f32),
                max_width: -f32::INFINITY,
                cumulative_heights: Vec::new(),
                widths: Vec::new(),
                links: Vec::new(),
                images: HashMap::new(),
                invalidated: HashMap::new(),
                scheduled4render: HashMap::new(),
                memory_used: 0,
                last_rendered: VecDeque::new(),
                sender_rerender,
            },
            receiver_rerender,
        )
    }

    pub fn is_uninit(&self) -> bool {
        self.cumulative_heights.is_empty() && self.max_width == -f32::INFINITY
    }

    pub fn update_metadata(
        &mut self,
        max_width: f32,
        cumulative_heights: &[f32],
        widths: &[f32],
        links: &[Vec<Link>],
    ) {
        self.max_width = max_width;
        self.cumulative_heights = cumulative_heights.to_owned();
        self.widths = widths.to_owned();
        self.links = links.to_owned();
    }

    pub fn invalidate_registry(&mut self) {
        self.invalidated.clear();
        self.scheduled4render.clear();
        for k in self.images.keys() {
            self.invalidated.insert(*k, ());
        }
    }

    pub fn scroll(&mut self, amount: (f32, f32)) {
        self.offset.0 += amount.0;
        self.offset.1 += amount.1;
        self.bound_viewer();
    }

    pub fn pages(&self) -> usize {
        self.cumulative_heights.len()
    }

    pub fn jump(&mut self, page: usize) -> Result<(), String> {
        let res = if page >= self.cumulative_heights.len() {
            Err("Given page number is larger than the number of pages".to_owned())
        } else {
            Ok(())
        };

        self.page_first = usize::min(page, self.cumulative_heights.len() - 1);

        if page == 0 {
            self.offset.1 = 0.0f32;
        } else {
            self.offset.1 = self.cumulative_heights[self.page_first - 1];
        }
        self.bound_viewer();

        res
    }

    pub fn scale(&mut self, scale: f32) {
        self.scale += scale;
        self.bound_viewer();
    }

    #[allow(dead_code)]
    pub fn get_scale(&self) -> f32 {
        self.scale
    }

    pub fn page_first(&self) -> usize {
        self.page_first
    }

    #[allow(dead_code)]
    pub fn page_view(&self) -> usize {
        self.page_view
    }

    pub fn offset(&self) -> (f32, f32) {
        self.offset
    }

    /* ============================= Calculation methods ============================= */
    fn offset2page(&self, offset: f32) -> usize {
        /* Update page index by performing binary search */
        let res = self.cumulative_heights.binary_search_by(|x: &f32| {
            x.partial_cmp(&offset)
                .expect("NaN value found in cumulative height vector")
        });

        match res {
            Ok(x) => x,
            Err(x) => x,
        }
    }

    pub fn bound_viewer(&mut self) {
        let config = CONFIG.get().unwrap();

        let terminal_size_lock = TERMINAL_SIZE.get().unwrap().read().unwrap();

        self.scale = f32::max(self.scale, config.viewer.scale_min);
        self.offset.0 = f32::max(
            self.offset.0,
            f32::min(
                0.0f32,
                terminal_size_lock.width as f32 - self.max_width * self.scale,
            ),
        );
        self.offset.0 = f32::min(
            self.offset.0,
            f32::max(
                0.0f32,
                terminal_size_lock.width as f32 - self.max_width * self.scale,
            ),
        );
        let max_yoffset = f32::max(
            -10.0f32,
            self.cumulative_heights.last().unwrap_or(&0.0f32)
                - terminal_size_lock.height as f32 / self.scale,
        );
        self.offset.1 = f32::max(self.offset.1, -10.0f32);
        self.offset.1 = f32::min(self.offset.1, max_yoffset);

        self.page_first = self.offset2page(self.offset.1);
        self.page_view = self.offset2page(
            self.offset.1 + terminal_size_lock.height as f32 * 0.5 / self.scale,
        );
        let mut min_page = 0;
        if !self.cumulative_heights.is_empty() {
            min_page = self.cumulative_heights.len() - 1;
        }
        self.page_view = usize::min(self.page_view, min_page);
    }

    pub fn scale_page2terminal(&mut self) {
        let terminal_size_lock = TERMINAL_SIZE.get().unwrap().read().unwrap();

        let factor = terminal_size_lock.width as f32 / (self.max_width * self.scale);
        self.scale *= factor;
        self.bound_viewer();
    }

    pub fn center_viewer(&mut self) {
        let terminal_size_lock = TERMINAL_SIZE.get().unwrap().read().unwrap();

        self.offset.0 =
            terminal_size_lock.width as f32 * 0.5 - self.max_width * self.scale * 0.5;
    }

    pub fn page_height(&self, page: usize) -> Result<f32, String> {
        let page_prev_height = if page > 0 {
            *self.cumulative_heights.get(page - 1).unwrap_or(&0.0f32)
        } else {
            0.0f32
        };

        let page_height = *self.cumulative_heights.get(page).ok_or(format!(
            "Wrong page index provided when retrieving page height, index: {}",
            page
        ))? - page_prev_height;

        Ok(page_height)
    }

    pub fn page_width(&self, page: usize) -> Result<f32, String> {
        self.widths
            .get(page)
            .ok_or(format!("Could not get page {} width", page))
            .copied()
    }

    fn calculate_display_bounds(&self) -> Vec<(usize, DisplayRect)> {
        /* Calculated bounds to display the pages in the temrinal */
        let mut bounds = Vec::new();
        /* Current terminal height */
        let terminal_height = TERMINAL_SIZE.get().unwrap().read().unwrap().height as f32;
        /* Bottom margin */
        let margin_bottom = CONFIG.get().unwrap().viewer.margin_bottom;
        /* Number of pages */
        let pages_num = self.cumulative_heights.len();
        /* The index of the first rendered page */
        let mut page_index = self.page_first();

        if pages_num <= page_index {
            return bounds;
        }

        let mut page_offset = self.offset().1;
        page_offset -= if page_index == 0 {
            0.0f32
        } else {
            self.cumulative_heights[page_index - 1]
        };
        let mut displayed_offset = -page_offset * self.scale;

        /* Cumulative displayed page height */
        while displayed_offset < terminal_height && page_index < pages_num {
            let height = ((self
                .page_height(page_index)
                .expect("Could not retrieve page height"))
                - margin_bottom)
                * self.scale;

            let width = self.page_width(page_index).unwrap() * self.scale;

            bounds.push((
                page_index,
                DisplayRect {
                    x: self.offset().0 as i32,
                    y: displayed_offset as i32,
                    width: width as i32,
                    height: height as i32,
                },
            ));

            displayed_offset += height + margin_bottom * self.scale;
            page_index += 1;
        }

        bounds
    }

    /* ================================ Miscellaneous ================================ */
    pub fn intersect_link(&self, mouse: MouseEvent) -> Option<Link> {
        let mut intersected_page = None;
        let mut intersected_rect = None;
        for bound in self.calculate_display_bounds() {
            let (page, rect) = bound;
            if rect_point_intersect(&rect, &mouse) {
                intersected_page = Some(page);
                intersected_rect = Some(rect);
                break;
            }
        }

        intersected_page?;

        let page_point = (
            ((mouse.column as i32 - intersected_rect.unwrap().x) as f32 / self.scale)
                as i32,
            ((mouse.row as i32 - intersected_rect.unwrap().y) as f32 / self.scale) as i32,
        );
        let mut intersected_link = None;
        let links = &self.links[intersected_page.unwrap()];
        for link in links.iter() {
            if rect_point_intersect(&link.bounds, &page_point) {
                intersected_link = Some(link);
                break;
            }
        }

        intersected_link?;

        Some(intersected_link.unwrap().clone())
    }

    pub fn uri_hint(&self, link: &Link) {
        let config = CONFIG.get().unwrap();
        if !config.viewer.uri_hint.enabled {
            return;
        }

        let terminal_size = TERMINAL_SIZE.get().unwrap().read().unwrap();

        let width = (terminal_size.width as f32 * config.viewer.uri_hint.width) as usize;
        if width < 5 {
            return;
        }

        let text = if link.uri.len() <= width {
            if link.uri.starts_with('#') {
                format!("Page {}", link.page + 1)
            } else {
                link.uri.to_string()
            }
        } else {
            format!("{}..", &link.uri[0..(width - 2)])
        };

        execute!(
            io::stdout(),
            SavePosition,
            MoveToRow(terminal_size.rows),
            SetBackgroundColor(config.viewer.uri_hint.background),
            SetForegroundColor(config.viewer.uri_hint.foreground),
            Print(text),
            ResetColor,
            RestorePosition
        )
        .unwrap();
    }

    pub fn handle_image(&mut self, page: usize, image: Option<Arc<RwLock<Image>>>) {
        macro_rules! remove_image {
            ($page:expr) => {
                self.memory_used -= self.images[&$page].read().unwrap().size();
                self.images.remove(&$page);
                self.invalidated.remove(&$page);
            };
        }

        let config = CONFIG.get().unwrap();

        if image.is_none() {
            remove_image!(page);
            return;
        }

        if self.invalidated.contains_key(&page) {
            remove_image!(page);
        }

        let image_unwrapped = image.unwrap();
        self.memory_used += image_unwrapped.read().unwrap().size();
        self.last_rendered.push_back(page);

        self.images.insert(page, image_unwrapped);
        self.scheduled4render.remove(&page);

        while self.memory_used >= config.viewer.memory_limit {
            let page2remove = self.last_rendered.pop_front().unwrap();
            if !self.images.contains_key(&page2remove) {
                continue;
            }
            remove_image!(page2remove);
        }
    }

    fn load_or_display(
        &mut self,
        page: usize,
        rect: DisplayRect,
        preload: bool,
        renderer: &Renderer,
    ) -> Option<usize> {
        if (!self.images.contains_key(&page) || self.invalidated.contains_key(&page))
            && !self.scheduled4render.contains_key(&page)
        {
            let res = renderer.send_action(RendererAction::Display(page));
            if res.is_ok() {
                self.scheduled4render.insert(page, ());
            }

            return None;
        }

        if self.images.contains_key(&page) {
            let image = self.images[&page].read().unwrap();
            if preload {
                image.check().unwrap();
                return Some(page);
            } else {
                let has_displayed = image.display(rect).unwrap();

                if has_displayed {
                    return Some(page);
                }
            }
        }

        None
    }

    /* Displays the pages based on the internal state of the offset.
     * Calculates how many pages should be rendered based on the terminal size */
    pub fn display_pages(&mut self, renderer: &Renderer) -> Result<Vec<usize>, String> {
        let config = CONFIG.get().unwrap();
        let preloaded = config.viewer.pages_preloaded;

        /* Track what images have been actually displayed on the screen to
         * later check if there occured errors during the display */
        let mut displayed = Vec::new();
        let none_rect = DisplayRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };

        /* The index of the first rendered page */
        let mut page_index = self.page_first();
        if self.cumulative_heights.len() <= page_index {
            return Ok(displayed);
        }

        /* Preload N pages before the first displayed page to avoid flickering pages */
        for i in 0..usize::min(preloaded, page_index) {
            let r = self.load_or_display(page_index - 1 - i, none_rect, true, renderer);
            if let Some(page) = r {
                displayed.push(page);
            }
        }

        for (page, rect) in self.calculate_display_bounds() {
            let r = self.load_or_display(page, rect, false, renderer);
            if let Some(page) = r {
                displayed.push(page);
            }
            page_index += 1;
        }

        /* Preload N pages after the last displayed page */
        for _ in 0..usize::min(preloaded, self.cumulative_heights.len() - page_index) {
            let r = self.load_or_display(page_index, none_rect, true, renderer);
            if let Some(page) = r {
                displayed.push(page);
            }
            page_index += 1;
        }

        Ok(displayed)
    }

    pub fn schedule_transfer(&mut self, page: usize) {
        let image = self.images[&page].clone();
        let _ = image.read().unwrap().transfer();
        let _ = self.sender_rerender.send(());
    }
}
