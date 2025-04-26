pub mod language;
pub mod theme;

use std::{
    collections::{HashMap, HashSet},
    env::current_exe,
    fs::{read_dir, read_to_string},
    path::{absolute, Path, PathBuf},
};

use language::{IndentWidth, Language};
use serde::Deserialize;
use theme::Theme;

use crate::{
    platform::dialog::{message, MessageKind},
    text::{
        doc::Doc,
        syntax::{Syntax, SyntaxRange, SyntaxToken},
    },
};

const CONFIG_FILE: &str = "config.toml";
const CONFIG_DIR: &str = "config";
const DEFAULT_COMMENT: fn() -> String = || "//".to_owned();
const DEFAULT_TRIM_TRAILING_WHITESPACE: fn() -> bool = || true;
const DEFAULT_TERMINAL_HEIGHT: fn() -> f32 = || 12.0;
const DEFAULT_IGNORED_DIRS: fn() -> Vec<String> = || {
    ["target", "build", "out", ".git"]
        .iter()
        .copied()
        .map(str::to_owned)
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
    pub fn get_syntax(self) -> Syntax {
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
    extensions: Vec<String>,
    #[serde(default)]
    indent_width: IndentWidth,
    #[serde(default = "DEFAULT_COMMENT")]
    comment: String,
    lsp_language_id: Option<String>,
    language_server_command: Option<String>,
    #[serde(borrow)]
    syntax: Option<SyntaxDesc<'a>>,
}

#[derive(Deserialize, Debug)]
struct ConfigDesc<'a> {
    font: String,
    font_size: f32,
    #[serde(default = "DEFAULT_TRIM_TRAILING_WHITESPACE")]
    trim_trailing_whitespace: bool,
    #[serde(default = "DEFAULT_TERMINAL_HEIGHT")]
    terminal_height: f32,
    theme: &'a str,
    #[serde(default = "DEFAULT_IGNORED_DIRS")]
    ignored_dirs: Vec<String>,
}

pub struct ConfigError {
    title: &'static str,
    text: String,
}

impl ConfigError {
    pub fn new(title: &'static str, text: String) -> Self {
        Self { title, text }
    }

    pub fn show_message(&self) {
        message(self.title, &self.text, MessageKind::Ok);
    }
}

pub struct Config {
    pub font: String,
    pub font_size: f32,
    pub trim_trailing_whitespace: bool,
    pub terminal_height: f32,
    pub theme: Theme,
    pub languages: Vec<Language>,
    pub extension_languages: HashMap<String, usize>,
    pub ignored_dirs: HashSet<String>,
}

impl Config {
    pub fn load(dir: &Path) -> Result<Config, ConfigError> {
        let mut path = PathBuf::new();

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
                let language_desc =
                    Self::load_file_data::<LanguageDesc>(&path, &language_desc_string)?;

                let index = languages.len();

                languages.push(Language {
                    index,
                    indent_width: language_desc.indent_width,
                    comment: language_desc.comment,
                    lsp_language_id: language_desc.lsp_language_id,
                    language_server_command: language_desc.language_server_command,
                    syntax: language_desc
                        .syntax
                        .map(|syntax_desc| syntax_desc.get_syntax()),
                });

                for extension in language_desc.extensions {
                    extension_languages.insert(extension, index);
                }
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
        path.set_extension("toml");

        let theme_string = Self::load_file_string(&path)?;
        let theme = Self::load_file_data(&path, &theme_string)?;

        let ignored_dirs = HashSet::from_iter(config_desc.ignored_dirs);

        Ok(Config {
            font: config_desc.font,
            font_size: config_desc.font_size,
            trim_trailing_whitespace: config_desc.trim_trailing_whitespace,
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
                format!("Unable to open \"{}\": {}", file_name, err),
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

        match basic_toml::from_str::<T>(string) {
            Ok(data) => Ok(data),
            Err(err) => Err(ConfigError::new(
                "Error Loading Config",
                format!("Unable to load \"{}\": {}", file_name, err),
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

    pub fn get_indent_width_for_doc(&self, doc: &Doc) -> IndentWidth {
        self.get_language_for_doc(doc)
            .map(|language| language.indent_width)
            .unwrap_or_default()
    }

    pub fn get_dir() -> PathBuf {
        let relative_dir = {
            if let Some(exe_dir) = current_exe().as_ref().ok().and_then(|exe| exe.parent()) {
                let mut config_path = exe_dir.to_owned();
                config_path.push(CONFIG_DIR);

                if config_path.exists() {
                    return config_path;
                }
            }

            CONFIG_DIR
        };

        absolute(relative_dir).unwrap()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font: String::new(),
            font_size: 13.0,
            trim_trailing_whitespace: DEFAULT_TRIM_TRAILING_WHITESPACE(),
            terminal_height: DEFAULT_TERMINAL_HEIGHT(),
            theme: Theme::default(),
            languages: Vec::new(),
            extension_languages: HashMap::new(),
            ignored_dirs: HashSet::from_iter(DEFAULT_IGNORED_DIRS()),
        }
    }
}
