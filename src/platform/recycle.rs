use std::path::Path;

use super::platform_impl::{self, result::Result};

pub fn recycle(path: &Path) -> Result<()> {
    platform_impl::recycle::recycle(path)
}
