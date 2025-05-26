mod drivers;
use crossbeam_channel::Receiver;
use drivers::input::{GraphicsResponse, TerminalKey};
use drivers::{graphics::terminal_graphics_test_support, tui::*};
use nix::pty::Winsize;

mod threads;

mod image;
use crate::image::*;

mod viewer;
use crate::viewer::*;

mod globals;
use crate::globals::*;

mod config;
use crate::config::*;

use std::hash::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::ops::Deref;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::sync::{MutexGuard, RwLock};
use std::time::{Duration, SystemTime};

use nix::sys::termios::Termios;

/* Tracks the last executed times of signals for throattling */
struct LastExecuted {
    pub load: SystemTime,
    pub alpha: SystemTime,
    pub inverse: SystemTime,
}

fn main() {
    /* ============================= Uncook the terminal ============================= */
    let tty_data_original_main: Termios =
        terminal_control_raw_mode().expect("Error when setting terminal to raw mode");
    let tty_data_original_panic_hook: Mutex<Termios> =
        Mutex::from(tty_data_original_main.clone());

    terminal_tui_clear();

    /* ========================== Cook the terminal on panic ========================= */
    let default_panic: Box<
        dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync + 'static,
    > = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let tty = tty_data_original_panic_hook.lock().unwrap();
        /* Atleast try to cook the terminal on error before printing the message.
         * Do not handle the error to prevent possible infinite loops when panicking. */
        let _ = terminal_control_default_mode(tty.deref());
        default_panic(info);
    }));

    /* ============================= STDIN parser thread ============================= */
    let (key_input, graphics_input) = threads::stdin::spawn();
    RECEIVER_GR.get_or_init(|| Mutex::new(graphics_input));

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
    let winsize_change = threads::winsize::spawn();

    /* =========== Generate a random ID which is unique for every instance =========== */
    let random_u64: u64 = RandomState::new().build_hasher().finish();
    SOFTWARE_ID.get_or_init(|| format!("{random_u64:X}"));

    /* ====================== Viewer - The core of this program ====================== */
    let file: String = std::env::args().nth(1).expect("No provided pdf!");
    let (mut viewer, sender_rerender) = Viewer::new();

    let (mut renderer, result_receiver) = threads::renderer::Renderer::new();
    renderer.run(&file).expect("Couldn't start renderer thread");
    renderer
        .send_and_confirm_action(threads::renderer::RendererAction::Load)
        .expect("Cannot send action to renderer thread");

    /* ========================= Thread notifying file change ======================== */
    let file_reload: Receiver<()> =
        threads::fnotify::spawn(&file).expect("Could not init file watcher");

    /* ============================== Main program loop ============================== */
    let mut throttle_data = LastExecuted {
        load: SystemTime::now() - Duration::from_millis(500),
        alpha: SystemTime::now() - Duration::from_millis(500),
        inverse: SystemTime::now() - Duration::from_millis(500),
    };

    'main: loop {
        macro_rules! key_processor {
            ($key:ident) => {
                let exit: bool =
                    handle_key($key, &mut viewer, &renderer, &mut throttle_data);
                if exit {
                    break 'main;
                }
            };
        }

        /* sel[0..1] are the results from the renderer thread */
        let mut sel = result_receiver.construct_select();
        sel.recv(&file_reload);
        sel.recv(&sender_rerender);
        sel.recv(&key_input);
        sel.recv(&winsize_change);

        let index_ready = sel.ready();
        match index_ready {
            0 | 1 => {
                let result = result_receiver
                    .try_recv_priority(index_ready)
                    .expect("Could not receive priority");

                match result {
                    threads::renderer::RendererResult::PageMetadata {
                        max_page_width,
                        cumulative_heights,
                    } => {
                        viewer.update_metadata(max_page_width, &cumulative_heights);
                        viewer.invalidate_registry();
                        viewer.center_viewer();
                        result_receiver.clear_priority(1);
                    }
                    threads::renderer::RendererResult::Image { page, data } => {
                        viewer.handle_image(page, data);
                    }
                }
            }
            2 => {
                let _ = file_reload
                    .try_recv()
                    .expect("Could not receive file reload");
                if throttle_data.load.elapsed().unwrap() >= Duration::from_millis(1000) {
                    renderer
                        .send_and_confirm_action(threads::renderer::RendererAction::Load)
                        .expect("Cannot send action to renderer thread");
                }
            }
            3 => {
                let _ = sender_rerender
                    .try_recv()
                    .expect("Could not receive rerender");
            }
            4 => {
                let key = key_input.try_recv().expect("Could not receive key");
                key_processor!(key);
            }
            5 => {
                let _ = winsize_change
                    .try_recv()
                    .expect("Could not receive from win-size");
            }
            _ => unreachable!(),
        };

        /* Since displaying pages is a bit slow, handle all the key events
         * that were produced in the meantime. This creates an illusion that
         * no delay exists (ish) */
        // while let Ok(key) = key_input.try_recv() {
        //     key_processor!(key);
        // }

        let gr: MutexGuard<Receiver<GraphicsResponse>> =
            RECEIVER_GR.get().unwrap().lock().unwrap();

        terminal_tui_clear();

        let displayed: Vec<usize> = viewer
            .display_pages(&renderer)
            .expect("Could not display pages");
        for page in displayed {
            let res: GraphicsResponse = gr.recv().unwrap();
            if res.payload().contains("OK") {
                continue;
            }

            viewer.schedule_transfer(page);
        }
    }

    RUNNING.store(false, Ordering::Release);

    /* ========================== Cook the terminal on exit ========================== */
    terminal_control_default_mode(&tty_data_original_main)
        .expect("Error when setting terminal to default mode");
}

