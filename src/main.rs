mod drivers;
use drivers::{graphics::terminal_graphics_test_support, tui::*};

mod image;
use crate::image::*;

use std::ops::Deref;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::{io::stdout, io::Write, sync::Mutex, thread};

use mupdf::Colorspace;
use mupdf::Matrix;
use mupdf::Pixmap;
use mupdf::{document::Document, Page};
use nix::libc;
use nix::pty::Winsize;
use nix::sys::termios::Termios;

use notify::RecursiveMode;
use notify::{
    event::{DataChange, ModifyKind},
    Watcher,
};

const PRECISION: f64 = 2.0f64;
const SCROLL: f32 = 10.0f32;
const SCALE: f32 = 0.5f32;
const SCALE_MIN: f32 = 0.2f32;
const MARGIN_BOTTOM: f32 = 100.0f32;

fn main() {
    /* ============================= Uncook the terminal ============================= */
    let tty_data_original_main: Termios =
        terminal_control_raw_mode().expect("Error when setting terminal to raw mode");
    let tty_data_original_panic_hook: Mutex<Termios> =
        Mutex::from(tty_data_original_main.clone());

    terminal_tui_clear().expect("Error when clearing the screen");

    /* ========================== Cook the terminal on panic ========================= */
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let tty = tty_data_original_panic_hook.lock().unwrap();
        /* Atleast try to cook the terminal on error before printing the message.
         * Do not handle the error to prevent possible infinite loops when panicking. */
        let _ = terminal_control_default_mode(tty.deref());
        default_panic(info);
    }));

    /* ========== Check if the terminal supports the Kitty graphics protocol ========= */
    terminal_graphics_test_support()
        .expect("Error when testing terminal support of the Kitty graphics protocol");

    /* ==================== Thread notifying terminal size change ==================== */
    let (sender_winsize, receive_winsize) = channel::<libc::winsize>();
    thread::spawn(move || {
        let mut wz: libc::winsize = terminal_tui_get_dimensions().unwrap();
        loop {
            thread::sleep(Duration::from_millis(100));

            let tmp: libc::winsize = terminal_tui_get_dimensions().unwrap();
            if tmp == wz {
                continue;
            }

            wz = tmp;
            /* Notify that the terminal window has been changed */
            sender_winsize
                .send(wz)
                .expect("Could not send a terminal window size change signal");
        }
    });

    /* ========================= Thread notifying file change ======================== */
    let (sender_file, receive_file) = channel::<bool>();
    let mut watcher_file =
        notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let event: notify::Event =
                res.expect("Could not watch file changes for the given file");

            match event.kind {
                notify::EventKind::Modify(ModifyKind::Data(DataChange::Any)) => {
                    sender_file
                        .send(true)
                        .expect("Could not send a file change signal");
                }
                _ => (),
            }
        })
        .expect("Could not initialize a file watcher for the given file");

    let file: String = std::env::args().nth(1).expect("No provided pdf!");

    watcher_file
        .watch(Path::new(file.as_str()), RecursiveMode::NonRecursive)
        .expect("Could not start watching file changes for the given file");

    /* ============================== Main program loop ============================== */
    let mut app: AppState = AppState::init(file).expect("Could not create the app state");

    let mut ctm: Matrix = Matrix::new_scale(PRECISION as f32, PRECISION as f32);
    let cs: Colorspace = Colorspace::device_rgb();

    let pxpercol: f64 = app.size.ws_xpixel as f64 / app.size.ws_col as f64;
    let pxperrow: f64 = app.size.ws_ypixel as f64 / app.size.ws_row as f64;

    let paddingcol: usize = (pxpercol * PRECISION / SCALE_MIN as f64).ceil() as usize;
    let paddingrow: usize = (pxperrow * PRECISION / SCALE_MIN as f64).ceil() as usize;

    let padding: usize = std::cmp::max(paddingcol, paddingrow);
    for i in 0..5 {
        let pm: Pixmap = app.cache[i].to_pixmap(&ctm, &cs, 0.0, true).unwrap();
        app.images
            .push(Image::new(&pm, PRECISION, padding).unwrap());
    }

    let mut key_iter = terminal_tui_key_iter().peekable();
    let mut winsize_iter = receive_winsize.try_iter().peekable();
    let mut file_iter = receive_file.try_iter().peekable();

    loop {
        /* Break the loop if a CTRL-C or similar sigint signal was sent */
        if chan_has!(key_iter) {
            let key: TerminalKey = **key_iter.peek().as_ref().unwrap();
            let event: (bool, bool) = handle_key(key, &mut app);
            if event.0 {
                break;
            }

            key_iter.next();
        }

        /* Rerender on terminal window size change */
        if chan_has!(winsize_iter) {
            app.size = *winsize_iter.peek().unwrap();
            winsize_iter.next();
        }

        /* Do something on file change */
        if chan_has!(file_iter) {
            file_iter.next();
        }

        terminal_tui_clear().expect("Error when clearing the screen");

        app.display_pages().expect("Could not display pages");

        print!(
            "\x1B[{};1H\x1B[1;31mPAGE {} | SCALE {}",
            app.size.ws_row,
            app.offset.page(),
            app.scale
        );
        stdout().flush().expect("Could not flush stdout.");

        while !chan_has!(key_iter) && !chan_has!(winsize_iter) && !chan_has!(file_iter) {
            key_iter.next();
            winsize_iter.next();
            file_iter.next();
        }
    }

    /* ========================== Cook the terminal on exit ========================== */
    terminal_control_default_mode(&tty_data_original_main)
        .expect("Error when setting terminal to default mode");
}

