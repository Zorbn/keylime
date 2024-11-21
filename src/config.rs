pub mod theme;

use std::{env::current_exe, fs::read_to_string, path::PathBuf};

use serde::{de::Error, Deserialize, Deserializer};
use theme::Theme;

use crate::{
    platform::dialog::{message, MessageKind},
    ui::color::Color,
};

const CONFIG_PATH: &str = "config.toml";

#[derive(Deserialize, Debug)]
pub struct Config {
    pub font: String,
    pub font_size: f32,
    #[serde(deserialize_with = "theme_from_path")]
    pub theme: Theme,
}

impl Config {
    pub fn load() -> Option<Config> {
        let config_dir = Self::get_config_directory();

        let mut path = PathBuf::new();

        path.push(config_dir);
        path.push(CONFIG_PATH);

        let config_string = match read_to_string(&path) {
            Ok(config_string) => config_string,
            Err(err) => {
                message(
                    "Error Opening Config",
                    &format!("Unable to open config: {}", err),
                    MessageKind::Ok,
                );
                return None;
            }
        };

        let config = match basic_toml::from_str::<Config>(&config_string) {
            Ok(config) => config,
            Err(err) => {
                message(
                    "Error Loading Config",
                    &format!("Unable to load config: {}", err),
                    MessageKind::Ok,
                );
                return None;
            }
        };

        println!("{:?}", config.font);

        Some(config)
    }

    fn get_config_directory() -> PathBuf {
        if let Some(exe_dir) = current_exe().as_ref().ok().and_then(|exe| exe.parent()) {
            let mut config_path = exe_dir.to_owned();
            config_path.push(CONFIG_PATH);

            if config_path.exists() {
                return exe_dir.to_owned();
            }
        }

        PathBuf::from(".")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font: "Consolas".into(),
            font_size: 13.0,
            theme: Theme {
                normal: Color::from_hex(0x000000FF),
                comment: Color::from_hex(0x008000FF),
                keyword: Color::from_hex(0x0000FFFF),
                function: Color::from_hex(0x795E26FF),
                number: Color::from_hex(0x098658FF),
                symbol: Color::from_hex(0x000000FF),
                string: Color::from_hex(0xA31515FF),
                preprocessor: Color::from_hex(0xAF00DBFF),
                selection: Color::from_hex(0x4CADE47F),
                line_number: Color::from_hex(0x6E7681FF),
                border: Color::from_hex(0xE5E5E5FF),
                background: Color::from_hex(0xF5F5F5FF),
            },
        }
    }
}

fn theme_from_path<'de, D>(deserializer: D) -> Result<Theme, D::Error>
where
    D: Deserializer<'de>,
{
    let file_name: &str = Deserialize::deserialize(deserializer)?;

    let path = format!("{}.toml", file_name);

    let string = read_to_string(path)
        .map_err(|_| D::Error::custom(format!("unable to load theme \"{}\"", file_name)))?;

    basic_toml::from_str::<Theme>(&string).map_err(|err| D::Error::custom(err.to_string()))
}
