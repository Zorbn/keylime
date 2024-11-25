pub mod theme;

use std::{
    collections::{HashMap, HashSet},
    env::current_exe,
    fs::{read_dir, read_to_string},
    path::{Path, PathBuf},
};

use serde::Deserialize;
use theme::Theme;

use crate::{
    platform::dialog::{message, MessageKind},
    text::{
        doc::Doc,
        syntax::{Syntax, SyntaxRange, SyntaxToken},
    },
    ui::color::Color,
};

const CONFIG_PATH: &str = "config.toml";
const DEFAULT_COMMENT: fn() -> &'static str = || "//";
const DEFAULT_TRIM_TRAILING_WHITESPACE: fn() -> bool = || true;

#[derive(Deserialize, Debug)]
struct SyntaxDesc<'a> {
    #[serde(default, borrow)]
    keywords: Vec<&'a str>,
    #[serde(default)]
    tokens: Vec<SyntaxToken>,
    #[serde(default)]
    ranges: Vec<SyntaxRange>,
}

impl<'a> SyntaxDesc<'a> {
    pub fn get_syntax(self) -> Syntax {
        let mut keywords = HashSet::new();

        for keyword in self.keywords {
            keywords.insert(keyword.chars().collect());
        }

        Syntax {
            keywords,
            tokens: self.tokens,
            ranges: self.ranges,
        }
    }
}

#[derive(Deserialize, Debug)]
struct LanguageDesc<'a> {
    #[serde(borrow)]
    extensions: Vec<&'a str>,
    indent_width: Option<usize>,
    #[serde(default = "DEFAULT_COMMENT")]
    comment: &'a str,
    syntax: Option<SyntaxDesc<'a>>,
}

#[derive(Deserialize, Debug)]
struct ConfigDesc<'a> {
    font: &'a str,
    font_size: f32,
    #[serde(default = "DEFAULT_TRIM_TRAILING_WHITESPACE")]
    trim_trailing_whitespace: bool,
    theme: &'a str,
}

pub struct Language {
    pub indent_width: Option<usize>,
    pub syntax: Option<Syntax>,
    pub comment: String,
}

pub struct Config {
    pub font: String,
    pub font_size: f32,
    pub trim_trailing_whitespace: bool,
    pub theme: Theme,
    pub languages: Vec<Language>,
    pub extension_languages: HashMap<String, usize>,
}

impl Config {
    pub fn load() -> Option<Config> {
        let config_dir = Self::get_config_directory();

        let mut path = PathBuf::new();

        path.push(&config_dir);
        path.push("languages");

        let mut languages = Vec::new();
        let mut extension_languages = HashMap::new();

        if let Ok(entries) = read_dir(&path) {
            for entry in entries {
                let Ok(entry) = entry else {
                    continue;
                };

                let path = entry.path();
                let language_desc_string = Self::load_file_string(&path)?;
                let language_desc =
                    Self::load_file_data::<LanguageDesc>(&path, &language_desc_string)?;

                let language_index = languages.len();

                languages.push(Language {
                    indent_width: language_desc.indent_width,
                    comment: language_desc.comment.to_owned(),
                    syntax: language_desc
                        .syntax
                        .map(|syntax_desc| syntax_desc.get_syntax()),
                });

                for extension in language_desc.extensions {
                    extension_languages.insert(extension.to_owned(), language_index);
                }
            }
        }

        path.clear();
        path.push(&config_dir);
        path.push(CONFIG_PATH);

        let config_desc_string = Self::load_file_string(&path)?;
        let config_desc = Self::load_file_data::<ConfigDesc>(&path, &config_desc_string)?;

        path.clear();
        path.push(&config_dir);
        path.push("themes");
        path.push(config_desc.theme);
        path.set_extension("toml");

        let theme_string = Self::load_file_string(&path)?;
        let theme = Self::load_file_data(&path, &theme_string)?;

        Some(Config {
            font: config_desc.font.to_owned(),
            font_size: config_desc.font_size,
            trim_trailing_whitespace: config_desc.trim_trailing_whitespace,
            theme,
            languages,
            extension_languages,
        })
    }

    fn load_file_string(path: &Path) -> Option<String> {
        let file_name = path
            .file_stem()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or_default();

        match read_to_string(path) {
            Ok(string) => Some(string),
            Err(err) => {
                message(
                    "Error Opening Config",
                    &format!("Unable to open \"{}\": {}", file_name, err),
                    MessageKind::Ok,
                );

                None
            }
        }
    }

    fn load_file_data<'a, T: Deserialize<'a> + 'a>(path: &Path, string: &'a str) -> Option<T> {
        let file_name = path
            .file_stem()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or_default();

        match basic_toml::from_str::<T>(string) {
            Ok(data) => Some(data),
            Err(err) => {
                message(
                    "Error Loading Config",
                    &format!("Unable to load \"{}\": {}", file_name, err),
                    MessageKind::Ok,
                );

                None
            }
        }
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
            trim_trailing_whitespace: DEFAULT_TRIM_TRAILING_WHITESPACE(),
            theme: Theme {
                normal: Color::from_hex(0x000000FF),
                comment: Color::from_hex(0x008000FF),
                keyword: Color::from_hex(0x0000FFFF),
                function: Color::from_hex(0x795E26FF),
                number: Color::from_hex(0x098658FF),
                symbol: Color::from_hex(0x000000FF),
                string: Color::from_hex(0xA31515FF),
                meta: Color::from_hex(0xAF00DBFF),
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
