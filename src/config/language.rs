use serde::Deserialize;
use serde_json::Value;

use crate::{platform::gfx::Gfx, pool::Pooled, text::syntax::Syntax};

use super::{LanguageDesc, SyntaxDesc};

const DEFAULT_BLOCK_START_DELIMITERS: fn() -> Vec<Pooled<String>> =
    || ["{", "[", "("].iter().copied().map(Into::into).collect();
const DEFAULT_BLOCK_END_DELIMITERS: fn() -> Vec<Pooled<String>> =
    || ["}", "]", ")"].iter().copied().map(Into::into).collect();

#[derive(Deserialize, Debug, Default, Clone, Copy)]
#[serde(untagged)]
pub enum IndentWidth {
    #[default]
    Tab,
    Spaces(usize),
}

impl IndentWidth {
    pub fn grapheme_count(&self) -> usize {
        match self {
            Self::Tab => 1,
            Self::Spaces(indent_width) => *indent_width,
        }
    }

    pub fn measure(&self, gfx: &mut Gfx) -> usize {
        gfx.measure_text(self.grapheme()) * self.grapheme_count()
    }

    pub fn grapheme(&self) -> &'static str {
        match self {
            Self::Tab => "\t",
            Self::Spaces(_) => " ",
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct LanguageLsp {
    pub language_id: Option<Pooled<String>>,
    pub command: Option<Pooled<String>>,
    pub options: Option<Value>,
}

pub enum DelimiterKind {
    Start,
    End,
}

#[derive(Debug, Deserialize)]
pub struct LanguageBlocks {
    #[serde(default)]
    pub do_start_on_newline: bool,
    #[serde(default = "DEFAULT_BLOCK_START_DELIMITERS")]
    pub start_delimiters: Vec<Pooled<String>>,
    #[serde(default = "DEFAULT_BLOCK_END_DELIMITERS")]
    pub end_delimiters: Vec<Pooled<String>>,
    #[serde(default)]
    pub are_delimiters_words: bool,
}

impl Default for LanguageBlocks {
    fn default() -> Self {
        Self {
            do_start_on_newline: false,
            start_delimiters: DEFAULT_BLOCK_START_DELIMITERS(),
            end_delimiters: DEFAULT_BLOCK_END_DELIMITERS(),
            are_delimiters_words: false,
        }
    }
}

pub struct Language {
    pub index: usize,
    pub name: Pooled<String>,
    pub indent_width: IndentWidth,
    pub blocks: LanguageBlocks,
    pub syntax: Option<Syntax>,
    pub comment: Pooled<String>,
    pub lsp: LanguageLsp,
}

impl Language {
    pub(super) fn new(index: usize, desc: LanguageDesc) -> Self {
        Self {
            index,
            name: desc.name,
            indent_width: desc.indent_width,
            blocks: desc.blocks,
            comment: desc.comment,
            lsp: desc.lsp,
            syntax: desc.syntax.map(SyntaxDesc::syntax),
        }
    }
}
