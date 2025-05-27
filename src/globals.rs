use crate::{drivers::graphics::GraphicsResponse, Config};
use crossbeam_channel::Receiver;
use crossterm::terminal::WindowSize;
use std::sync::{atomic::AtomicBool, Mutex, OnceLock, RwLock};

pub const CONFIG_FILENAME: &str = "meowpdf";
pub const DEFAULT_CONFIG: &str = r#"
[viewer]
# Determines how fast the document is scrolled
scroll_speed = 20.0
# Determines at what precision the pages are rendered
render_precision = 1.5
# Determines the image data limit that the software holds in RAM (bytes)
memory_limit = 314572800
# Determines the default scale of the viewer when starting the viewer
scale_default = 0.5
# Determines the minimal possible zoom-out of the viewer
scale_min = 0.2
# Determines the scaling amount when zooming in or out
scale_amount = 0.5
# Determines the margin on the bottom of each page
margin_bottom = 10.0
# Determines the amount of pages that are preloaded in advance 
pages_preloaded = 3
"#;

/* Hate on me for those global singletons as much as you want. */
pub static CONFIG: OnceLock<Config> = OnceLock::new();
pub static RECEIVER_GR: OnceLock<Mutex<Receiver<GraphicsResponse>>> = OnceLock::new();
pub static TERMINAL_SIZE: OnceLock<RwLock<WindowSize>> = OnceLock::new();
pub static IMAGE_PADDING: OnceLock<usize> = OnceLock::new();
pub static SOFTWARE_ID: OnceLock<String> = OnceLock::new();
pub static RUNNING: AtomicBool = AtomicBool::new(true);

#[macro_export]
macro_rules! chan_has {
    ($chan:expr) => {
        $chan.peek().is_some()
    };
}

#[macro_export]
macro_rules! clear_channel {
    ($chan:expr) => {
        while let Ok(_) = $chan.try_recv() {}
    };
}

#[macro_export]
macro_rules! has_elapsed {
    ($var:expr) => {{
        let elapsed = $var.elapsed().unwrap().as_millis();
        if elapsed >= 500 {
            $var = SystemTime::now();
        }
        elapsed >= 500
    }};
}