fn handle_key(
    key: TerminalKey,
    viewer: &mut Viewer,
    renderer: &threads::renderer::Renderer,
    throttle_data: &mut LastExecuted,
) -> bool {
    /* `true` indicates that the caller should exit *safely* the current process */
    let config: &Config = CONFIG.get().unwrap();
    let res: bool = match key {
        TerminalKey::CTRLC
        | TerminalKey::CTRLD
        | TerminalKey::OTHER(b'q')
        | TerminalKey::OTHER(b'Q') => true,
        TerminalKey::UP => {
            viewer.scroll((0.0f32, config.viewer.scroll_speed));
            return false;
        }
        TerminalKey::DOWN => {
            viewer.scroll((0.0f32, -config.viewer.scroll_speed));
            return false;
        }
        TerminalKey::LEFT => {
            viewer.scroll((-config.viewer.scroll_speed, 0.0f32));
            return false;
        }
        TerminalKey::RIGHT => {
            viewer.scroll((config.viewer.scroll_speed, 0.0f32));
            return false;
        }
        TerminalKey::OTHER(b'+') => {
            viewer.scale(config.viewer.scale_amount);
            return false;
        }
        TerminalKey::OTHER(b'-') => {
            viewer.scale(-config.viewer.scale_amount);
            return false;
        }
        TerminalKey::OTHER(b'a') | TerminalKey::OTHER(b'A') => {
            if throttle_data.alpha.elapsed().unwrap() < Duration::from_millis(500) {
                return false;
            }

            throttle_data.alpha = SystemTime::now();

            renderer
                .send_and_confirm_action(threads::renderer::RendererAction::ToggleAlpha)
                .expect("Could not send action to renderer");
            viewer.invalidate_registry();
            return false;
        }
        TerminalKey::OTHER(b'i') | TerminalKey::OTHER(b'I') => {
            if throttle_data.inverse.elapsed().unwrap() < Duration::from_millis(500) {
                return false;
            }

            throttle_data.inverse = SystemTime::now();
            renderer
                .send_and_confirm_action(threads::renderer::RendererAction::ToggleInverse)
                .expect("Could not send action to renderer");
            viewer.invalidate_registry();
            return false;
        }
        TerminalKey::OTHER(b'c') | TerminalKey::OTHER(b'C') => {
            viewer.center_viewer();
            return false;
        }
        TerminalKey::OTHER(b'G') => {
            let last_page: usize = viewer.pages() - 1;
            let _ = viewer.jump(last_page);
            return false;
        }
        _ => false,
    };
    return res;
}
