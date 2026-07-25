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
            Self::Str(str) => str.fmt(f),
            Self::String(string) => string.fmt(f),
        }
    }
}

impl From<&'static str> for PlatformError {
    fn from(value: &'static str) -> Self {
        Self::Str(value)
    }
}

pub type Result<T> = std::result::Result<T, PlatformError>;
