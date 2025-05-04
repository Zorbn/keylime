use crate::{
    config::theme::Theme,
    geometry::{rect::Rect, sides::Sides, visual_position::VisualPosition},
    platform::gfx::Gfx,
};

#[derive(Debug, PartialEq, Eq)]
pub enum PopupAlignment {
    TopLeft,
    Above,
}

pub fn draw_popup(
    message: &str,
    position: VisualPosition,
    alignment: PopupAlignment,
    theme: &Theme,
    gfx: &mut Gfx,
) -> Rect {
    let mut popup_bounds = Rect::ZERO;

    for line in message.lines() {
        popup_bounds.height += gfx.line_height();

        let line_width = gfx.measure_text(line) as f32 * gfx.glyph_width();
        popup_bounds.width = popup_bounds.width.max(line_width);
    }

    let margin = gfx.glyph_width();
    popup_bounds = popup_bounds.add_margin(margin);

    popup_bounds.x = position.x;
    popup_bounds.y = position.y;

    if alignment == PopupAlignment::Above {
        popup_bounds.x -= margin;
        popup_bounds.y -= popup_bounds.height;

        if popup_bounds.right() > gfx.width() - margin {
            popup_bounds.x -= popup_bounds.right() - (gfx.width() - margin);
        }

        popup_bounds.x = popup_bounds.x.max(margin);
    }

    gfx.begin(Some(popup_bounds));

    gfx.add_bordered_rect(
        popup_bounds.unoffset_by(popup_bounds),
        Sides::ALL,
        theme.background,
        theme.border,
    );

    for (y, line) in message.lines().enumerate() {
        let y = y as f32 * gfx.line_height() + gfx.line_padding() + margin;

        gfx.add_text(line, margin, y, theme.normal);
    }

    gfx.end();

    popup_bounds
}
