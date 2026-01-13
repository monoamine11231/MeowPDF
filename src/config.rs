use std::{collections::HashSet, mem::discriminant};

use toml::{Table, Value};

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

/* Remove old config variables, add new defaults if not existant */
fn fix_config_toml(current: &mut toml::Table, default: &toml::Table) -> bool {
    let mut config_has_changed = false;

    let keys: HashSet<String> = current.keys().chain(default.keys()).cloned().collect();
    for key in keys {
        let key_in_current = current.contains_key(&key);
        let key_in_default = default.contains_key(&key);

        if key_in_current && !key_in_default {
            /* Remove old config variable that has been removed in new version */
            current.remove(&key);
            config_has_changed = true;
        } else if !key_in_current && key_in_default {
            /* Add new config variable that has been added in new version */
            current.insert(key.clone(), default[&key].clone());
            config_has_changed = true;
        } else if discriminant(&current[&key]) != discriminant(&default[&key]) {
            /* If different variants, infer the variant from the default */
            current[&key] = default[&key].clone();
            config_has_changed = true;
        } else if let (Value::Table(current_rec), Value::Table(default_rec)) =
            (&mut current[&key], &default[&key])
        {
            /* If both were tables, check recursively */
            config_has_changed |= fix_config_toml(current_rec, default_rec);
        }
    }

    config_has_changed
}

