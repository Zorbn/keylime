use std::collections::VecDeque;

use crate::{
    config::Config,
    geometry::{rect::Rect, visual_position::VisualPosition},
    input::{
        mods::Mods,
        mouse_button::MouseButton,
        mouse_scroll::MouseScroll,
        mousebind::{Mousebind, MousebindKind},
    },
    platform::gfx::Gfx,
    ui::msg::Msg,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WidgetId {
    index: usize,
    generation: usize,
}

impl WidgetId {
    pub const ROOT: Self = Self {
        index: 0,
        generation: 0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetLayout {
    Horizontal,
    Vertical,
    Tab { index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetScale {
    Fractional(f32),
    Fixed(f32),
}

#[derive(Debug)]
pub struct WidgetSettings {
    pub is_shown: bool,
    pub wants_msgs: bool,
    pub scale: WidgetScale,
    pub layout: WidgetLayout,
    pub popup: Option<Rect>,
    pub main_child_index: Option<usize>,
}

impl Default for WidgetSettings {
    fn default() -> Self {
        Self {
            is_shown: true,
            wants_msgs: true,
            scale: WidgetScale::Fractional(1.0),
            layout: WidgetLayout::Vertical,
            popup: None,
            main_child_index: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Widget {
    bounds: Rect,

    settings: WidgetSettings,
    parent_id: Option<WidgetId>,
    child_ids: Vec<WidgetId>,
    msgs: VecDeque<Msg>,
    did_handle_msgs: bool,
}

#[derive(Debug, Default)]
struct WidgetSlot {
    widget: Widget,
    generation: usize,
}

#[derive(Debug, Clone, Copy)]
struct Border {
    widget_id: WidgetId,
    index: usize,
    precedence: usize,
}

struct GrabbedBorders {
    position: VisualPosition,
    horizontal: Option<Border>,
    vertical: Option<Border>,
    did_drag: bool,
}

#[derive(Default)]
struct HoveredBorders {
    horizontal: Option<Border>,
    vertical: Option<Border>,
}

pub struct Ui {
    widget_slots: Vec<WidgetSlot>,
    hovered_widget_id: WidgetId,
    focus_history: Vec<WidgetId>,
    unused_widget_indices: Vec<usize>,
    grabbed_borders: Option<GrabbedBorders>,
    hovered_borders: HoveredBorders,
    is_dragging: bool,
}

impl Ui {
    pub fn new(width: f32, height: f32) -> Self {
        let root = Widget {
            bounds: Rect::new(0.0, 0.0, width, height),

            settings: Default::default(),
            parent_id: None,
            child_ids: Vec::new(),
            msgs: VecDeque::new(),
            did_handle_msgs: true,
        };

        Self {
            widget_slots: vec![WidgetSlot {
                widget: root,
                generation: 0,
            }],
            hovered_widget_id: WidgetId::ROOT,
            focus_history: Vec::new(),
            unused_widget_indices: Vec::new(),
            grabbed_borders: None,
            hovered_borders: Default::default(),
            is_dragging: false,
        }
    }

    pub fn new_widget(&mut self, parent_id: WidgetId, options: WidgetSettings) -> WidgetId {
        let (widget_id, widget) = if let Some(index) = self.unused_widget_indices.pop() {
            let slot = &mut self.widget_slots[index];

            let widget_id = WidgetId {
                index,
                generation: slot.generation,
            };

            (widget_id, &mut slot.widget)
        } else {
            let index = self.widget_slots.len();

            self.widget_slots.push(WidgetSlot::default());

            let widget = &mut self.widget_slots[index].widget;

            let widget_id = WidgetId {
                index,
                generation: 0,
            };

            (widget_id, widget)
        };

        widget.bounds = Rect::ZERO;

        widget.settings = options;
        widget.parent_id = Some(parent_id);
        widget.child_ids.clear();
        widget.msgs.clear();

        self.widget_mut(parent_id).child_ids.push(widget_id);
        self.update_parent_layout(parent_id);

        widget_id
    }

    pub fn remove_widget(&mut self, widget_id: WidgetId) {
        if widget_id == WidgetId::ROOT {
            return;
        }

        if self.widget_slots[widget_id.index].generation != widget_id.generation {
            return;
        }

        if self.is_focused(widget_id) {
            self.unfocus(widget_id);
        }

        self.widget_mut(widget_id).msgs.clear();

        let parent_id = self.widget(widget_id).parent_id.unwrap_or(WidgetId::ROOT);
        let parent = self.widget_mut(parent_id);

        if let Some(index) = parent
            .child_ids
            .iter()
            .position(|child_id| *child_id == widget_id)
        {
            parent.child_ids.remove(index);
        }

        for i in (0..self.widget(widget_id).child_ids.len()).rev() {
            let child_id = self.widget(widget_id).child_ids[i];

            self.remove_widget(child_id);
        }

        self.widget_slots[widget_id.index].generation += 1;
        self.unused_widget_indices.push(widget_id.index);

        self.update_parent_layout(parent_id);
    }

    pub fn send(&mut self, to_widget_id: WidgetId, msg: Msg) {
        let widget = self.widget_mut(to_widget_id);

        if widget.settings.wants_msgs {
            widget.msgs.push_back(msg);
        } else {
            self.skip(to_widget_id, msg);
        }
    }

    fn send_to_focused_child(&mut self, msg: Msg) {
        let focused_widget_id = self.focused_widget_id();

        if focused_widget_id == WidgetId::ROOT {
            return;
        }

        self.send(focused_widget_id, msg);
    }

    pub fn broadcast(&mut self, to_widget_id: WidgetId, msg: Msg) {
        for i in 0..self.widget(to_widget_id).child_ids.len() {
            let child_id = self.widget(to_widget_id).child_ids[i];
            self.broadcast(child_id, msg.clone());
        }

        self.send(to_widget_id, msg);
    }

    pub fn skip(&mut self, widget_id: WidgetId, msg: Msg) {
        if matches!(msg, Msg::Resize { .. } | Msg::GainedFocus | Msg::LostFocus) {
            return;
        }

        let parent_id = self.widget(widget_id).parent_id.unwrap_or(WidgetId::ROOT);

        if parent_id != WidgetId::ROOT {
            self.send(parent_id, msg);
        }
    }

    pub fn has_msgs(&mut self) -> bool {
        self.widget_slots
            .iter_mut()
            .enumerate()
            .any(|(index, slot)| {
                let has_msgs = !slot.widget.msgs.is_empty();

                if has_msgs {
                    assert!(
                        slot.widget.did_handle_msgs,
                        "Widget didn't handle msgs: {:?}, {:?}!",
                        WidgetId {
                            index,
                            generation: slot.generation
                        },
                        slot.widget.msgs
                    );

                    slot.widget.did_handle_msgs = false;
                }

                has_msgs
            })
    }

    pub fn msg(&mut self, id: WidgetId) -> Option<Msg> {
        let widget = self.widget_mut(id);
        let msg = widget.msgs.pop_front();

        if msg.is_none() {
            widget.did_handle_msgs = true;
        }

        msg
    }

    pub fn receive_msgs(&mut self, gfx: &Gfx) {
        while let Some(msg) = self.msg(WidgetId::ROOT) {
            match msg {
                Msg::Resize { width, height } => {
                    self.update_layout(WidgetId::ROOT, Rect::new(0.0, 0.0, width, height))
                }
                Msg::Mousebind(Mousebind {
                    button: Some(MouseButton::Left),
                    x,
                    y,
                    mods: Mods::NONE,
                    kind: MousebindKind::Press,
                    ..
                }) => {
                    self.is_dragging = true;

                    let position = VisualPosition::new(x, y);

                    let Some(focused_widget_id) = self.get_widget_id_at(position, WidgetId::ROOT)
                    else {
                        continue;
                    };

                    let horizontal =
                        self.get_border(focused_widget_id, WidgetLayout::Horizontal, x, y, gfx);

                    let vertical =
                        self.get_border(focused_widget_id, WidgetLayout::Vertical, x, y, gfx);

                    if horizontal.is_none() && vertical.is_none() {
                        self.focus(focused_widget_id);
                        self.send_to_focused_child(msg);
                        continue;
                    }

                    self.grabbed_borders = Some(GrabbedBorders {
                        position,
                        horizontal,
                        vertical,
                        did_drag: false,
                    });
                }
                Msg::Mousebind(Mousebind {
                    button: Some(MouseButton::Left),
                    mods: Mods::NONE,
                    kind: MousebindKind::Release,
                    ..
                }) => {
                    self.is_dragging = false;

                    let do_send_to_child = self
                        .grabbed_borders
                        .take()
                        .is_none_or(|click| !click.did_drag);

                    if do_send_to_child {
                        self.send_to_focused_child(msg);
                    }
                }
                Msg::Mousebind(Mousebind {
                    x,
                    y,
                    kind: MousebindKind::Move,
                    ..
                }) => {
                    let hit_id = self
                        .get_widget_id_at(VisualPosition::new(x, y), WidgetId::ROOT)
                        .unwrap_or(WidgetId::ROOT);

                    self.hovered_borders.horizontal =
                        self.get_border(hit_id, WidgetLayout::Horizontal, x, y, gfx);

                    self.hovered_borders.vertical =
                        self.get_border(hit_id, WidgetLayout::Vertical, x, y, gfx);

                    let Some(GrabbedBorders {
                        position,
                        horizontal: horizontal_dragged_border,
                        vertical: vertical_dragged_border,
                        did_drag,
                    }) = &mut self.grabbed_borders
                    else {
                        if self.is_dragging {
                            self.send_to_focused_child(msg);
                        } else if hit_id != WidgetId::ROOT {
                            self.send(hit_id, msg);
                        }

                        continue;
                    };

                    *did_drag = true;

                    let dx = x - position.x;
                    let dy = y - position.y;

                    *position = VisualPosition::new(x, y);

                    let horizontal_dragged_border = *horizontal_dragged_border;
                    let vertical_dragged_border = *vertical_dragged_border;

                    self.handle_dragged_border(horizontal_dragged_border, dx, dy, gfx);
                    self.handle_dragged_border(vertical_dragged_border, dx, dy, gfx);

                    let id_to_layout = horizontal_dragged_border
                        .zip(vertical_dragged_border)
                        .map(|(h, v)| if h.precedence > v.precedence { h } else { v })
                        .or(horizontal_dragged_border)
                        .or(vertical_dragged_border)
                        .map(|border| border.widget_id)
                        .unwrap();

                    let bounds = self.bounds(id_to_layout);
                    self.update_layout(id_to_layout, bounds);
                }
                Msg::MouseScroll(MouseScroll { x, y, .. }) => {
                    let hit_id = self
                        .get_widget_id_at(VisualPosition::new(x, y), WidgetId::ROOT)
                        .unwrap_or(WidgetId::ROOT);

                    if hit_id != WidgetId::ROOT {
                        self.send(hit_id, msg);
                    }
                }
                _ => self.send_to_focused_child(msg),
            }
        }
    }

    fn handle_dragged_border(
        &mut self,
        dragged_border: Option<Border>,
        dx: f32,
        dy: f32,
        gfx: &Gfx,
    ) -> Option<()> {
        let dragged_border = dragged_border?;
        let parent = self.get_widget(dragged_border.widget_id)?;

        if dragged_border.index + 1 >= parent.child_ids.len() {
            return None;
        }

        let bounds = parent.bounds;

        let (delta, size) = match parent.settings.layout {
            WidgetLayout::Horizontal => (dx, bounds.width),
            WidgetLayout::Vertical => (dy, bounds.height),
            WidgetLayout::Tab { .. } => return None,
        };

        let total_scale = self.widget_total_scale(dragged_border.widget_id);
        let min_scale = Self::border_radius(gfx) * 2.0 / size * total_scale;
        let delta = delta / size * total_scale;

        self.drag_widget(
            dragged_border.widget_id,
            dragged_border.index,
            delta,
            min_scale,
        );

        Some(())
    }

    fn drag_widget(&mut self, parent_id: WidgetId, index: usize, delta: f32, min_scale: f32) {
        let parent = self.widget(parent_id);
        let first_child_id = parent.child_ids[index];
        let second_child_id = parent.child_ids[index + 1];

        let allowed_delta =
            self.allowed_drag_delta(first_child_id, second_child_id, delta, min_scale);

        let remaining_delta = delta - allowed_delta;

        if let Some(index) = index
            .checked_add_signed(delta.signum() as isize)
            .filter(|index| *index + 1 < parent.child_ids.len())
        {
            self.drag_widget(parent_id, index, remaining_delta, min_scale);
        }

        let allowed_delta =
            self.allowed_drag_delta(first_child_id, second_child_id, delta, min_scale);

        if let WidgetScale::Fractional(scale) = &mut self.widget_mut(first_child_id).settings.scale
        {
            *scale += allowed_delta;
        }

        if let WidgetScale::Fractional(scale) = &mut self.widget_mut(second_child_id).settings.scale
        {
            *scale -= allowed_delta;
        }
    }

    fn allowed_drag_delta(
        &self,
        first_child_id: WidgetId,
        second_child_id: WidgetId,
        delta: f32,
        min_scale: f32,
    ) -> f32 {
        let WidgetScale::Fractional(first_scale) = self.widget(first_child_id).settings.scale
        else {
            return 0.0;
        };

        let WidgetScale::Fractional(second_scale) = self.widget(second_child_id).settings.scale
        else {
            return 0.0;
        };

        if delta < 0.0 {
            delta.max((min_scale - first_scale).min(0.0))
        } else {
            delta.min((second_scale - min_scale).max(0.0))
        }
    }

    fn get_border(
        &self,
        mut widget_id: WidgetId,
        layout: WidgetLayout,
        x: f32,
        y: f32,
        gfx: &Gfx,
    ) -> Option<Border> {
        if matches!(layout, WidgetLayout::Tab { .. }) {
            return None;
        }

        let mut precedence = 0;

        while widget_id != WidgetId::ROOT {
            let parent_id = self.widget(widget_id).parent_id.unwrap_or(WidgetId::ROOT);
            let parent = self.widget(parent_id);

            if parent.settings.layout != layout {
                widget_id = parent_id;
                precedence += 1;
                continue;
            }

            let mut divider_x = parent.bounds.x;
            let mut divider_y = parent.bounds.y;

            let border_children = parent
                .child_ids
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, child_id)| self.is_part_of_layout(*child_id));

            let mut matched_border = None;

            for (index, child_id) in border_children {
                let child = self.widget(child_id);

                if !matches!(child.settings.scale, WidgetScale::Fractional(..)) {
                    matched_border = None;
                    continue;
                }

                if matched_border.is_some() {
                    return matched_border;
                }

                match parent.settings.layout {
                    WidgetLayout::Horizontal => divider_x = child.bounds.right(),
                    WidgetLayout::Vertical => divider_y = child.bounds.bottom(),
                    WidgetLayout::Tab { .. } => {}
                };

                let did_grab_horizontal = parent.settings.layout == WidgetLayout::Horizontal
                    && (x - divider_x).abs() < Self::border_radius(gfx);
                let did_grab_vertical = parent.settings.layout == WidgetLayout::Vertical
                    && (y - divider_y).abs() < Self::border_radius(gfx);

                if did_grab_horizontal || did_grab_vertical {
                    matched_border = Some(Border {
                        widget_id: parent_id,
                        index,
                        precedence,
                    });
                }
            }

            widget_id = parent_id;
            precedence += 1;
        }

        None
    }

    fn border_radius(gfx: &Gfx) -> f32 {
        gfx.border_width() * 2.0
    }

    pub fn draw(&mut self, config: &Config, gfx: &mut Gfx) {
        let (horizontal, vertical, is_dragged) = if let Some(GrabbedBorders {
            horizontal,
            vertical,
            ..
        }) = self.grabbed_borders
        {
            (horizontal, vertical, true)
        } else {
            (
                self.hovered_borders.horizontal,
                self.hovered_borders.vertical,
                false,
            )
        };

        if horizontal.is_none() && vertical.is_none() {
            return;
        }

        gfx.begin(None);

        self.draw_border(horizontal, is_dragged, config, gfx);
        self.draw_border(vertical, is_dragged, config, gfx);

        gfx.end();
    }

    fn draw_border(
        &mut self,
        border: Option<Border>,
        is_dragged: bool,
        config: &Config,
        gfx: &mut Gfx,
    ) {
        let Some(border) = border else {
            return;
        };

        let parent = &self.widget(border.widget_id);

        let rect = match parent.settings.layout {
            WidgetLayout::Horizontal => {
                let x = self.bounds(parent.child_ids[border.index]).right();

                Rect::new(
                    x - Self::border_radius(gfx),
                    parent.bounds.y,
                    Self::border_radius(gfx) * 2.0,
                    parent.bounds.height,
                )
            }
            WidgetLayout::Vertical => {
                let y = self.bounds(parent.child_ids[border.index]).bottom();

                Rect::new(
                    parent.bounds.x,
                    y - Self::border_radius(gfx),
                    parent.bounds.width,
                    Self::border_radius(gfx) * 2.0,
                )
            }
            WidgetLayout::Tab { .. } => return,
        };

        gfx.add_rect(
            rect,
            if is_dragged {
                config.theme.keyword
            } else {
                config.theme.border
            },
        );
    }

    fn update_layout(&mut self, widget_id: WidgetId, bounds: Rect) {
        let widget = self.widget_mut(widget_id);
        widget.bounds = bounds;

        let child_count = widget.child_ids.len();
        let total_scale = self.widget_total_scale(widget_id);

        let mut next_child_x = bounds.x;
        let mut next_child_y = bounds.y;

        for i in 0..child_count {
            let widget = self.widget(widget_id);
            let child_id = widget.child_ids[i];

            if !self.is_part_of_layout(child_id) {
                continue;
            }

            let child = self.widget(child_id);
            let child_x = next_child_x;
            let child_y = next_child_y;

            let mut child_width = bounds.width;
            let mut child_height = bounds.height;

            match widget.settings.layout {
                WidgetLayout::Horizontal => {
                    child_width = match child.settings.scale {
                        WidgetScale::Fractional(scale) => {
                            (bounds.width * scale / total_scale).ceil()
                        }
                        WidgetScale::Fixed(scale) => scale,
                    }
                    .min(bounds.right() - child_x);

                    next_child_x += child_width;
                }
                WidgetLayout::Vertical => {
                    child_height = match child.settings.scale {
                        WidgetScale::Fractional(scale) => {
                            (bounds.height * scale / total_scale).ceil()
                        }
                        WidgetScale::Fixed(scale) => scale,
                    }
                    .min(bounds.bottom() - child_y);

                    next_child_y += child_height;
                }
                WidgetLayout::Tab { index } => {
                    let widget = self.widget_mut(widget_id);
                    let index = index.min(widget.child_ids.len().saturating_sub(1));
                    widget.settings.layout = WidgetLayout::Tab { index };
                }
            }

            self.update_layout(
                child_id,
                Rect::new(child_x, child_y, child_width, child_height),
            );

            self.send(
                child_id,
                Msg::Resize {
                    width: child_width,
                    height: child_height,
                },
            );
        }
    }

    fn widget_total_scale(&self, widget_id: WidgetId) -> f32 {
        let widget = self.widget(widget_id);

        let size = match widget.settings.layout {
            WidgetLayout::Horizontal => widget.bounds.width,
            WidgetLayout::Vertical => widget.bounds.height,
            WidgetLayout::Tab { .. } => return 1.0,
        };

        let mut total_fractional_scale = 0.0;
        let mut total_fixed_scale = 0.0;

        for child_id in &self.widget(widget_id).child_ids {
            if !self.is_part_of_layout(*child_id) {
                continue;
            }

            let child = self.widget(*child_id);
            match child.settings.scale {
                WidgetScale::Fractional(scale) => total_fractional_scale += scale,
                WidgetScale::Fixed(scale) => total_fixed_scale += scale,
            }
        }

        total_fractional_scale / ((size - total_fixed_scale) / size)
    }

    fn get_widget_id_at(&self, position: VisualPosition, widget_id: WidgetId) -> Option<WidgetId> {
        if !self.is_visible(widget_id) {
            return None;
        }

        let widget = self.widget(widget_id);

        for child_id in widget.child_ids.iter() {
            if let Some(widget_id) = self.get_widget_id_at(position, *child_id) {
                return Some(widget_id);
            }
        }

        (widget.bounds.contains_position(position)).then_some(widget_id)
    }

    fn focused_widget_id(&self) -> WidgetId {
        self.focus_history.last().copied().unwrap_or_default()
    }

    pub fn focus(&mut self, widget_id: WidgetId) {
        let widget = self.widget(widget_id);

        if let WidgetLayout::Tab { index } = widget.settings.layout {
            if let Some(child_id) = widget
                .child_ids
                .get(index)
                .copied()
                .filter(|child_id| self.is_part_of_layout(*child_id))
            {
                self.focus(child_id);
                return;
            }
        }

        if let Some(child_id) = widget.settings.main_child_index.and_then(|index| {
            widget
                .child_ids
                .get(index)
                .copied()
                .filter(|child_id| self.is_part_of_layout(*child_id))
        }) {
            self.focus(child_id);
            return;
        }

        for child_id in &widget.child_ids {
            if self.is_part_of_layout(*child_id) {
                self.focus(*child_id);
                return;
            }
        }

        if !self.is_focused(widget_id) {
            self.send(self.focused_widget_id(), Msg::LostFocus);
            self.send(widget_id, Msg::GainedFocus);
        }

        self.remove_from_focused(widget_id);
        self.show(widget_id);
        self.focus_history.push(widget_id);
    }

    pub fn unfocus(&mut self, widget_id: WidgetId) {
        if !self.is_focused(widget_id) {
            return;
        }

        self.focus_history.pop();

        self.send(widget_id, Msg::LostFocus);
        self.send(self.focused_widget_id(), Msg::GainedFocus);
    }

    pub fn unfocus_hierarchy(&mut self, widget_id: WidgetId) {
        while !self.focus_history.is_empty() && self.is_in_focused_hierarchy(widget_id) {
            self.focus_history.pop();
        }
    }

    fn remove_from_focused(&mut self, widget_id: WidgetId) {
        let index = self
            .focus_history
            .iter()
            .position(|focused_id| *focused_id == widget_id);

        if let Some(index) = index {
            self.focus_history.remove(index);
        }
    }

    fn hover(&mut self, widget_id: WidgetId) {
        self.hovered_widget_id = widget_id;
    }

    pub fn show(&mut self, widget_id: WidgetId) {
        let widget = self.widget_mut(widget_id);

        if widget.settings.is_shown {
            return;
        }

        widget.settings.is_shown = true;
        self.update_parent_layout(widget_id);
    }

    pub fn hide(&mut self, widget_id: WidgetId) {
        self.remove_from_focused(widget_id);

        let widget = self.widget_mut(widget_id);

        if !widget.settings.is_shown {
            return;
        }

        widget.settings.is_shown = false;
        self.update_parent_layout(widget_id);
    }

    pub fn set_shown(&mut self, widget_id: WidgetId, is_shown: bool) {
        if is_shown {
            self.show(widget_id);
        } else {
            self.hide(widget_id);
        }
    }

    pub fn is_focused(&self, widget_id: WidgetId) -> bool {
        self.focused_widget_id() == widget_id
    }

    pub fn is_in_focused_hierarchy(&self, widget_id: WidgetId) -> bool {
        // let widget = self.widget(widget_id);

        // TODO:
        // if widget.settings.is_component && widget.settings.is_shown {
        //     if let Some(parent_id) = widget.parent_id {
        //         return self.is_in_focused_hierarchy(parent_id);
        //     }
        // }

        let mut focused_hierarchy_id = self.focused_widget_id();

        loop {
            if focused_hierarchy_id == widget_id {
                return true;
            }

            if let Some(parent_id) = self.widget(focused_hierarchy_id).parent_id {
                focused_hierarchy_id = parent_id;
            } else {
                return false;
            }
        }
    }

    fn get_widget(&self, widget_id: WidgetId) -> Option<&Widget> {
        self.widget_slots
            .get(widget_id.index)
            .filter(|slot| slot.generation == widget_id.generation)
            .map(|slot| &slot.widget)
    }

    fn widget(&self, widget_id: WidgetId) -> &Widget {
        let slot = &self.widget_slots[widget_id.index];
        assert!(slot.generation == widget_id.generation);

        &slot.widget
    }

    fn widget_mut(&mut self, widget_id: WidgetId) -> &mut Widget {
        let slot = &mut self.widget_slots[widget_id.index];
        assert!(slot.generation == widget_id.generation);

        &mut slot.widget
    }

    pub fn bounds(&self, widget_id: WidgetId) -> Rect {
        self.widget(widget_id).bounds
    }

    pub fn child_ids(&self, widget_id: WidgetId) -> &[WidgetId] {
        &self.widget(widget_id).child_ids
    }

    pub fn move_child(&mut self, child_id: WidgetId, to_index: usize) {
        let Some(parent_id) = self.widget(child_id).parent_id else {
            return;
        };

        let parent = self.widget_mut(parent_id);

        let Some(from_index) = parent
            .child_ids
            .iter()
            .position(|widget_id| *widget_id == child_id)
        else {
            return;
        };

        parent.child_ids.remove(from_index);
        parent.child_ids.insert(to_index, child_id);
    }

    pub fn layout(&self, widget_id: WidgetId) -> WidgetLayout {
        self.widget(widget_id).settings.layout
    }

    pub fn set_layout(&mut self, widget_id: WidgetId, layout: WidgetLayout) {
        let widget = self.widget_mut(widget_id);

        widget.settings.layout = layout;

        self.update_parent_layout(widget_id);
    }

    pub fn set_scale(&mut self, widget_id: WidgetId, scale: WidgetScale) {
        self.widget_mut(widget_id).settings.scale = scale;
        self.update_parent_layout(widget_id);
    }

    fn update_parent_layout(&mut self, widget_id: WidgetId) {
        let parent_id = self.widget(widget_id).parent_id.unwrap_or(WidgetId::ROOT);
        let parent = self.widget(parent_id);
        let parent_bounds = parent.bounds;

        self.update_layout(parent_id, parent_bounds);
    }

    pub fn is_hovered(&self, widget_id: WidgetId) -> bool {
        self.hovered_widget_id == widget_id
    }

    pub fn is_visible(&self, widget_id: WidgetId) -> bool {
        let widget = self.widget(widget_id);

        if !widget.settings.is_shown {
            return false;
        }

        if let Some(parent_id) = widget.parent_id {
            if let WidgetLayout::Tab { index } = self.layout(parent_id) {
                if self.child_ids(parent_id).get(index) != Some(&widget_id) {
                    return false;
                }
            }

            self.is_visible(parent_id)
        } else {
            true
        }
    }

    fn is_part_of_layout(&self, widget_id: WidgetId) -> bool {
        let widget = self.widget(widget_id);

        widget.settings.popup.is_none() && widget.settings.is_shown
    }
}
