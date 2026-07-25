use std::fmt::Display;

#[derive(Debug)]
#[allow(dead_code)]
pub enum PlatformError {
    Str(&'static str),
    String(String),
}

impl Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::Str(str) => str.fmt(f),
            PlatformError::String(string) => string.fmt(f),
        }
    }
}

pub type Result<T> = std::result::Result<T, PlatformError>;
