mod tui;
use crate::tui::*;

mod graphics;
use crate::graphics::*;

mod image;
use crate::image::*;

use std::ops::Deref;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::time::SystemTime;
use std::{io::stdout, io::Write, sync::Mutex, thread};

use mupdf::Colorspace;
use mupdf::Matrix;
use mupdf::Pixmap;
use mupdf::Rect;
use mupdf::{document::Document, Page};
use nix::libc;
use nix::pty::Winsize;
use nix::sys::termios::Termios;

use notify::RecursiveMode;
use notify::{
    event::{DataChange, ModifyKind},
    Watcher,
};

const PRECISION: f64 = 3.0f64;
const SCROLL: f32 = 1.0f32;
const SCALE: f32 = 0.5f32;
const SCALE_MIN: f32 = 0.2f32;

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

    let mut pm: Pixmap = app.cache[0].to_pixmap(&ctm, &cs, 0.0, true).unwrap();

    let b: Rect = app.cache[0].bounds().unwrap();

    let a = SystemTime::now();
    let img: Image = Image::new(&pm, PRECISION).unwrap();

    // panic!("w: {}|h: {}", pm.width(), pm.height());
    // match a.elapsed() {
    //     Ok(x) => panic!("aa: {}", x.as_millis()),
    //     Err(_) => panic!("asdasd"),
    // };

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

            if event.1 {
                ctm = Matrix::new_scale(app.scale, app.scale);
                pm = app.cache[0].to_pixmap(&ctm, &cs, 0.0, true).unwrap();

                terminal_graphics_transfer_bitmap(
                    228,
                    pm.width() as usize,
                    pm.height() as usize,
                    pm.samples(),
                    false,
                )
                .unwrap();
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

        img.display(
            app.offset.0 as i32,
            app.offset.1 as i32,
            app.scale as f64,
            &app.size,
        ).unwrap();

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
            app.offset.1 -= SCROLL;
            return (false, false);
        }
        TerminalKey::DOWN => {
            app.offset.1 += SCROLL;
            return (false, false);
        }
        TerminalKey::LEFT => {
            app.offset.0 -= SCROLL;
            return (false, false);
        }
        TerminalKey::RIGHT => {
            app.offset.0 += SCROLL;
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

struct AppState {
    size: Winsize,
    file: String,
    document: Document,
    cumulative_heights: Vec<f32>,
    cache: Vec<Page>,
    scale: f32,
    /* Offset is given in page width and page height units */
    offset: (f32, f32),
}

impl AppState {
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

            cumulative_heights
                .push(cumulative_heights.last().unwrap_or(&0.0f32) + height);
            cache.push(page);
        }

        let tmp = cache[0].bounds().unwrap();
        // panic!("aaaa: {} {}", tmp.width(), tmp.height());

        let app: AppState = Self {
            size: winsize,
            file: file,
            document: document,
            cumulative_heights: cumulative_heights,
            cache: cache,
            scale: 0.2,
            offset: (50.0f32, 50.0f32),
        };
        Ok(app)
    }

    fn document_load(&mut self) -> Result<(), String> {
        let mut cumulative_heights: Vec<f32> = Vec::new();
        let mut cache: Vec<Page> = Vec::new();

        let page_count: i32 = self
            .document
            .page_count()
            .map_err(|x| format!("Could not extract the number of pages: {}", x))?;

        for i in 0..page_count {
            let page: Page = self
                .document
                .load_page(i)
                .map_err(|x| format!("Could not load page {}: {}", i, x))?;

            let height: f32 = page
                .bounds()
                .map_err(|x| format!("Could not get bounds for page {}: {}", i, x))?
                .height();

            cumulative_heights
                .push(cumulative_heights.last().unwrap_or(&0.0f32) + height);
            cache.push(page);
        }

        self.cumulative_heights = cumulative_heights;
        self.cache = cache;

        Ok(())
    }

    /* Simple binary search to find page id based on y offset */
    fn page_index(&self, y_offset: f32) -> Result<usize, String> {
        if y_offset < 0.0 {
            Err("Provided y offset is less than 0.0".to_string())?;
        }

        if self.cumulative_heights.len() == 0 {
            Err("No pages are loaded")?;
        }

        let res = self.cumulative_heights.binary_search_by(|x: &f32| {
            x.partial_cmp(&y_offset)
                .expect("NaN value found in cumulative height vector")
        });

        let id: usize = match res {
            Ok(x) => x,
            Err(x) => x - 1,
        };

        Ok(id)
    }
}

#[macro_export]
macro_rules! chan_has {
    ($chan:expr) => {
        $chan.peek().is_some()
    };
}
