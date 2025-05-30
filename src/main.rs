mod drivers;
use crossbeam_channel::Receiver;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, window_size, Clear, ClearType,
    EnterAlternateScreen, LeaveAlternateScreen, WindowSize,
};
use drivers::graphics::ClearImages;
use drivers::graphics::{terminal_graphics_test_support, GraphicsResponse};
use keybinds::{KeyInput, Keybinds};

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
use std::io;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::sync::{MutexGuard, RwLock};
use std::time::{Duration, SystemTime};

/* Tracks the last executed times of signals for throattling */
struct LastExecuted {
    pub load: SystemTime,
    pub alpha: SystemTime,
    pub inverse: SystemTime,
}

fn main() {
    /* ============================= Uncook the terminal ============================= */
    enable_raw_mode().expect("Could not cook the terminal");
    execute!(io::stdout(), EnterAlternateScreen).expect("Could not enter alt mode");
    execute!(io::stdout(), Hide).expect("Could not hide cursor");
    execute!(io::stdout(), Clear(ClearType::All)).expect("Could not clear terminal");
    execute!(io::stdout(), EnableMouseCapture).expect("Could not enable mouse capture");

    /* ========================== Cook the terminal on panic ========================= */
    let default_panic: Box<
        dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync + 'static,
    > = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        /* Atleast try to cook the terminal on error before printing the message.
         * Do not handle the error to prevent possible infinite loops when panicking. */

        let _ = execute!(io::stdout(), DisableMouseCapture);
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        let _ = execute!(io::stdout(), Show);
        let _ = disable_raw_mode();
        default_panic(info);
    }));

    /* ============================= STDIN parser thread ============================= */
    let (key_input, graphics_input, winsize_change) = threads::event::spawn();
    RECEIVER_GR.get_or_init(|| Mutex::new(graphics_input));

    /* ========== Check if the terminal supports the Kitty graphics protocol ========= */
    terminal_graphics_test_support()
        .expect("Error when testing terminal support of the Kitty graphics protocol");

    /* ================================= Load config ================================= */
    let mut key_matcher: Keybinds<ConfigAction>;
    {
        let mut config = config_load_or_create().expect("Could not load config");
        key_matcher = config.bindings.unwrap();
        config.bindings = None;
        CONFIG.get_or_init(|| config);
    }

    /* ======================= Calculate padding for all images ====================== */
    let winsize_tmp: WindowSize = window_size().expect("Could not get win size");
    let winsize = WindowSize {
        rows: winsize_tmp.rows,
        columns: winsize_tmp.columns,
        width: winsize_tmp.width,
        height: winsize_tmp.height,
    };
    TERMINAL_SIZE.get_or_init(|| RwLock::new(winsize_tmp));

    let config: &Config = CONFIG.get().unwrap();

    let pxpercol: f64 = winsize.width as f64 / winsize.columns as f64;
    let pxperrow: f64 = winsize.height as f64 / winsize.rows as f64;

    let paddingcol: usize = (pxpercol * config.viewer.render_precision
        / config.viewer.scale_min as f64)
        .ceil() as usize;
    let paddingrow: usize = (pxperrow * config.viewer.render_precision
        / config.viewer.scale_min as f64)
        .ceil() as usize;

    IMAGE_PADDING.get_or_init(|| std::cmp::max(paddingcol, paddingrow));

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
                        widths
                    } => {
                        let uninit = viewer.is_uninit();

                        viewer.update_metadata(max_page_width, &cumulative_heights, &widths);
                        viewer.invalidate_registry();
                        viewer.center_viewer();
                        if uninit {
                            viewer.scale_page2terminal();
                        }
                        result_receiver.clear_priority(1);
                    }
                    threads::renderer::RendererResult::Image { page, data } => {
                        viewer.handle_image(page, data);
                    }
                }
            }
            2 => {
                file_reload
                    .try_recv()
                    .expect("Could not receive file reload");
                if throttle_data.load.elapsed().unwrap() >= Duration::from_millis(1000) {
                    renderer
                        .send_and_confirm_action(threads::renderer::RendererAction::Load)
                        .expect("Cannot send action to renderer thread");
                }
            }
            3 => {
                sender_rerender
                    .try_recv()
                    .expect("Could not receive rerender");
            }
            4 => {
                let key = key_input.try_recv().expect("Could not receive key");
                if handle_key(
                    key,
                    &mut key_matcher,
                    &mut viewer,
                    &renderer,
                    &mut throttle_data,
                ) {
                    break 'main;
                }
            }
            5 => {
                let (width, height) = winsize_change
                    .try_recv()
                    .expect("Could not receive from win-size");

                let mut handle = TERMINAL_SIZE
                    .get()
                    .unwrap()
                    .write()
                    .expect("Could not get win sie handle");
                handle.width = width;
                handle.height = height;
            }
            _ => unreachable!(),
        };

        let gr: MutexGuard<Receiver<GraphicsResponse>> =
            RECEIVER_GR.get().unwrap().lock().unwrap();

        execute!(io::stdout(), ClearImages).expect("Could not clear images");

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
    execute!(io::stdout(), DisableMouseCapture).expect("Could not disable mouse capture");
    execute!(io::stdout(), LeaveAlternateScreen).expect("Could not leave alt mode");
    execute!(io::stdout(), Show).expect("Could not show cursor");
    disable_raw_mode().expect("Could not uncook the terminal");
}

