pub mod language;
pub mod theme;

use std::{
    collections::{HashMap, HashSet},
    env::current_exe,
    fs::{read_dir, read_to_string},
    path::{Path, PathBuf},
};

use language::{IndentWidth, Language};
use serde::Deserialize;
use theme::Theme;

use crate::{
    normalizable::Normalizable,
    platform::dialog::{message, MessageKind},
    pool::{format_pooled, Pooled, PATH_POOL, STRING_POOL},
    text::{
        doc::Doc,
        syntax::{Syntax, SyntaxRange, SyntaxToken},
    },
};

const CONFIG_FILE: &str = "config.json";
const CONFIG_DIR: &str = "config";
const DEFAULT_COMMENT: fn() -> Pooled<String> = || "//".into();
const DEFAULT_TRIM_TRAILING_WHITESPACE: fn() -> bool = || true;
const DEFAULT_FORMAT_ON_SAVE: fn() -> bool = || true;
const DEFAULT_TERMINAL_HEIGHT: fn() -> f32 = || 12.0;
const DEFAULT_IGNORED_DIRS: fn() -> Vec<Pooled<String>> = || {
    ["target", "build", "out", ".git"]
        .iter()
        .copied()
        .map(|str| str.into())
        .collect()
};

#[derive(Deserialize, Debug)]
struct SyntaxDesc<'a> {
    #[serde(default, borrow)]
    keywords: Vec<&'a str>,
    #[serde(default)]
    tokens: Vec<SyntaxToken>,
    #[serde(default)]
    ranges: Vec<SyntaxRange>,
}

impl SyntaxDesc<'_> {
    pub fn syntax(self) -> Syntax {
        let mut keywords = HashSet::new();

        for keyword in self.keywords {
            keywords.insert(keyword.to_string());
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
    name: Pooled<String>,
    extensions: Vec<Pooled<String>>,
    #[serde(default)]
    indent_width: IndentWidth,
    #[serde(default = "DEFAULT_COMMENT")]
    comment: Pooled<String>,
    lsp_language_id: Option<Pooled<String>>,
    language_server_command: Option<Pooled<String>>,
    #[serde(borrow)]
    syntax: Option<SyntaxDesc<'a>>,
}

#[derive(Deserialize, Debug)]
struct ConfigDesc<'a> {
    font: Pooled<String>,
    font_size: f32,
    #[serde(default = "DEFAULT_TRIM_TRAILING_WHITESPACE")]
    trim_trailing_whitespace: bool,
    #[serde(default = "DEFAULT_FORMAT_ON_SAVE")]
    format_on_save: bool,
    #[serde(default = "DEFAULT_TERMINAL_HEIGHT")]
    terminal_height: f32,
    theme: &'a str,
    #[serde(default = "DEFAULT_IGNORED_DIRS")]
    ignored_dirs: Vec<Pooled<String>>,
}

pub struct ConfigError {
    title: &'static str,
    text: Pooled<String>,
}

impl ConfigError {
    pub fn new(title: &'static str, text: Pooled<String>) -> Self {
        Self { title, text }
    }

    pub fn show_message(&self) {
        message(self.title, &self.text, MessageKind::Ok);
    }
}

pub struct Config {
    pub font: Pooled<String>,
    pub font_size: f32,
    pub trim_trailing_whitespace: bool,
    pub format_on_save: bool,
    pub terminal_height: f32,
    pub theme: Theme,
    pub languages: Vec<Language>,
    pub extension_languages: HashMap<Pooled<String>, usize>,
    pub ignored_dirs: HashSet<Pooled<String>>,
}

impl Config {
    pub fn load(dir: &Path) -> Result<Config, ConfigError> {
        let mut path = PATH_POOL.new_item();
        path.push(dir);
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
                let mut language_desc =
                    Self::load_file_data::<LanguageDesc>(&path, &language_desc_string)?;

                let index = languages.len();

                for extension in language_desc.extensions.drain(..) {
                    extension_languages.insert(extension, index);
                }

                languages.push(Language::new(index, language_desc));
            }
        }

        path.clear();
        path.push(dir);
        path.push(CONFIG_FILE);

        let config_desc_string = Self::load_file_string(&path)?;
        let config_desc = Self::load_file_data::<ConfigDesc>(&path, &config_desc_string)?;

        path.clear();
        path.push(dir);
        path.push("themes");
        path.push(config_desc.theme);
        path.set_extension("json");

        let theme_string = Self::load_file_string(&path)?;
        let theme = Self::load_file_data(&path, &theme_string)?;

        let ignored_dirs = HashSet::from_iter(config_desc.ignored_dirs);

        Ok(Config {
            font: config_desc.font,
            font_size: config_desc.font_size,
            trim_trailing_whitespace: config_desc.trim_trailing_whitespace,
            format_on_save: config_desc.format_on_save,
            terminal_height: config_desc.terminal_height.max(0.0),
            ignored_dirs,
            theme,
            languages,
            extension_languages,
        })
    }

    fn load_file_string(path: &Path) -> Result<String, ConfigError> {
        let file_name = path
            .file_stem()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or_default();

        match read_to_string(path) {
            Ok(string) => Ok(string),
            Err(err) => Err(ConfigError::new(
                "Error Opening Config",
                format_pooled!("Unable to open \"{}\": {}", file_name, err),
            )),
        }
    }

    fn load_file_data<'a, T: Deserialize<'a> + 'a>(
        path: &Path,
        string: &'a str,
    ) -> Result<T, ConfigError> {
        let file_name = path
            .file_stem()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or_default();

        match serde_json::from_str::<T>(string) {
            Ok(data) => Ok(data),
            Err(err) => Err(ConfigError::new(
                "Error Loading Config",
                format_pooled!("Unable to load \"{}\": {}", file_name, err),
            )),
        }
    }

    pub fn get_language(&self, extension: &str) -> Option<&Language> {
        self.extension_languages
            .get(extension)
            .and_then(|index| self.languages.get(*index))
    }

    pub fn get_language_for_doc<'a>(&'a self, doc: &Doc) -> Option<&'a Language> {
        doc.path()
            .some()
            .and_then(|path| path.extension())
            .and_then(|extension| extension.to_str())
            .and_then(|extension| self.get_language(extension))
    }

    pub fn indent_width_for_doc(&self, doc: &Doc) -> IndentWidth {
        self.get_language_for_doc(doc)
            .map(|language| language.indent_width)
            .unwrap_or_default()
    }

    pub fn dir() -> Pooled<PathBuf> {
        if let Some(exe_dir) = current_exe().as_ref().ok().and_then(|exe| exe.parent()) {
            let mut config_path: Pooled<PathBuf> = exe_dir.into();

            loop {
                config_path.push(CONFIG_DIR);

                if config_path.exists() {
                    return config_path;
                }

                config_path.pop();

                if config_path.parent().is_none() {
                    break;
                }

                config_path.pop();
            }
        }

        CONFIG_DIR.normalized().unwrap()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font: STRING_POOL.new_item(),
            font_size: 13.0,
            trim_trailing_whitespace: DEFAULT_TRIM_TRAILING_WHITESPACE(),
            format_on_save: DEFAULT_FORMAT_ON_SAVE(),
            terminal_height: DEFAULT_TERMINAL_HEIGHT(),
            theme: Theme::default(),
            languages: Vec::new(),
            extension_languages: HashMap::new(),
            ignored_dirs: HashSet::from_iter(DEFAULT_IGNORED_DIRS()),
        }
    }
}
