mod drivers;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use drivers::input::{GraphicsResponse, StdinDFA, StdinInput, TerminalKey};
use drivers::{graphics::terminal_graphics_test_support, tui::*};
use nix::pty::Winsize;

mod image;
use crate::image::*;

mod viewer;
use crate::viewer::*;

mod globals;
use crate::globals::*;

mod config;
use crate::config::*;

mod bar;
use crate::bar::*;

use std::hash::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::stdin;
use std::io::Read;
use std::io::Stdin;
use std::ops::Deref;
use std::path::Path;
use std::sync::{MutexGuard, RwLock, RwLockWriteGuard};
use std::time::Duration;
use std::{sync::Mutex, thread};

use nix::sys::termios::Termios;

use notify::RecursiveMode;
use notify::{
    event::{DataChange, ModifyKind},
    Watcher,
};

fn main() {
    /* ============================= Uncook the terminal ============================= */
    let tty_data_original_main: Termios =
        terminal_control_raw_mode().expect("Error when setting terminal to raw mode");
    let tty_data_original_panic_hook: Mutex<Termios> =
        Mutex::from(tty_data_original_main.clone());

    terminal_tui_clear();

    /* ========================== Cook the terminal on panic ========================= */
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let tty = tty_data_original_panic_hook.lock().unwrap();
        /* Atleast try to cook the terminal on error before printing the message.
         * Do not handle the error to prevent possible infinite loops when panicking. */
        let _ = terminal_control_default_mode(tty.deref());
        default_panic(info);
    }));

    /* ============================= STDIN parser thread ============================= */
    let (sender_key, receive_key) = unbounded::<TerminalKey>();
    let (sender_gr, receive_gr) = unbounded::<GraphicsResponse>();

    RECEIVER_GR.get_or_init(|| Mutex::new(receive_gr));
    thread::spawn(move || {
        let mut dfa: StdinDFA = StdinDFA::new();
        let mut handle: Stdin = stdin();
        let mut b: [u8; 1] = [0u8];
        loop {
            handle.read(&mut b).unwrap();
            let token: Option<StdinInput> = dfa.feed(b[0]);

            if token.is_none() {
                continue;
            }
            match token.unwrap() {
                StdinInput::TerminalKey(x) => {
                    /* Ignore send errors since they occur only when trying to
                     * close the application */
                    let _ = sender_key.send(x);
                }
                StdinInput::GraphicsResponse(x) => {
                    /* Ignore send errors since they occur only when trying to
                     * close the application */
                    let _ = sender_gr.send(x);
                }
                _ => (),
            }
        }
    });

    /* ========== Check if the terminal supports the Kitty graphics protocol ========= */
    terminal_graphics_test_support()
        .expect("Error when testing terminal support of the Kitty graphics protocol");

    /* ================================= Load config ================================= */
    CONFIG.get_or_init(|| config_load_or_create().expect("Could not load config"));

    /* ======================= Calculate padding for all images ====================== */
    let winsize: Winsize =
        terminal_tui_get_dimensions().expect("Could not get terminal dimensions: {}");

    if winsize.ws_xpixel == 0 || winsize.ws_ypixel == 0 {
        panic!("Could not get terminal dimensions: Invalid results from IOCTL");
    }
    TERMINAL_SIZE.get_or_init(|| RwLock::new(winsize));
    let config: &Config = CONFIG.get().unwrap();

    let pxpercol: f64 = winsize.ws_xpixel as f64 / winsize.ws_col as f64;
    let pxperrow: f64 = winsize.ws_ypixel as f64 / winsize.ws_row as f64;

    let paddingcol: usize = (pxpercol * config.viewer.render_precision
        / config.viewer.scale_min as f64)
        .ceil() as usize;
    let paddingrow: usize = (pxperrow * config.viewer.render_precision
        / config.viewer.scale_min as f64)
        .ceil() as usize;

    IMAGE_PADDING.get_or_init(|| std::cmp::max(paddingcol, paddingrow));

    /* ==================== Thread notifying terminal size change ==================== */
    let (sender_winsize, receive_winsize) = unbounded::<()>();
    thread::spawn(move || {
        let mut wz: Winsize = terminal_tui_get_dimensions().unwrap();
        loop {
            thread::sleep(Duration::from_millis(100));

            let tmp: Winsize = terminal_tui_get_dimensions().unwrap();
            if tmp == wz {
                continue;
            }

            wz = tmp;
            /* Notify that the terminal window has been changed */
            sender_winsize
                .send(())
                .expect("Could not send a terminal window size change signal");

            /* Change terminal window size global variable */
            let mut terminal_size_lock: RwLockWriteGuard<Winsize> =
                TERMINAL_SIZE.get().unwrap().write().unwrap();
            *terminal_size_lock = wz;
        }
    });

    /* =========== Generate a random ID which is unique for every instance =========== */
    let random_u64: u64 = RandomState::new().build_hasher().finish();
    SOFTWARE_ID.get_or_init(|| format!("{random_u64:X}"));

    /* ====================== Viewer - The core of this program ====================== */
    let file: String = std::env::args().nth(1).expect("No provided pdf!");
    let mut viewer: Viewer = Viewer::new(&file).expect("Could not initialize the viewer");
    viewer.run().unwrap();

    /* ========================= Thread notifying file change ======================== */
    let sender_reload_init: Sender<()> = viewer.reload_initiator.clone().unwrap();
    let mut watcher_file =
        notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let event: notify::Event =
                res.expect("Could not watch file changes for the given file");

            match event.kind {
                notify::EventKind::Modify(ModifyKind::Data(DataChange::Any)) => {
                    sender_reload_init
                        .send(())
                        .expect("Could not send a file change signal");
                }
                _ => (),
            }
        })
        .expect("Could not initialize a file watcher for the given file");

    watcher_file
        .watch(Path::new(file.as_str()), RecursiveMode::NonRecursive)
        .expect("Could not start watching file changes for the given file");

    let bar: Bar = Bar::new();

    /* ============================== Main program loop ============================== */
    'main: loop {
        macro_rules! key_processor {
            ($key:ident) => {
                let exit: bool = handle_key($key, &mut viewer);
                if exit {
                    break 'main;
                }
            };
        }

        select! {
            recv(receive_key) -> key => {
                let key_unwrapped: TerminalKey = key.unwrap();
                key_processor!(key_unwrapped);
            },
            recv(receive_winsize) -> _ => (),
            recv(viewer.rerender_instructor.clone().unwrap()) -> _ => (),
            recv(viewer.reload_informer_global.clone().unwrap()) -> _ => (),
        }

        /* Since displaying pages is a bit slow, handle all the key events
         * that were produced in the meantime. This creates an illusion that
         * no delay exists (ish) */
        while let Ok(key) = receive_key.try_recv() {
            key_processor!(key);
        }

        let gr: MutexGuard<Receiver<GraphicsResponse>> =
            RECEIVER_GR.get().unwrap().lock().unwrap();

        terminal_tui_clear();
        bar.render(&viewer).unwrap();

        let displayed: Vec<usize> =
            viewer.display_pages().expect("Could not display pages");
        for page in displayed {
            let res: GraphicsResponse = gr.recv().unwrap();
            if res.payload().contains("OK") {
                continue;
            }

            viewer.schedule_transfer(page);
        }
    }

    /* ========================== Cook the terminal on exit ========================== */
    terminal_control_default_mode(&tty_data_original_main)
        .expect("Error when setting terminal to default mode");
}

