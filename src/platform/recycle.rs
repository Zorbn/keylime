use std::path::Path;

use super::{platform_impl, result::Result};

pub fn recycle(path: &Path) -> Result<()> {
    Ok(platform_impl::recycle::recycle(path)?)
}
