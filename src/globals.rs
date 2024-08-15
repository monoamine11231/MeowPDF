use crate::{drivers::input::GraphicsResponse, Config};
use crossbeam_channel::Receiver;
use nix::pty::Winsize;
use std::sync::{Mutex, OnceLock, RwLock};

pub const CONFIG_FILENAME: &'static str = "meowpdf";
pub const DEFAULT_CONFIG: &'static str = r#"
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
[bar]
# Determines the position of the bar. Valid values are `top` and `bottom`
position = "bottom"
#segment_mode = "\u001B[48;2;0;0;255m {mode} \u001B[38;2;0;0;255m\u001B[48;2;0;255;0m\uE0B0"
#segment_file = "\u001B[38;2;255;255;255m\u001B[48;2;0;255;0m {file} \u001B[38;2;0;255;0m\u001B[48;2;255;0;0m\uE0B0"
#segment_scale = "\u001B[38;2;255;255;255m {page}"
"#;

/* Hate on me for those global singletons as much as you want. */
pub static CONFIG: OnceLock<Config> = OnceLock::new();
pub static RECEIVER_GR: OnceLock<Mutex<Receiver<GraphicsResponse>>> = OnceLock::new();
pub static TERMINAL_SIZE: OnceLock<RwLock<Winsize>> = OnceLock::new();
pub static IMAGE_PADDING: OnceLock<usize> = OnceLock::new();
pub static SOFTWARE_ID: OnceLock<String> = OnceLock::new();

#[macro_export]
macro_rules! chan_has {
    ($chan:expr) => {
        $chan.peek().is_some()
    };
}

#[macro_export]
macro_rules! clear_channel {
    ($chan:ident) => {
        while let Ok(_) = $chan.try_recv() {}
    };
}
