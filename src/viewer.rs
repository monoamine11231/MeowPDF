use mupdf::{Document, Page};
use nix::pty::Winsize;

use crate::{terminal_tui_get_dimensions, Image};


const PRECISION: f64 = 2.0f64;
const MARGIN_BOTTOM: f32 = 100.0f32;

pub struct ViewerOffset {
    page: usize,
    /* Offset is given in page width and page height units */
    offset: (f32, f32),
    cumulative_heights: Vec<f32>,
}

impl ViewerOffset {
    pub fn new(cumulative_heights: Vec<f32>) -> Self {
        Self {
            page: 0,
            offset: (0.0f32, 0.0f32),
            cumulative_heights: cumulative_heights,
        }
    }

    pub fn scroll(&mut self, amount: (f32, f32)) {
        self.offset.0 += amount.0;
        self.offset.1 += amount.1;

        /* Update page index by performing binary search */
        let res = self.cumulative_heights.binary_search_by(|x: &f32| {
            x.partial_cmp(&self.offset.1)
                .expect("NaN value found in cumulative height vector")
        });

        let index: usize = match res {
            Ok(x) => x,
            Err(x) => x,
        };

        self.page = index;
    }

    pub fn jump(&mut self, page: usize) {
        self.page = page;

        if page == 0 {
            self.offset.1 = 0.0f32;
        } else {
            self.offset.1 = self.cumulative_heights[page - 1];
        }
    }

    pub fn page(&self) -> usize {
        return self.page;
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
    pub size: Winsize,
    pub file: String,
    pub document: Document,
    pub cache: Vec<Page>,
    pub images: Vec<Image>,
    pub scale: f32,
    pub offset: ViewerOffset,
}

impl Viewer {
    /* =============================== Document loading ============================== */
    pub fn init(file: String) -> Result<Self, String> {
        let winsize: Winsize = terminal_tui_get_dimensions()
            .map_err(|x| format!("Could not get terminal dimensions: {}", x))?;

        if winsize.ws_xpixel == 0 || winsize.ws_ypixel == 0 {
            Err("Could not get terminal dimensions: Invalid results from IOCTL")?;
        }
        
        let document: Document = Document::open(file.as_str())
            .map_err(|x| format!("Could not open the given PDF file: {}", x))?;

        if !document.is_pdf() {
            Err("The given PDF file is not a PDF!".to_string())?;
        }

        let mut cumulative_heights: Vec<f32> = Vec::new();
        let mut cache: Vec<Page> = Vec::new();

        let page_count: i32 = document
            .page_count()
            .map_err(|x| format!("Could not extract the number of pages: {}", x))?;

        for i in 0..page_count {
            let page: Page = document
                .load_page(i)
                .map_err(|x| format!("Could not load page {}: {}", i, x))?;

            let height: f32 = page
                .bounds()
                .map_err(|x| format!("Could not get bounds for page {}: {}", i, x))?
                .height();

            cumulative_heights.push(
                cumulative_heights.last().unwrap_or(&0.0f32) + height + MARGIN_BOTTOM,
            );
            cache.push(page);
        }

        let viewer: Viewer = Self {
            size: winsize,
            file: file,
            document: document,
            cache: cache,
            images: Vec::new(),
            scale: 0.2,
            offset: ViewerOffset::new(cumulative_heights),
        };
        Ok(viewer)
    }

    /* Used when watched document has been changed */
    // pub fn document_load(&mut self) -> Result<(), String> {
    //     let mut cumulative_heights: Vec<f32> = Vec::new();
    //     let mut cache: Vec<Page> = Vec::new();

    //     let page_count: i32 = self
    //         .document
    //         .page_count()
    //         .map_err(|x| format!("Could not extract the number of pages: {}", x))?;

    //     for i in 0..page_count {
    //         let page: Page = self
    //             .document
    //             .load_page(i)
    //             .map_err(|x| format!("Could not load page {}: {}", i, x))?;

    //         let height: f32 = page
    //             .bounds()
    //             .map_err(|x| format!("Could not get bounds for page {}: {}", i, x))?
    //             .height();

    //         cumulative_heights.push(
    //             cumulative_heights.last().unwrap_or(&0.0f32) + height + MARGIN_BOTTOM,
    //         );
    //         cache.push(page);
    //     }

    //     self.cache = cache;

    //     Ok(())
    // }

    /* ================================ Miscellaneous ================================ */

    /* Displays the pages based on the internal state of the offset.
     * Calculates how many pages should be rendered based on the terminal size
     */
    pub fn display_pages(&self) -> Result<(), String> {
        /* The index of the first rendered page */
        let mut page_index: usize = self.offset.page();

        /* Offset inside target page */
        let mut page_offset: f32 = self.offset.offset().1;
        page_offset -= if page_index == 0 {
            0.0f32
        } else {
            self.offset.cumulative_heights[page_index - 1]
        };

        self.images[page_index]
            .display(
                self.offset.offset().0 as i32,
                (-page_offset * self.scale) as i32,
                self.scale as f64,
                &self.size,
            )
            .map_err(|x| format!("Could not display page {}: {}", 0, x))?;

        let mut px_displayed_vertically: usize = ((self
            .offset
            .page_height(page_index)
            .map_err(|x| {
            format!("Could not retrieve page height: {}", x)
        })? - page_offset)
            * self.scale) as usize;

        page_index += 1;
        while px_displayed_vertically < self.size.ws_ypixel as usize {
            if page_index >= self.images.len() {
                break;
            }
            self.images[page_index]
                .display(
                    self.offset.offset().0 as i32,
                    px_displayed_vertically as i32,
                    self.scale as f64,
                    &self.size,
                )
                .map_err(|x| format!("Could not display page {}: {}", page_index, x))?;

            px_displayed_vertically += (self
                .offset
                .page_height(page_index)
                .map_err(|x| format!("Could not retrieve page height: {}", x))?
                * self.scale) as usize;
            page_index += 1;
        }

        Ok(())
    }
}