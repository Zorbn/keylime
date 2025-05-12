use std::{
    iter::Peekable,
    ops::Deref,
    path::{Path, PathBuf},
    str::{self, Chars},
};

use crate::{
    normalizable::Normalizable,
    pool::{Pooled, PATH_POOL, STRING_POOL},
};

const URI_SCHEME: &str = "file:///";

pub fn uri_to_path(uri: &str) -> Option<Pooled<PathBuf>> {
    if !uri.starts_with(URI_SCHEME) {
        return None;
    }

    let mut chars = uri[URI_SCHEME.len()..].chars().peekable();
    let mut c = chars.next();
    let mut result = STRING_POOL.new_item();

    if let Some(first_char) = c {
        if first_char.is_ascii_alphabetic() && chars.peek() == Some(&':') {
            // This is a drive letter.
            result.push(first_char.to_ascii_uppercase());
            c = chars.next();
        } else {
            // No drive letter, add a root slash instead.
            result.push('/');
        }
    }

    while let Some(next_char) = c {
        result.push(match next_char {
            '%' => decode_uri_char(&mut chars)?,
            _ => next_char,
        });

        c = chars.next()
    }

    Some(PATH_POOL.init_item(|path| path.push(result.deref())))
}

fn decode_uri_char(chars: &mut Peekable<Chars>) -> Option<char> {
    let first_digit = chars.next()?;
    let second_digit = chars.next()?;

    if !first_digit.is_ascii_hexdigit() || !second_digit.is_ascii_hexdigit() {
        return None;
    }

    let digits = &[first_digit as u8, second_digit as u8];
    let hex_string = str::from_utf8(digits).ok()?;

    u8::from_str_radix(hex_string, 16)
        .ok()
        .map(|value| value as char)
}

pub fn path_to_uri(path: &Path) -> Pooled<String> {
    assert!(path.is_normal());

    let mut result = STRING_POOL.new_item();
    result.push_str(URI_SCHEME);

    if let Some(parent) = path.parent() {
        for component in parent {
            let Some(component) = component.to_str() else {
                continue;
            };

            if matches!(component, "/" | "\\") {
                continue;
            }

            encode_path_component(component, &mut result);
            result.push('/');
        }
    }

    if let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) {
        encode_path_component(file_name, &mut result);
    }

    result
}

fn encode_path_component(component: &str, result: &mut String) {
    for c in component.chars() {
        match c {
            ' ' => result.push_str("%20"),
            '\\' => result.push('/'),
            _ => result.push(c),
        }
    }
}