fn handle_key(key: TerminalKey, viewer: &mut Viewer) -> bool {
    /* `true` indicates that the caller should exit *safely* the current process */
    let config: &Config = CONFIG.get().unwrap();
    let mut offset_lock: RwLockWriteGuard<ViewerOffset> = viewer.offset.write().unwrap();
    let res: bool = match key {
        TerminalKey::CTRLC
        | TerminalKey::CTRLD
        | TerminalKey::OTHER(b'q')
        | TerminalKey::OTHER(b'Q') => true,
        TerminalKey::UP => {
            offset_lock.scroll((0.0f32, config.viewer.scroll_speed));
            return false;
        }
        TerminalKey::DOWN => {
            offset_lock.scroll((0.0f32, -config.viewer.scroll_speed));
            return false;
        }
        TerminalKey::LEFT => {
            offset_lock.scroll((-config.viewer.scroll_speed, 0.0f32));
            return false;
        }
        TerminalKey::RIGHT => {
            offset_lock.scroll((config.viewer.scroll_speed, 0.0f32));
            return false;
        }
        TerminalKey::OTHER(b'+') => {
            offset_lock.scale(config.viewer.scale_amount);
            return false;
        }
        TerminalKey::OTHER(b'-') => {
            offset_lock.scale(-config.viewer.scale_amount);
            return false;
        }
        _ => false,
    };
    return res;
}
