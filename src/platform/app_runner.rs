use super::{platform_impl, result::Result};

pub fn run_app() -> Result<()> {
    Ok(platform_impl::app_runner::run_app()?)
}
