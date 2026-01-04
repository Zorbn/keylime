#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum MouseScrollKind {
    Start,
    Continue,
    Stop,
    Instant,
}

#[derive(Clone, Copy)]
pub struct MouseScroll {
    pub delta: f32,
    pub is_horizontal: bool,
    pub kind: MouseScrollKind,
    pub x: f32,
    pub y: f32,
}
