use super::{rect::Rect, visual_position::VisualPosition};

#[derive(Debug)]
pub struct Quad {
    pub top_left: VisualPosition,
    pub top_right: VisualPosition,
    pub bottom_left: VisualPosition,
    pub bottom_right: VisualPosition,
}

impl Quad {
    pub fn offset_by(&self, rect: Rect) -> Self {
        let delta = VisualPosition::new(rect.x, rect.y);

        Self {
            top_left: (self.top_left + delta).floor(),
            top_right: (self.top_right + delta).floor(),
            bottom_left: (self.bottom_left + delta).floor(),
            bottom_right: (self.bottom_right + delta).floor(),
        }
    }
}

impl From<Rect> for Quad {
    fn from(
        Rect {
            x,
            y,
            width,
            height,
        }: Rect,
    ) -> Self {
        Self {
            top_left: VisualPosition::new(x, y),
            top_right: VisualPosition::new(x + width, y),
            bottom_left: VisualPosition::new(x, y + height),
            bottom_right: VisualPosition::new(x + width, y + height),
        }
    }
}