pub fn config_load_or_create() -> Result<Config, String> {
    let mut config =
        config_dir().ok_or("Incompatible OS: No config directory has been found")?;
    config.push(CONFIG_FILENAME);

    let mut config_content = String::new();
    if !config.as_path().exists() {
        std::fs::write(config.as_path(), DEFAULT_CONFIG.as_bytes())
            .map_err(|x| format!("Could not create and write config file: {}", x))?;
        config_content.push_str(DEFAULT_CONFIG);
    } else {
        config_content = std::fs::read_to_string(config.as_path())
            .map_err(|x| format!("Could not open and read config file: {}", x))?;

        /* Remove old config variables, add new defaults if not existant */
        let mut current_config_toml = config_content
            .parse::<Table>()
            .map_err(|x| format!("Could not parse config contents as TOML: {}", x))?;
        let default_config_toml = DEFAULT_CONFIG.parse::<Table>().map_err(|x| {
            format!("Could not parse default config contents as TOML: {}", x)
        })?;

        /* Check if the current config has been fixed, and if so just rewrite the old one */
        if fix_config_toml(&mut current_config_toml, &default_config_toml) {
            let fixed_config_content = toml::to_string_pretty(&current_config_toml)
                .map_err(|x| format!("Could not serialize toml to string: {}", x))?;

            std::fs::write(config.as_path(), fixed_config_content.as_bytes()).map_err(
                |x| {
                    format!(
                        "Could not open and write fixed config to config file: {}",
                        x
                    )
                },
            )?;

            config_content = fixed_config_content;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_config_no_change() {
        const REFERENCE_CONFIG: &str = r#"
		[viewer]
		# Determines how fast the document is scrolled
		scroll_speed = 20.0
		# Determines at what precision the pages are rendered
		render_precision = 1.5
		# Determines the image data limit that the software holds in RAM (bytes)
		memory_limit = 314572800
		# Minimum scale amount allowed
		scale_min = 0.2
		# Determines the default scale of the viewer when starting the viewer
		scale_default = 0.5
		# Determines the scaling amount when zooming in or out
		scale_amount = 0.5
		# Determines the margin on the bottom of each page
		margin_bottom = 10.0
		# Determines the amount of pages that are preloaded in advance 
		pages_preloaded = 3
		# Inverse vertical scroll
		inverse_scroll = false
		
		[viewer.uri_hint]
		# Enabled URI hints
		enabled = true
		# Background color of hint bar
		background = "blue"
		# Foreground color of hint bar text
		foreground = "white"
		# Hint bar width percentage based on terminal width
		width = 0.2 
		
		[bindings]
		"Ctrl+a" = "ToggleAlpha"
		"Ctrl+o" = "ToggleInverse"
		"C" = "CenterViewer"
		"h" = "MoveLeft"
		"j" = "MoveDown"
		"k" = "MoveUp"
		"l" = "MoveRight"
		"Up" = "MoveUp"
		"Left" = "MoveLeft"
		"Right" = "MoveRight"
		"Down" = "MoveDown"
		"Plus" = "ZoomIn"
		"-" = "ZoomOut"
		"g g" = "JumpFirstPage"
		"G" = "JumpLastPage"
		"PageUp" = "PrevPage"
		"PageDown" = "NextPage"
		"Ctrl+b" = "PrevPage"
		"Ctrl+f" = "NextPage"
		"q" = "Quit"
		"Q" = "Quit"
		"#;

        const TEST_CONFIG: &str = r#"
		[viewer]
		# Determines how fast the document is scrolled
		scroll_speed = 20.0
		# Determines at what precision the pages are rendered
		render_precision = 1.5
		# Determines the image data limit that the software holds in RAM (bytes)
		memory_limit = 314572800
		# Minimum scale amount allowed
		scale_min = 0.2
		# Determines the default scale of the viewer when starting the viewer
		scale_default = 0.5
		# Determines the scaling amount when zooming in or out
		scale_amount = 0.5
		# Determines the margin on the bottom of each page
		margin_bottom = 10.0
		# Determines the amount of pages that are preloaded in advance 
		pages_preloaded = 3
		# Inverse vertical scroll
		inverse_scroll = false
		
		[viewer.uri_hint]
		# Enabled URI hints
		enabled = true
		# Background color of hint bar
		background = "blue"
		# Foreground color of hint bar text
		foreground = "white"
		# Hint bar width percentage based on terminal width
		width = 0.2 
		
		[bindings]
		"Ctrl+a" = "ToggleAlpha"
		"Ctrl+o" = "ToggleInverse"
		"C" = "CenterViewer"
		"h" = "MoveLeft"
		"j" = "MoveDown"
		"k" = "MoveUp"
		"l" = "MoveRight"
		"Up" = "MoveUp"
		"Left" = "MoveLeft"
		"Right" = "MoveRight"
		"Down" = "MoveDown"
		"Plus" = "ZoomIn"
		"-" = "ZoomOut"
		"g g" = "JumpFirstPage"
		"G" = "JumpLastPage"
		"PageUp" = "PrevPage"
		"PageDown" = "NextPage"
		"Ctrl+b" = "PrevPage"
		"Ctrl+f" = "NextPage"
		"q" = "Quit"
		"Q" = "Quit"
		"#;

        let mut test_config_toml = TEST_CONFIG.parse::<Table>().unwrap();
        let reference_config_toml = REFERENCE_CONFIG.parse::<Table>().unwrap();

        assert!(test_config_toml == reference_config_toml);
        assert!(fix_config_toml(&mut test_config_toml, &reference_config_toml) == false);
        assert!(test_config_toml == reference_config_toml);
    }

    #[test]
    fn test_fix_config_removal_of_non_existent_variables() {
        const REFERENCE_CONFIG: &str = r#"
		[viewer]
		# Determines how fast the document is scrolled
		scroll_speed = 20.0
		# Determines at what precision the pages are rendered
		render_precision = 1.5
		# Determines the image data limit that the software holds in RAM (bytes)
		memory_limit = 314572800
		# Minimum scale amount allowed
		scale_min = 0.2
		# Determines the default scale of the viewer when starting the viewer
		scale_default = 0.5
		# Determines the scaling amount when zooming in or out
		scale_amount = 0.5
		# Determines the margin on the bottom of each page
		margin_bottom = 10.0
		# Determines the amount of pages that are preloaded in advance 
		pages_preloaded = 3
		# Inverse vertical scroll
		inverse_scroll = false
		
		[viewer.uri_hint]
		# Enabled URI hints
		enabled = true
		# Background color of hint bar
		background = "blue"
		# Foreground color of hint bar text
		foreground = "white"
		# Hint bar width percentage based on terminal width
		width = 0.2 
		
		[bindings]
		"Ctrl+a" = "ToggleAlpha"
		"Ctrl+o" = "ToggleInverse"
		"C" = "CenterViewer"
		"h" = "MoveLeft"
		"j" = "MoveDown"
		"k" = "MoveUp"
		"l" = "MoveRight"
		"Up" = "MoveUp"
		"Left" = "MoveLeft"
		"Right" = "MoveRight"
		"Down" = "MoveDown"
		"Plus" = "ZoomIn"
		"-" = "ZoomOut"
		"g g" = "JumpFirstPage"
		"G" = "JumpLastPage"
		"PageUp" = "PrevPage"
		"PageDown" = "NextPage"
		"Ctrl+b" = "PrevPage"
		"Ctrl+f" = "NextPage"
		"q" = "Quit"
		"Q" = "Quit"
		"#;

        const TEST_CONFIG: &str = r#"
		non_existent_1 = "hello!"
        
		[viewer]
		# Determines how fast the document is scrolled
		scroll_speed = 22.0
		# Determines at what precision the pages are rendered
		render_precision = 1.5
		# Determines the image data limit that the software holds in RAM (bytes)
		memory_limit = 31457
		# Minimum scale amount allowed
		scale_min = 0.2
		# Determines the default scale of the viewer when starting the viewer
		scale_default = 0.5
		# Determines the scaling amount when zooming in or out
		scale_amount = 0.5
		# Determines the margin on the bottom of each page
		margin_bottom = 10.0
		# Determines the amount of pages that are preloaded in advance 
		pages_preloaded = 3
		# Inverse vertical scroll
		inverse_scroll = false

		non_existent_2 = 0.0
		
		[viewer.uri_hint]
		# Enabled URI hints
		enabled = true
		# Background color of hint bar
		background = "blue"
		# Foreground color of hint bar text
		foreground = "red"
		# Hint bar width percentage based on terminal width
		width = 0.2 

		non_existent_3 = 0.0
		
		[bindings]
		"Ctrl+a" = "ToggleAlpha"
		"Ctrl+o" = "ToggleInverse"
		"C" = "CenterViewer"
		"h" = "MoveLeft"
		"j" = "MoveDown"
		"k" = "MoveUp"
		"l" = "MoveRight"
		"Up" = "MoveUp"
		"Left" = "MoveLeft"
		"Right" = "MoveRight"
		"Down" = "MoveDown"
		"Plus" = "ZoomIn"
		"-" = "ZoomOut"
		"g g" = "JumpFirstPage"
		"G" = "JumpLastPage"
		"PageUp" = "PrevPage"
		"PageDown" = "NextPage"
		"Ctrl+b" = "PrevPage"
		"Ctrl+f" = "NextPage"
		"q" = "Quit"
		"Q" = "Quit"

		non_existent_4 = true
		"#;

        let mut test_config_toml = TEST_CONFIG.parse::<Table>().unwrap();
        let reference_config_toml = REFERENCE_CONFIG.parse::<Table>().unwrap();

        assert!(fix_config_toml(
            &mut test_config_toml,
            &reference_config_toml
        ));

        /* [viewer] */
        assert!(test_config_toml.get("non_existent_1").is_none());
        assert!(test_config_toml["viewer"]["scroll_speed"] == Value::Float(22.0));
        assert!(test_config_toml["viewer"]["render_precision"] == Value::Float(1.5));
        assert!(test_config_toml["viewer"]["memory_limit"] == Value::Integer(31457));
        assert!(test_config_toml["viewer"]["scale_min"] == Value::Float(0.2));
        assert!(test_config_toml["viewer"]["scale_default"] == Value::Float(0.5));
        assert!(test_config_toml["viewer"]["scale_amount"] == Value::Float(0.5));
        assert!(test_config_toml["viewer"]["margin_bottom"] == Value::Float(10.0));
        assert!(test_config_toml["viewer"]["pages_preloaded"] == Value::Integer(3));
        assert!(test_config_toml["viewer"]["inverse_scroll"] == Value::Boolean(false));
        assert!(test_config_toml["viewer"].get("non_existent_2").is_none());

        assert!(
            test_config_toml["viewer"]
                .as_table()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
                == reference_config_toml["viewer"]
                    .as_table()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>()
        );

        /* [viewer.uri_hint] */
        assert!(
            test_config_toml["viewer"]["uri_hint"]["enabled"] == Value::Boolean(true)
        );
        assert!(
            test_config_toml["viewer"]["uri_hint"]["background"]
                == Value::String("blue".to_owned())
        );
        assert!(
            test_config_toml["viewer"]["uri_hint"]["foreground"]
                == Value::String("red".to_owned())
        );
        assert!(test_config_toml["viewer"]["uri_hint"]["width"] == Value::Float(0.2));
        assert!(test_config_toml["viewer"]["uri_hint"]
            .get("non_existent_3")
            .is_none());

        assert!(
            test_config_toml["viewer"]["uri_hint"]
                .as_table()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
                == reference_config_toml["viewer"]["uri_hint"]
                    .as_table()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>()
        );

        /* [bindings] */
        assert!(
            test_config_toml["bindings"]["Ctrl+a"]
                == Value::String("ToggleAlpha".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Ctrl+o"]
                == Value::String("ToggleInverse".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["C"] == Value::String("CenterViewer".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["h"] == Value::String("MoveLeft".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["j"] == Value::String("MoveDown".to_owned())
        );
        assert!(test_config_toml["bindings"]["k"] == Value::String("MoveUp".to_owned()));
        assert!(
            test_config_toml["bindings"]["l"] == Value::String("MoveRight".to_owned())
        );
        assert!(test_config_toml["bindings"]["Up"] == Value::String("MoveUp".to_owned()));
        assert!(
            test_config_toml["bindings"]["Left"] == Value::String("MoveLeft".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Right"]
                == Value::String("MoveRight".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Down"] == Value::String("MoveDown".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Plus"] == Value::String("ZoomIn".to_owned())
        );
        assert!(test_config_toml["bindings"]["-"] == Value::String("ZoomOut".to_owned()));
        assert!(
            test_config_toml["bindings"]["g g"]
                == Value::String("JumpFirstPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["G"] == Value::String("JumpLastPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["PageUp"]
                == Value::String("PrevPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["PageDown"]
                == Value::String("NextPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Ctrl+b"]
                == Value::String("PrevPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Ctrl+f"]
                == Value::String("NextPage".to_owned())
        );
        assert!(test_config_toml["bindings"]["q"] == Value::String("Quit".to_owned()));
        assert!(test_config_toml["bindings"]["Q"] == Value::String("Quit".to_owned()));
        assert!(test_config_toml["bindings"].get("non_existent_4").is_none());

        assert!(
            test_config_toml["bindings"]
                .as_table()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
                == reference_config_toml["bindings"]
                    .as_table()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_fix_config_addition_of_non_existent_variables() {
        const REFERENCE_CONFIG: &str = r#"
		show = true
        
		[viewer]
		# Determines how fast the document is scrolled
		scroll_speed = 20.0
		# Determines at what precision the pages are rendered
		render_precision = 1.5
		# Determines the image data limit that the software holds in RAM (bytes)
		memory_limit = 314572800
		# Minimum scale amount allowed
		scale_min = 0.2
		# Determines the default scale of the viewer when starting the viewer
		scale_default = 0.5
		# Determines the scaling amount when zooming in or out
		scale_amount = 0.5
		# Determines the margin on the bottom of each page
		margin_bottom = 10.0
		# Determines the amount of pages that are preloaded in advance 
		pages_preloaded = 3
		# Inverse vertical scroll
		inverse_scroll = false

		inverse_inverse_scroll = true
		
		[viewer.uri_hint]
		# Enabled URI hints
		enabled = true
		# Background color of hint bar
		background = "blue"
		# Foreground color of hint bar text
		foreground = "white"
		# Hint bar width percentage based on terminal width
		width = 0.2 

		height = 1.0
		
		[bindings]
		"Ctrl+a" = "ToggleAlpha"
		"Ctrl+o" = "ToggleInverse"
		"C" = "CenterViewer"
		"h" = "MoveLeft"
		"j" = "MoveDown"
		"k" = "MoveUp"
		"l" = "MoveRight"
		"Up" = "MoveUp"
		"Left" = "MoveLeft"
		"Right" = "MoveRight"
		"Down" = "MoveDown"
		"Plus" = "ZoomIn"
		"-" = "ZoomOut"
		"g g" = "JumpFirstPage"
		"G" = "JumpLastPage"
		"PageUp" = "PrevPage"
		"PageDown" = "NextPage"
		"Ctrl+b" = "PrevPage"
		"Ctrl+f" = "NextPage"
		"q" = "Quit"
		"Q" = "Quit"

		"bbbb" = "Quit"
		"#;

        const TEST_CONFIG: &str = r#"
		[viewer]
		# Determines how fast the document is scrolled
		scroll_speed = 22.0
		# Determines at what precision the pages are rendered
		render_precision = 1.5
		# Determines the image data limit that the software holds in RAM (bytes)
		memory_limit = 31457
		# Minimum scale amount allowed
		scale_min = 0.2
		# Determines the default scale of the viewer when starting the viewer
		scale_default = 0.5
		# Determines the scaling amount when zooming in or out
		scale_amount = 0.5
		# Determines the margin on the bottom of each page
		margin_bottom = 10.0
		# Determines the amount of pages that are preloaded in advance 
		pages_preloaded = 3
		# Inverse vertical scroll
		inverse_scroll = false
		
		[viewer.uri_hint]
		# Enabled URI hints
		enabled = true
		# Background color of hint bar
		background = "blue"
		# Foreground color of hint bar text
		foreground = "red"
		# Hint bar width percentage based on terminal width
		width = 0.2 

		[bindings]
		"Ctrl+a" = "ToggleAlpha"
		"Ctrl+o" = "ToggleInverse"
		"C" = "CenterViewer"
		"h" = "MoveLeft"
		"j" = "MoveDown"
		"k" = "MoveUp"
		"l" = "MoveRight"
		"Up" = "MoveUp"
		"Left" = "MoveLeft"
		"Right" = "MoveRight"
		"Down" = "MoveDown"
		"Plus" = "ZoomIn"
		"-" = "ZoomOut"
		"g g" = "JumpFirstPage"
		"G" = "JumpLastPage"
		"PageUp" = "PrevPage"
		"PageDown" = "NextPage"
		"Ctrl+b" = "PrevPage"
		"Ctrl+f" = "NextPage"
		"q" = "Quit"
		"Q" = "Quit"
		"#;

        let mut test_config_toml = TEST_CONFIG.parse::<Table>().unwrap();
        let reference_config_toml = REFERENCE_CONFIG.parse::<Table>().unwrap();

        assert!(fix_config_toml(
            &mut test_config_toml,
            &reference_config_toml
        ));

        assert!(test_config_toml["show"] == Value::Boolean(true));

        /* [viewer] */
        assert!(test_config_toml["viewer"]["scroll_speed"] == Value::Float(22.0));
        assert!(test_config_toml["viewer"]["render_precision"] == Value::Float(1.5));
        assert!(test_config_toml["viewer"]["memory_limit"] == Value::Integer(31457));
        assert!(test_config_toml["viewer"]["scale_min"] == Value::Float(0.2));
        assert!(test_config_toml["viewer"]["scale_default"] == Value::Float(0.5));
        assert!(test_config_toml["viewer"]["scale_amount"] == Value::Float(0.5));
        assert!(test_config_toml["viewer"]["margin_bottom"] == Value::Float(10.0));
        assert!(test_config_toml["viewer"]["pages_preloaded"] == Value::Integer(3));
        assert!(test_config_toml["viewer"]["inverse_scroll"] == Value::Boolean(false));
        assert!(
            test_config_toml["viewer"]["inverse_inverse_scroll"] == Value::Boolean(true)
        );

        assert!(
            test_config_toml["viewer"]
                .as_table()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
                == reference_config_toml["viewer"]
                    .as_table()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>()
        );

        /* [viewer.uri_hint] */
        assert!(
            test_config_toml["viewer"]["uri_hint"]["enabled"] == Value::Boolean(true)
        );
        assert!(
            test_config_toml["viewer"]["uri_hint"]["background"]
                == Value::String("blue".to_owned())
        );
        assert!(
            test_config_toml["viewer"]["uri_hint"]["foreground"]
                == Value::String("red".to_owned())
        );
        assert!(test_config_toml["viewer"]["uri_hint"]["width"] == Value::Float(0.2));
        assert!(test_config_toml["viewer"]["uri_hint"]["height"] == Value::Float(1.0));

        assert!(
            test_config_toml["viewer"]["uri_hint"]
                .as_table()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
                == reference_config_toml["viewer"]["uri_hint"]
                    .as_table()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>()
        );

        /* [bindings] */
        assert!(
            test_config_toml["bindings"]["Ctrl+a"]
                == Value::String("ToggleAlpha".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Ctrl+o"]
                == Value::String("ToggleInverse".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["C"] == Value::String("CenterViewer".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["h"] == Value::String("MoveLeft".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["j"] == Value::String("MoveDown".to_owned())
        );
        assert!(test_config_toml["bindings"]["k"] == Value::String("MoveUp".to_owned()));
        assert!(
            test_config_toml["bindings"]["l"] == Value::String("MoveRight".to_owned())
        );
        assert!(test_config_toml["bindings"]["Up"] == Value::String("MoveUp".to_owned()));
        assert!(
            test_config_toml["bindings"]["Left"] == Value::String("MoveLeft".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Right"]
                == Value::String("MoveRight".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Down"] == Value::String("MoveDown".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Plus"] == Value::String("ZoomIn".to_owned())
        );
        assert!(test_config_toml["bindings"]["-"] == Value::String("ZoomOut".to_owned()));
        assert!(
            test_config_toml["bindings"]["g g"]
                == Value::String("JumpFirstPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["G"] == Value::String("JumpLastPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["PageUp"]
                == Value::String("PrevPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["PageDown"]
                == Value::String("NextPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Ctrl+b"]
                == Value::String("PrevPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Ctrl+f"]
                == Value::String("NextPage".to_owned())
        );
        assert!(test_config_toml["bindings"]["q"] == Value::String("Quit".to_owned()));
        assert!(test_config_toml["bindings"]["Q"] == Value::String("Quit".to_owned()));
        assert!(test_config_toml["bindings"]["bbbb"] == Value::String("Quit".to_owned()));

        assert!(
            test_config_toml["bindings"]
                .as_table()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
                == reference_config_toml["bindings"]
                    .as_table()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_fix_config_replacement_of_wrong_variants() {
        const REFERENCE_CONFIG: &str = r#"
		[viewer]
		# Determines how fast the document is scrolled
		scroll_speed = 20.0
		# Determines at what precision the pages are rendered
		render_precision = 1.5
		# Determines the image data limit that the software holds in RAM (bytes)
		memory_limit = 314572800
		# Minimum scale amount allowed
		scale_min = 0.2
		# Determines the default scale of the viewer when starting the viewer
		scale_default = 0.5
		# Determines the scaling amount when zooming in or out
		scale_amount = 0.5
		# Determines the margin on the bottom of each page
		margin_bottom = 10.0
		# Determines the amount of pages that are preloaded in advance 
		pages_preloaded = 3
		# Inverse vertical scroll
		inverse_scroll = "no"
		
		[viewer.uri_hint]
		# Enabled URI hints
		enabled = true
		# Background color of hint bar
		background = "blue"
		# Foreground color of hint bar text
		foreground = 10.0
		# Hint bar width percentage based on terminal width
		width = 0.2 
		
		[bindings]
		"Ctrl+a" = "ToggleAlpha"
		"Ctrl+o" = "ToggleInverse"
		"C" = "CenterViewer"
		"h" = "MoveLeft"
		"j" = "MoveDown"
		"k" = "MoveUp"
		"l" = "MoveRight"
		"Up" = "MoveUp"
		"Left" = "MoveLeft"
		"Right" = "MoveRight"
		"Down" = "MoveDown"
		"Plus" = "ZoomIn"
		"-" = "ZoomOut"
		"g g" = "JumpFirstPage"
		"G" = "JumpLastPage"
		"PageUp" = "PrevPage"
		"PageDown" = "NextPage"
		"Ctrl+b" = "PrevPage"
		"Ctrl+f" = "NextPage"
		"q" = "Quit"
		"Q" = 2
		"#;

        const TEST_CONFIG: &str = r#"
		[viewer]
		# Determines how fast the document is scrolled
		scroll_speed = 22.0
		# Determines at what precision the pages are rendered
		render_precision = 1.5
		# Determines the image data limit that the software holds in RAM (bytes)
		memory_limit = 31457
		# Minimum scale amount allowed
		scale_min = 0.2
		# Determines the default scale of the viewer when starting the viewer
		scale_default = 0.5
		# Determines the scaling amount when zooming in or out
		scale_amount = 0.5
		# Determines the margin on the bottom of each page
		margin_bottom = 10.0
		# Determines the amount of pages that are preloaded in advance 
		pages_preloaded = 3
		# Inverse vertical scroll
		inverse_scroll = false
		
		[viewer.uri_hint]
		# Enabled URI hints
		enabled = true
		# Background color of hint bar
		background = "blue"
		# Foreground color of hint bar text
		foreground = "white"
		# Hint bar width percentage based on terminal width
		width = 0.2 
		
		[bindings]
		"Ctrl+a" = "ToggleAlpha"
		"Ctrl+o" = "ToggleInverse"
		"C" = "CenterViewer"
		"h" = "MoveLeft"
		"j" = "MoveDown"
		"k" = "MoveUp"
		"l" = "MoveRight"
		"Up" = "MoveUp"
		"Left" = "MoveLeft"
		"Right" = "MoveRight"
		"Down" = "MoveDown"
		"Plus" = "ZoomIn"
		"-" = "ZoomOut"
		"g g" = "JumpFirstPage"
		"G" = "JumpLastPage"
		"PageUp" = "PrevPage"
		"PageDown" = "NextPage"
		"Ctrl+b" = "PrevPage"
		"Ctrl+f" = "NextPage"
		"q" = "Quit"
		"Q" = "Quit"
		"#;

        let mut test_config_toml = TEST_CONFIG.parse::<Table>().unwrap();
        let reference_config_toml = REFERENCE_CONFIG.parse::<Table>().unwrap();

        assert!(fix_config_toml(
            &mut test_config_toml,
            &reference_config_toml
        ));

        /* [viewer] */
        assert!(test_config_toml["viewer"]["scroll_speed"] == Value::Float(22.0));
        assert!(test_config_toml["viewer"]["render_precision"] == Value::Float(1.5));
        assert!(test_config_toml["viewer"]["memory_limit"] == Value::Integer(31457));
        assert!(test_config_toml["viewer"]["scale_min"] == Value::Float(0.2));
        assert!(test_config_toml["viewer"]["scale_default"] == Value::Float(0.5));
        assert!(test_config_toml["viewer"]["scale_amount"] == Value::Float(0.5));
        assert!(test_config_toml["viewer"]["margin_bottom"] == Value::Float(10.0));
        assert!(test_config_toml["viewer"]["pages_preloaded"] == Value::Integer(3));
        assert!(
            test_config_toml["viewer"]["inverse_scroll"]
                == Value::String("no".to_owned())
        );

        assert!(
            test_config_toml["viewer"]
                .as_table()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
                == reference_config_toml["viewer"]
                    .as_table()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>()
        );

        /* [viewer.uri_hint] */
        assert!(
            test_config_toml["viewer"]["uri_hint"]["enabled"] == Value::Boolean(true)
        );
        assert!(
            test_config_toml["viewer"]["uri_hint"]["background"]
                == Value::String("blue".to_owned())
        );
        assert!(
            test_config_toml["viewer"]["uri_hint"]["foreground"] == Value::Float(10.0)
        );
        assert!(test_config_toml["viewer"]["uri_hint"]["width"] == Value::Float(0.2));

        assert!(
            test_config_toml["viewer"]["uri_hint"]
                .as_table()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
                == reference_config_toml["viewer"]["uri_hint"]
                    .as_table()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>()
        );

        /* [bindings] */
        assert!(
            test_config_toml["bindings"]["Ctrl+a"]
                == Value::String("ToggleAlpha".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Ctrl+o"]
                == Value::String("ToggleInverse".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["C"] == Value::String("CenterViewer".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["h"] == Value::String("MoveLeft".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["j"] == Value::String("MoveDown".to_owned())
        );
        assert!(test_config_toml["bindings"]["k"] == Value::String("MoveUp".to_owned()));
        assert!(
            test_config_toml["bindings"]["l"] == Value::String("MoveRight".to_owned())
        );
        assert!(test_config_toml["bindings"]["Up"] == Value::String("MoveUp".to_owned()));
        assert!(
            test_config_toml["bindings"]["Left"] == Value::String("MoveLeft".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Right"]
                == Value::String("MoveRight".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Down"] == Value::String("MoveDown".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Plus"] == Value::String("ZoomIn".to_owned())
        );
        assert!(test_config_toml["bindings"]["-"] == Value::String("ZoomOut".to_owned()));
        assert!(
            test_config_toml["bindings"]["g g"]
                == Value::String("JumpFirstPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["G"] == Value::String("JumpLastPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["PageUp"]
                == Value::String("PrevPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["PageDown"]
                == Value::String("NextPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Ctrl+b"]
                == Value::String("PrevPage".to_owned())
        );
        assert!(
            test_config_toml["bindings"]["Ctrl+f"]
                == Value::String("NextPage".to_owned())
        );
        assert!(test_config_toml["bindings"]["q"] == Value::String("Quit".to_owned()));
        assert!(test_config_toml["bindings"]["Q"] == Value::Integer(2));

        assert!(
            test_config_toml["bindings"]
                .as_table()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
                == reference_config_toml["bindings"]
                    .as_table()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>()
        );
    }
}
