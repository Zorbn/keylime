pub mod theme;

use std::{collections::HashMap, env::current_exe, fs::read_to_string, path::PathBuf};

use serde::Deserialize;
use theme::Theme;

use crate::{
    platform::dialog::{message, MessageKind},
    text::{
        doc::Doc,
        syntax::{Syntax, SyntaxRange},
    },
    ui::color::Color,
};

const CONFIG_PATH: &str = "config.toml";
const DEFAULT_COMMENT: &str = "//";

#[derive(Deserialize, Debug)]
pub struct SyntaxDesc<'a> {
    #[serde(borrow)]
    pub keywords: Vec<&'a str>,
    pub ranges: Vec<SyntaxRange>,
}

#[derive(Deserialize, Debug)]
struct LanguageDesc<'a> {
    #[serde(borrow)]
    extensions: Vec<&'a str>,
    indent_width: Option<usize>,
    comment: Option<&'a str>,
    syntax: Option<SyntaxDesc<'a>>,
}

#[derive(Deserialize, Debug)]
struct ConfigDesc<'a> {
    font: String,
    font_size: f32,
    theme: String,
    #[serde(default, borrow)]
    language: HashMap<&'a str, LanguageDesc<'a>>,
}

pub struct Language {
    pub indent_width: Option<usize>,
    pub syntax: Option<Syntax>,
    pub comment: String,
}

pub struct Config {
    pub font: String,
    pub font_size: f32,
    pub theme: Theme,
    pub languages: Vec<Language>,
    pub extension_languages: HashMap<String, usize>,
}

impl Config {
    pub fn load() -> Option<Config> {
        let config_dir = Self::get_config_directory();

        let mut path = PathBuf::new();

        path.push(&config_dir);
        path.push(CONFIG_PATH);

        let config_desc_string = match read_to_string(&path) {
            Ok(config_desc_string) => config_desc_string,
            Err(err) => {
                message(
                    "Error Opening Config",
                    &format!("Unable to open config: {}", err),
                    MessageKind::Ok,
                );
                return None;
            }
        };

        let config_desc = match basic_toml::from_str::<ConfigDesc>(&config_desc_string) {
            Ok(config_desc) => config_desc,
            Err(err) => {
                message(
                    "Error Loading Config",
                    &format!("Unable to load config: {}", err),
                    MessageKind::Ok,
                );
                return None;
            }
        };

        path.clear();
        path.push(&config_dir);
        path.push("themes");
        path.push(&config_desc.theme);
        path.set_extension("toml");

        let theme_string = match read_to_string(&path) {
            Ok(theme_string) => theme_string,
            Err(err) => {
                message(
                    "Error Opening Theme",
                    &format!("Unable to open theme \"{}\": {}", config_desc.theme, err),
                    MessageKind::Ok,
                );
                return None;
            }
        };

        let theme = match basic_toml::from_str::<Theme>(&theme_string) {
            Ok(theme) => theme,
            Err(err) => {
                message(
                    "Error Loading Theme",
                    &format!("Unable to load theme \"{}\": {}", config_desc.theme, err),
                    MessageKind::Ok,
                );
                return None;
            }
        };

        let mut languages = Vec::new();
        let mut extension_languages = HashMap::new();

        for (_, language) in config_desc.language {
            let language_index = languages.len();

            let syntax = language
                .syntax
                .map(|syntax| Syntax::new(&syntax.keywords, &syntax.ranges));

            languages.push(Language {
                indent_width: language.indent_width,
                comment: language.comment.unwrap_or(DEFAULT_COMMENT).to_owned(),
                syntax,
            });

            for extension in language.extensions {
                extension_languages.insert(extension.to_owned(), language_index);
            }
        }

        Some(Config {
            font: config_desc.font,
            font_size: config_desc.font_size,
            theme,
            languages,
            extension_languages,
        })
    }

    pub fn get_language(&self, extension: &str) -> Option<&Language> {
        self.extension_languages
            .get(extension)
            .and_then(|index| self.languages.get(*index))
    }

    pub fn get_language_for_doc<'a>(&'a self, doc: &Doc) -> Option<&'a Language> {
        doc.path()
            .and_then(|path| path.extension())
            .and_then(|extension| extension.to_str())
            .and_then(|extension| self.get_language(extension))
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
            languages: Vec::new(),
            extension_languages: HashMap::new(),
        }
    }
}
