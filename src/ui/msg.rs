use crate::{
    geometry::rect::Rect,
    input::{action::Action, mouse_scroll::MouseScroll, mousebind::Mousebind},
    pool::Pooled,
};

#[derive(Debug, Clone)]
pub enum Msg {
    Resize { width: f32, height: f32 },
    PopupParentResized { bounds: Rect },
    FontChanged,
    GainedFocus,
    LostFocus,
    Mousebind(Mousebind),
    MouseScroll(MouseScroll),
    Grapheme(Pooled<String>),
    Action(Action),
    ShowCompletions,
    HideCompletions,
}
