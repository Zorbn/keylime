use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect, sides::Sides, visual_position::VisualPosition},
    platform::gfx::Gfx,
    pool::STRING_POOL,
    text::doc::{Doc, DocFlags},
    ui::core::WidgetSettings,
};

use super::{
    color::Color,
    core::{Ui, WidgetId},
    slot_list::SlotId,
    tab::Tab,
};

#[derive(Debug, PartialEq, Eq)]
pub enum PopupAlignment {
    TopLeft,
    Above,
}

const MAX_LINES: usize = 10;

pub struct Popup {
    tab: Tab,
    doc: Doc,
    widget_id: WidgetId,
}

impl Popup {
    pub fn new(parent_id: WidgetId, ui: &mut Ui) -> Self {
        Self {
            tab: Tab::new(SlotId::ZERO),
            doc: Doc::new(None, None, DocFlags::RAW),
            widget_id: ui.new_widget(
                parent_id,
                WidgetSettings {
                    is_shown: false,
                    ..Default::default()
                },
            ),
        }
    }

    pub fn layout(&mut self, position: VisualPosition, alignment: PopupAlignment, ctx: &mut Ctx) {
        let gfx = &mut ctx.gfx;

        let mut bounds = Rect::ZERO;
        bounds.height = self.doc.lines().len().min(MAX_LINES) as f32 * gfx.line_height();

        for line in self.doc.lines() {
            let line_width = gfx.measure_text(line) as f32 * gfx.glyph_width()
                + gfx.line_padding_x()
                + Tab::cursor_width(gfx);

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

        self.tab
            .layout(Rect::ZERO, bounds.add_margin(-margin), &self.doc, gfx);

        ctx.ui.widget_mut(self.widget_id).bounds = bounds;
    }

    pub fn is_animating(&self) -> bool {
        self.tab.is_animating()
    }

    pub fn update(&mut self, ctx: &mut Ctx) {
        ctx.ui
            .keybind_handler(self.widget_id, ctx.window)
            .drain(ctx.window);

        ctx.ui
            .grapheme_handler(self.widget_id, ctx.window)
            .drain(ctx.window);

        self.tab.update(self.widget_id, &mut self.doc, ctx);
    }

    pub fn update_camera(&mut self, ctx: &mut Ctx, dt: f32) {
        self.tab.update_camera(self.widget_id, &self.doc, ctx, dt);
    }

    pub fn draw(&mut self, foreground: Color, ctx: &mut Ctx) {
        if !ctx.ui.is_visible(self.widget_id) {
            return;
        }

        let gfx = &mut ctx.gfx;
        let theme = &ctx.config.theme;
        let bounds = ctx.ui.widget(self.widget_id).bounds;

        gfx.begin(Some(bounds));

        gfx.add_bordered_rect(
            bounds.unoffset_by(bounds),
            Sides::ALL,
            theme.background,
            theme.border,
        );

        gfx.end();

        if let Some(language) = ctx.config.get_language("md") {
            self.tab.update_highlights(language, &mut self.doc, ctx.gfx);
        }

        self.tab
            .draw(None, &mut self.doc, ctx.ui.is_focused(self.widget_id), ctx);
    }

    pub fn hide(&self, ui: &mut Ui) {
        ui.hide(self.widget_id());
    }

    pub fn show(&mut self, text: &str, ctx: &mut Ctx) {
        if ctx.ui.is_visible(self.widget_id) {
            let mut current_text = STRING_POOL.new_item();
            self.doc
                .collect_string(Position::ZERO, self.doc.end(), &mut current_text);

            if current_text.as_str() == text {
                return;
            }
        }

        self.doc.clear(ctx);
        self.doc.insert(Position::ZERO, text, ctx);

        self.tab.camera.reset();

        ctx.ui.show(self.widget_id);
    }

    pub fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    fn margin(gfx: &Gfx) -> f32 {
        gfx.glyph_width()
    }
}
