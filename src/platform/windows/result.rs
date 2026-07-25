use crate::platform::result::PlatformError;

impl From<windows::core::Error> for PlatformError {
    fn from(value: windows::core::Error) -> Self {
        PlatformError::String(value.message())
    }
}
