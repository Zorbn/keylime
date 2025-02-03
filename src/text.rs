pub mod action_history;
mod char_category;
pub mod cursor;
pub mod cursor_index;
pub mod doc;
pub mod line_pool;
mod pattern;
pub mod selection;
pub mod syntax;
pub mod syntax_highlighter;
pub mod tokenizer;
mod trie;
pub mod utf32;

macro_rules! text_trait {
    () => {
        impl IntoIterator<Item = impl std::borrow::Borrow<char>> + Clone
    };
    (move) => {
        impl IntoIterator<Item = impl std::borrow::Borrow<char>>
    };
}

pub(crate) use text_trait;
