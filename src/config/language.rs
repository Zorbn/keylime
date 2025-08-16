use std::collections::HashSet;

use serde::Deserialize;
use serde_json::Value;

use crate::{config::LanguageBlocksDesc, platform::gfx::Gfx, pool::Pooled, text::syntax::Syntax};

use super::{LanguageDesc, SyntaxDesc};

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

pub struct LanguageBlocks {
    pub do_start_on_newline: bool,
    pub start_tokens: HashSet<Pooled<String>>,
    pub end_tokens: HashSet<Pooled<String>>,
}

impl LanguageBlocks {
    pub(super) fn new(desc: LanguageBlocksDesc) -> Self {
        Self {
            do_start_on_newline: desc.do_start_on_newline,
            start_tokens: HashSet::from_iter(desc.start_tokens),
            end_tokens: HashSet::from_iter(desc.end_tokens),
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
            blocks: LanguageBlocks::new(desc.blocks),
            comment: desc.comment,
            lsp: desc.lsp,
            syntax: desc.syntax.map(SyntaxDesc::syntax),
        }
    }
}
