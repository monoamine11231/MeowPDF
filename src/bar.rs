use crate::{Config, ConfigBarPosition, Viewer, CONFIG, TERMINAL_SIZE};
use nix::libc::winsize;
use pfmt::{Fmt, FormatTable};
use std::{
    collections::HashMap,
    io::{stdout, StdoutLock, Write},
    sync::RwLockReadGuard,
};

#[derive(Debug)]
pub struct Bar {
    pub mode: BarMode,
}

#[derive(Clone, Debug)]
pub enum BarMode {
    VIEW,
    COMMAND(String),
}

impl Bar {
    pub fn new() -> Self {
        Self {
            mode: BarMode::VIEW,
        }
    }

    pub fn render(&self, viewer: &Viewer) -> Result<(), String> {
        let config: &Config = CONFIG.get().unwrap();
        let terminal_size: RwLockReadGuard<winsize> =
            TERMINAL_SIZE.get().unwrap().read().unwrap();
        let mut handle: StdoutLock = stdout().lock();

        let row: u16 = match config.bar.position {
            ConfigBarPosition::TOP => 0,
            ConfigBarPosition::BOTTOM => terminal_size.ws_row,
        };

        write!(
            handle,
            "\x1B[{};1H{}\x1B[0m",
            row,
            bar_build_string(self, viewer)?
        )
        .unwrap();
        handle.flush().unwrap();
        Ok(())
    }
}

fn bar_build_string(bar: &Bar, viewer: &Viewer) -> Result<String, String> {
    let config: &Config = CONFIG.get().unwrap();
    let terminal_size: RwLockReadGuard<winsize> =
        TERMINAL_SIZE.get().unwrap().read().unwrap();

    let mut table: HashMap<&str, Box<dyn Fmt>> = HashMap::new();
    let mode: &str = match bar.mode {
        BarMode::VIEW => "VIEW   ",
        BarMode::COMMAND(_) => "COMMAND",
    };
    table.insert("mode", Box::new(mode));
    table.insert("file", Box::new(String::from(viewer.file.clone())));
    table.insert("scale", Box::new(format!("{:.0}%", viewer.scale*100.0f32)));

    let mut res: String = String::new();
    res.push_str(
        table
            .format(config.bar.segment_mode.as_str())
            .map_err(|x| format!("Could not build bar: {:?}", x))?
            .as_str(),
    );

    res.push_str(
        table
            .format(config.bar.segment_file.as_str())
            .map_err(|x| format!("Could not build bar: {:?}", x))?
            .as_str(),
    );

    res.push_str(
        table
            .format(config.bar.segment_scale.as_str())
            .map_err(|x| format!("Could not build bar: {:?}", x))?
            .as_str(),
    );

    Ok(res)
}
