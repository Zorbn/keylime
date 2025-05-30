pub mod action_history;
pub mod compare;
pub mod cursor;
pub mod cursor_index;
pub mod doc;
pub mod grapheme;
pub mod grapheme_category;
mod pattern;
pub mod selection;
pub mod syntax;
pub mod syntax_highlighter;
pub mod tokenizer;
mod trie;

#[cfg(test)]
mod tests;