fn handle_key(
    key: KeyEvent,
    key_matcher: &mut Keybinds<ConfigAction>,
    viewer: &mut Viewer,
    renderer: &threads::renderer::Renderer,
    throttle_data: &mut LastExecuted,
) -> bool {
    let config: &Config = CONFIG.get().unwrap();

    let possible_action = key_matcher.dispatch(KeyInput::from(key));
    if possible_action.is_none() {
        return match key {
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                viewer.scroll((0.0f32, config.viewer.scroll_speed));
                false
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                viewer.scroll((0.0f32, -config.viewer.scroll_speed));
                false
            }
            KeyEvent {
                code: KeyCode::Left,
                ..
            } => {
                viewer.scroll((-config.viewer.scroll_speed, 0.0f32));
                false
            }
            KeyEvent {
                code: KeyCode::Right,
                ..
            } => {
                viewer.scroll((config.viewer.scroll_speed, 0.0f32));
                false
            }
            _ => false,
        };
    }

    let action = possible_action.unwrap();

    /* `true` indicates that the caller should exit *safely* the current process */
    let res: bool = match action {
        ConfigAction::CenterViewer => {
            viewer.center_viewer();
            false
        }
        ConfigAction::JumpFirstPage => {
            let _ = viewer.jump(0);
            false
        }
        ConfigAction::JumpLastPage => {
            let last_page: usize = viewer.pages() - 1;
            let _ = viewer.jump(last_page);
            false
        }
        ConfigAction::Quit => true,
        ConfigAction::ToggleAlpha => {
            if throttle_data.alpha.elapsed().unwrap() < Duration::from_millis(500) {
                return false;
            }

            throttle_data.alpha = SystemTime::now();

            renderer
                .send_and_confirm_action(threads::renderer::RendererAction::ToggleAlpha)
                .expect("Could not send action to renderer");
            viewer.invalidate_registry();
            false
        }
        ConfigAction::ToggleInverse => {
            if throttle_data.inverse.elapsed().unwrap() < Duration::from_millis(500) {
                return false;
            }

            throttle_data.inverse = SystemTime::now();
            renderer
                .send_and_confirm_action(threads::renderer::RendererAction::ToggleInverse)
                .expect("Could not send action to renderer");
            viewer.invalidate_registry();
            false
        }
        ConfigAction::ZoomIn => {
            viewer.scale(config.viewer.scale_amount);
            false
        }
        ConfigAction::ZoomOut => {
            viewer.scale(-config.viewer.scale_amount);
            false
        }
    };
    res
}
