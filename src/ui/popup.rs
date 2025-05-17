use crate::{
    ctx::Ctx,
    geometry::{rect::Rect, sides::Sides, visual_position::VisualPosition},
    platform::gfx::Gfx,
    pool::{Pooled, STRING_POOL},
    ui::core::WidgetSettings,
};

use super::{
    color::Color,
    core::{Ui, WidgetId},
};

#[derive(Debug, PartialEq, Eq)]
pub enum PopupAlignment {
    TopLeft,
    Above,
}

pub struct Popup {
    pub text: Pooled<String>,
    widget_id: WidgetId,
}

impl Popup {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        Self {
            text: STRING_POOL.new_item(),
            widget_id: ui.new_widget(
                parent_id,
                WidgetSettings {
                    is_visible: false,
                    ..Default::default()
                },
            ),
        }
    }

    pub fn layout(&mut self, position: VisualPosition, alignment: PopupAlignment, ctx: &mut Ctx) {
        let gfx = &mut ctx.gfx;

        let mut bounds = Rect::ZERO;

        for line in self.text.lines() {
            bounds.height += gfx.line_height();

            let line_width = gfx.measure_text(line) as f32 * gfx.glyph_width();
            bounds.width = bounds.width.max(line_width);
        }

        let margin = Self::margin(gfx);
        bounds = bounds.add_margin(margin);

        bounds.x = position.x;
        bounds.y = position.y;

        if alignment == PopupAlignment::Above {
            bounds.x -= margin;
            bounds.y -= bounds.height;

            if bounds.right() > gfx.width() - margin {
                bounds.x -= bounds.right() - (gfx.width() - margin);
            }

            bounds.x = bounds.x.max(margin);

            if bounds.bottom() > gfx.height() - margin {
                bounds.y -= bounds.bottom() - (gfx.height() - margin);
            }

            bounds.y = bounds.y.max(margin);
        }

        ctx.ui.widget_mut(self.widget_id).bounds = bounds;
    }

    pub fn draw(&self, foreground: Color, ctx: &mut Ctx) {
        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;

        if !ctx.ui.is_visible(self.widget_id) {
            return;
        }

        let bounds = ctx.ui.widget(self.widget_id).bounds;

        gfx.begin(Some(bounds));

        gfx.add_bordered_rect(
            bounds.unoffset_by(bounds),
            Sides::ALL,
            theme.background,
            theme.border,
        );

        let margin = Self::margin(gfx);

        for (y, line) in self.text.lines().enumerate() {
            let y = y as f32 * gfx.line_height() + gfx.line_padding_y() + margin;

            gfx.add_text(line, margin, y, foreground);
        }

        gfx.end();
    }

    pub fn hide(&mut self, ui: &mut Ui) {
        ui.hide(self.widget_id());
    }

    pub fn show(&mut self, text: &str, ui: &mut Ui) {
        self.text.clear();
        self.text.push_str(text);
        ui.show(self.widget_id());
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    fn margin(gfx: &mut Gfx) -> f32 {
        gfx.glyph_width()
    }
}
