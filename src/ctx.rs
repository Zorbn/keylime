use std::path::PathBuf;

use crate::{
    config::Config,
    lsp::Lsp,
    platform::{gfx::Gfx, window::Window},
    pool::Pooled,
    ui::core::Ui,
};

macro_rules! ctx_with_time {
    ($ctx:ident, $time:expr) => {
        &mut crate::ctx::Ctx {
            window: $ctx.window,
            gfx: $ctx.gfx,
            ui: $ctx.ui,
            config: $ctx.config,
            lsp: $ctx.lsp,
            current_dir: $ctx.current_dir,
            time: $time,
        }
    };
}

pub(crate) use ctx_with_time;

pub struct Ctx<'a> {
    pub window: &'a mut Window,
    pub gfx: &'a mut Gfx,
    pub ui: &'a mut Ui,
    pub config: &'a Config,
    pub lsp: &'a mut Lsp,
    pub current_dir: &'a mut Pooled<PathBuf>,
    pub time: f64,
}
