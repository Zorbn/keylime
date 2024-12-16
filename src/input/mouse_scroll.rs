#[derive(Clone, Copy)]
pub struct MouseScroll {
    pub delta: f32,
    pub is_horizontal: bool,
    pub is_precise: bool,
    pub x: f32,
    pub y: f32,
}
