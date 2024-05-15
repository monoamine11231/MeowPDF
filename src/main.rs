mod drivers;
use drivers::{graphics::terminal_graphics_test_support, tui::*};

mod image;
use crate::image::*;

mod viewer;
use crate::viewer::*;

use std::ops::Deref;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::{io::stdout, io::Write, sync::Mutex, thread};

use mupdf::Colorspace;
use mupdf::Matrix;
use mupdf::Pixmap;
use nix::libc;
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
    let mut viewer: Viewer = Viewer::init(file).expect("Could not initialize the viewer");

    let mut ctm: Matrix = Matrix::new_scale(PRECISION as f32, PRECISION as f32);
    let cs: Colorspace = Colorspace::device_rgb();

    let pxpercol: f64 = viewer.size.ws_xpixel as f64 / viewer.size.ws_col as f64;
    let pxperrow: f64 = viewer.size.ws_ypixel as f64 / viewer.size.ws_row as f64;

    let paddingcol: usize = (pxpercol * PRECISION / SCALE_MIN as f64).ceil() as usize;
    let paddingrow: usize = (pxperrow * PRECISION / SCALE_MIN as f64).ceil() as usize;

    let padding: usize = std::cmp::max(paddingcol, paddingrow);
    for i in 0..5 {
        let pm: Pixmap = viewer.cache[i].to_pixmap(&ctm, &cs, 0.0, true).unwrap();
        viewer.images
            .push(Image::new(&pm, PRECISION, padding).unwrap());
    }

    let mut key_iter = terminal_tui_key_iter().peekable();
    let mut winsize_iter = receive_winsize.try_iter().peekable();
    let mut file_iter = receive_file.try_iter().peekable();

    loop {
        /* Break the loop if a CTRL-C or similar sigint signal was sent */
        if chan_has!(key_iter) {
            let key: TerminalKey = **key_iter.peek().as_ref().unwrap();
            let event: (bool, bool) = handle_key(key, &mut viewer);
            if event.0 {
                break;
            }

            key_iter.next();
        }

        /* Rerender on terminal window size change */
        if chan_has!(winsize_iter) {
            viewer.size = *winsize_iter.peek().unwrap();
            winsize_iter.next();
        }

        /* Do something on file change */
        if chan_has!(file_iter) {
            file_iter.next();
        }

        terminal_tui_clear().expect("Error when clearing the screen");

        viewer.display_pages().expect("Could not display pages");

        print!(
            "\x1B[{};1H\x1B[1;31mPAGE {} | SCALE {}",
            viewer.size.ws_row,
            viewer.offset.page(),
            viewer.scale
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
fn handle_key(key: TerminalKey, viewer: &mut Viewer) -> (bool, bool) {
    let res: (bool, bool) = match key {
        TerminalKey::CTRLC
        | TerminalKey::CTRLD
        | TerminalKey::OTHER(b'q')
        | TerminalKey::OTHER(b'Q') => (true, false),
        TerminalKey::UP => {
            viewer.offset.scroll((0.0f32, -SCROLL));
            return (false, false);
        }
        TerminalKey::DOWN => {
            viewer.offset.scroll((0.0f32, SCROLL));
            return (false, false);
        }
        TerminalKey::LEFT => {
            viewer.offset.scroll((-SCROLL, 0.0f32));
            return (false, false);
        }
        TerminalKey::RIGHT => {
            viewer.offset.scroll((SCROLL, 0.0f32));
            return (false, false);
        }
        TerminalKey::OTHER(b'+') => {
            viewer.scale += SCALE;
            return (false, false);
        }
        TerminalKey::OTHER(b'-') => {
            viewer.scale -= SCALE;
            if viewer.scale < SCALE_MIN {
                viewer.scale = SCALE_MIN;
            }
            return (false, false);
        }
        _ => (false, false),
    };
    return res;
}

#[macro_export]
macro_rules! chan_has {
    ($chan:expr) => {
        $chan.peek().is_some()
    };
}
