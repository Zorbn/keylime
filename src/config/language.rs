use serde::Deserialize;
use serde_json::Value;

use crate::{platform::gfx::Gfx, pool::Pooled, text::syntax::Syntax};

use super::LanguageDesc;

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

pub struct Language {
    pub index: usize,
    pub name: Pooled<String>,
    pub indent_width: IndentWidth,
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
            comment: desc.comment,
            lsp: desc.lsp,
            syntax: desc.syntax.map(|syntax_desc| syntax_desc.syntax()),
        }
    }
}
