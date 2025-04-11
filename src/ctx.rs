use crate::{
    config::Config,
    editor_buffers::EditorBuffers,
    platform::{gfx::Gfx, window::Window},
};

pub struct Ctx<'a> {
    pub window: &'a mut Window,
    pub gfx: &'a mut Gfx,
    pub config: &'a Config,
    pub buffers: &'a mut EditorBuffers,
}
