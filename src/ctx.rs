use crate::{
    config::Config,
    editor_buffers::EditorBuffers,
    lsp::Lsp,
    platform::{gfx::Gfx, window::Window},
};

macro_rules! ctx_with_time {
    ($ctx:ident, $time:expr) => {
        &mut crate::ctx::Ctx {
            window: $ctx.window,
            gfx: $ctx.gfx,
            config: $ctx.config,
            buffers: $ctx.buffers,
            lsp: $ctx.lsp,
            time: $time,
        }
    };
}

pub(crate) use ctx_with_time;

pub struct Ctx<'a> {
    pub window: &'a mut Window,
    pub gfx: &'a mut Gfx,
    pub config: &'a Config,
    pub buffers: &'a mut EditorBuffers,
    pub lsp: &'a mut Lsp,
    pub time: f32,
}
