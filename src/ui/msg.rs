use crate::{
    input::{action::Action, mouse_scroll::MouseScroll, mousebind::Mousebind},
    pool::Pooled,
};

#[derive(Debug, Clone)]
pub enum Msg {
    Resize { width: f32, height: f32 },
    FontChanged,
    GainedFocus,
    LostFocus,
    Mousebind(Mousebind),
    MouseScroll(MouseScroll),
    Grapheme(Pooled<String>),
    Action(Action),
}
