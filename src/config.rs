use std::{
    fs::File,
    io::{Read, Write},
};

use crossterm::style::Color;
use dirs::config_dir;
use keybinds::Keybinds;
use serde::Deserialize;

use crate::{CONFIG_FILENAME, DEFAULT_CONFIG};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub viewer: ConfigViewer,
    pub bindings: Option<Keybinds<ConfigAction>>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigViewer {
    pub scroll_speed: f32,
    pub render_precision: f64,
    pub memory_limit: usize,
    pub scale_min: f32,
    pub scale_amount: f32,
    pub margin_bottom: f32,
    pub pages_preloaded: usize,
    pub inverse_scroll: bool,

    pub uri_hint: ConfigViewerUriHint,
}

#[derive(Debug, Deserialize)]
pub struct ConfigViewerUriHint {
    pub enabled: bool,
    pub background: Color,
    pub foreground: Color,
    pub width: f32,
}

#[derive(Debug, Deserialize)]
pub enum ConfigAction {
    ToggleAlpha,
    ToggleInverse,
    CenterViewer,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    ZoomIn,
    ZoomOut,
    JumpFirstPage,
    JumpLastPage,
    PrevPage,
    NextPage,
    Quit,
}

pub fn config_load_or_create() -> Result<Config, String> {
    let mut config =
        config_dir().ok_or("Incompatible OS: No config directory has been found")?;
    config.push(CONFIG_FILENAME);

    let mut config_content = String::new();
    if !config.as_path().exists() {
        let mut config = File::create(config.as_path())
            .map_err(|x| format!("Could not create config file: {}", x))?;
        config
            .write(DEFAULT_CONFIG.as_bytes())
            .map_err(|x| format!("Could not write to config file: {}", x))?;
        config_content.push_str(DEFAULT_CONFIG);
    } else {
        let mut config = File::open(config.as_path())
            .map_err(|x| format!("Could not open config file: {}", x))?;

        config
            .read_to_string(&mut config_content)
            .map_err(|x| format!("Could not read config file: {}", x))?;
    }

    let config_parsed: Config = toml::from_str(config_content.as_str())
        .map_err(|x| format!("Could not parse config file: {}", x))?;

    /* ========================== Check constant constraints ========================= */
    if config_parsed.viewer.render_precision <= 0.0f64 {
        return Err(
            "`config.viewer.render_precision` can not be negative or equal to 0!"
                .to_string(),
        );
    }

    if config_parsed.viewer.scale_min <= 0.0f32 {
        return Err(
            "`config.viewer.scale_min` can not be negative or equal to 0!".to_string(),
        );
    }

    if config_parsed.viewer.margin_bottom < 0.0f32 {
        return Err("`config.viewer.margin_bottom` can not be negative!".to_string());
    }

    if config_parsed.bindings.is_none() {
        return Err("`config.bindings` can not be empty!".to_string());
    }

    Ok(config_parsed)
}