/* `true` indicates that the caller should exit *safely* the current process */
fn handle_key(key: TerminalKey, app: &mut AppState) -> (bool, bool) {
    let res: (bool, bool) = match key {
        TerminalKey::CTRLC
        | TerminalKey::CTRLD
        | TerminalKey::OTHER(b'q')
        | TerminalKey::OTHER(b'Q') => (true, false),
        TerminalKey::UP => {
            app.offset.scroll((0.0f32, -SCROLL));
            return (false, false);
        }
        TerminalKey::DOWN => {
            app.offset.scroll((0.0f32, SCROLL));
            return (false, false);
        }
        TerminalKey::LEFT => {
            app.offset.scroll((-SCROLL, 0.0f32));
            return (false, false);
        }
        TerminalKey::RIGHT => {
            app.offset.scroll((SCROLL, 0.0f32));
            return (false, false);
        }
        TerminalKey::OTHER(b'+') => {
            app.scale += SCALE;
            return (false, false);
        }
        TerminalKey::OTHER(b'-') => {
            app.scale -= SCALE;
            if app.scale < SCALE_MIN {
                app.scale = SCALE_MIN;
            }
            return (false, false);
        }
        _ => (false, false),
    };
    return res;
}

struct ViewerOffset {
    page: usize,
    /* Offset is given in page width and page height units */
    offset: (f32, f32),
    cumulative_heights: Vec<f32>,
}

impl ViewerOffset {
    fn new(cumulative_heights: Vec<f32>) -> Self {
        Self {
            page: 0,
            offset: (0.0f32, 0.0f32),
            cumulative_heights: cumulative_heights,
        }
    }

    fn scroll(&mut self, amount: (f32, f32)) {
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

    fn jump(&mut self, page: usize) {
        self.page = page;

        if page == 0 {
            self.offset.1 = 0.0f32;
        } else {
            self.offset.1 = self.cumulative_heights[page - 1];
        }
    }

    fn page(&self) -> usize {
        return self.page;
    }

    fn offset(&self) -> (f32, f32) {
        return self.offset;
    }

    /* ============================= Calculation methods ============================= */
    fn page_height(&self, page: usize) -> Result<f32, String> {
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

struct AppState {
    size: Winsize,
    file: String,
    document: Document,
    cache: Vec<Page>,
    images: Vec<Image>,
    scale: f32,
    offset: ViewerOffset,
}

impl AppState {
    /* =============================== Document loading ============================== */
    fn init(file: String) -> Result<Self, String> {
        let winsize: Winsize = terminal_tui_get_dimensions()
            .map_err(|x| format!("Could not get terminal dimensions: {}", x))?;

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

        let app: AppState = Self {
            size: winsize,
            file: file,
            document: document,
            cache: cache,
            images: Vec::new(),
            scale: 0.2,
            offset: ViewerOffset::new(cumulative_heights),
        };
        Ok(app)
    }

    /* Used when watched document has been changed */
    // fn document_load(&mut self) -> Result<(), String> {
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
    fn display_pages(&self) -> Result<(), String> {
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

#[macro_export]
macro_rules! chan_has {
    ($chan:expr) => {
        $chan.peek().is_some()
    };
}
