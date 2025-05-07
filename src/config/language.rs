use serde::Deserialize;

use crate::{platform::gfx::Gfx, text::syntax::Syntax};

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
            IndentWidth::Tab => 1,
            IndentWidth::Spaces(indent_width) => *indent_width,
        }
    }

    pub fn measure(&self, gfx: &mut Gfx) -> usize {
        gfx.measure_text(self.grapheme()) * self.grapheme_count()
    }

    pub fn grapheme(&self) -> &'static str {
        match self {
            IndentWidth::Tab => "\t",
            IndentWidth::Spaces(_) => " ",
        }
    }
}

pub struct Language {
    pub index: usize,
    pub name: String,
    pub indent_width: IndentWidth,
    pub syntax: Option<Syntax>,
    pub comment: String,
    pub lsp_language_id: Option<String>,
    pub language_server_command: Option<String>,
}

impl Language {
    pub(super) fn new(index: usize, desc: LanguageDesc) -> Self {
        Self {
            index,
            name: desc.name,
            indent_width: desc.indent_width,
            comment: desc.comment,
            lsp_language_id: desc.lsp_language_id,
            language_server_command: desc.language_server_command,
            syntax: desc.syntax.map(|syntax_desc| syntax_desc.get_syntax()),
        }
    }
}
