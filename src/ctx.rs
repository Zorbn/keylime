use crate::{
    config::Config,
    lsp::Lsp,
    platform::{gfx::Gfx, window::Window},
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
    pub time: f64,
}
