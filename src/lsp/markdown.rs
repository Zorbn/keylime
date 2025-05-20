use crate::{
    pool::{Pooled, STRING_POOL},
    text::grapheme::{self, CharCursor},
};

pub fn markdown_to_plaintext(markdown: &str) -> Pooled<String> {
    const CODE_BLOCK_DELIMITER: &str = "```";

    let mut result = STRING_POOL.new_item();
    let mut is_in_code_block = false;

    for mut line in markdown.lines() {
        if is_in_code_block {
            if let Some(end) = line.find(CODE_BLOCK_DELIMITER) {
                is_in_code_block = false;
                result.push_str(&line[..end]);
                line = &line[end + CODE_BLOCK_DELIMITER.len()..];
            } else {
                result.push_str(line);
                result.push('\n');
                continue;
            }
        }

        let mut char_cursor = CharCursor::new(0, line.len());

        while char_cursor.index() < line.len() {
            let grapheme = grapheme::at(char_cursor.index(), line);

            if !grapheme::is_whitespace(grapheme) {
                break;
            }

            char_cursor.next_boundary(line);
        }

        let line = &line[char_cursor.index()..];

        if line.starts_with('#') {
            result.push('\n');
        } else if line.starts_with("---") {
            result.push('\n');
            continue;
        } else if line.starts_with(CODE_BLOCK_DELIMITER) {
            is_in_code_block = true;
            continue;
        }

        if !line.is_empty() {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}
