use serde::Deserialize;

use crate::{platform::gfx::Gfx, text::syntax::Syntax};

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
    pub indent_width: IndentWidth,
    pub syntax: Option<Syntax>,
    pub comment: String,
    pub language_server_command: Option<String>,
}
