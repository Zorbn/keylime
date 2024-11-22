use crate::{
    config::Config,
    geometry::{rect::Rect, visual_position::VisualPosition},
    input::{
        key::Key,
        keybind::{Keybind, MOD_ALT, MOD_CTRL, MOD_CTRL_ALT},
        mouse_button::MouseButton,
        mousebind::Mousebind,
    },
    platform::{gfx::Gfx, window::Window},
    temp_buffer::TempBuffer,
    text::line_pool::LinePool,
    ui::command_palette::CommandPalette,
};

use super::{doc_list::DocList, pane::Pane};

pub struct Editor {
    doc_list: DocList,
    // There should always be at least one pane.
    panes: Vec<Pane>,
    focused_pane_index: usize,
    bounds: Rect,
}

impl Editor {
    pub fn new(config: &Config, line_pool: &mut LinePool, time: f32) -> Self {
        let mut editor = Self {
            doc_list: DocList::new(),
            panes: Vec::new(),
            focused_pane_index: 0,
            bounds: Rect::zero(),
        };

        editor
            .panes
            .push(Pane::new(&mut editor.doc_list, config, line_pool, time));

        editor
    }

    pub fn is_animating(&self) -> bool {
        for pane in &self.panes {
            if pane.is_animating() {
                return true;
            }
        }

        false
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        self.bounds = bounds;

        let mut pane_bounds = Rect::new(
            self.bounds.x,
            self.bounds.y,
            (self.bounds.width / self.panes.len() as f32).ceil(),
            self.bounds.height,
        );

        for pane in &mut self.panes {
            pane.layout(pane_bounds, gfx, &mut self.doc_list);
            pane_bounds.x += pane_bounds.width;
        }
    }

    pub fn update(
        &mut self,
        command_palette: &mut CommandPalette,
        window: &mut Window,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        let mut mousebind_handler = window.get_mousebind_handler();

        while let Some(mousebind) = mousebind_handler.next(window) {
            let visual_position =
                VisualPosition::new(mousebind.x - self.bounds.x, mousebind.y - self.bounds.y);

            match mousebind {
                Mousebind {
                    button: Some(MouseButton::Left),
                    mods: 0,
                    is_drag: false,
                    ..
                } => {
                    if let Some((i, _)) = self
                        .panes
                        .iter()
                        .enumerate()
                        .filter(|(_, pane)| pane.bounds().contains_position(visual_position))
                        .nth(0)
                    {
                        self.focused_pane_index = i;
                    }

                    mousebind_handler.unprocessed(window, mousebind);
                }
                _ => mousebind_handler.unprocessed(window, mousebind),
            }
        }

        let mut keybind_handler = window.get_keybind_handler();

        while let Some(keybind) = keybind_handler.next(window) {
            match keybind {
                Keybind {
                    key: Key::N,
                    mods: MOD_CTRL_ALT,
                } => {
                    self.add_pane(config, line_pool, time);
                }
                Keybind {
                    key: Key::W,
                    mods: MOD_CTRL_ALT,
                } => {
                    self.close_pane(config, line_pool, time);
                }
                Keybind {
                    key: Key::PageUp,
                    mods: MOD_CTRL | MOD_CTRL_ALT,
                } => {
                    let pane = &self.panes[self.focused_pane_index];

                    if (keybind.mods & MOD_ALT != 0 || pane.focused_tab_index() == 0)
                        && self.focused_pane_index > 0
                    {
                        self.focused_pane_index -= 1;
                    } else {
                        keybind_handler.unprocessed(window, keybind);
                    }
                }
                Keybind {
                    key: Key::PageDown,
                    mods: MOD_CTRL | MOD_CTRL_ALT,
                } => {
                    let pane = &self.panes[self.focused_pane_index];

                    if (keybind.mods & MOD_ALT != 0
                        || pane.focused_tab_index() == pane.tabs_len() - 1)
                        && self.focused_pane_index < self.panes.len() - 1
                    {
                        self.focused_pane_index += 1;
                    } else {
                        keybind_handler.unprocessed(window, keybind);
                    }
                }
                _ => keybind_handler.unprocessed(window, keybind),
            }
        }

        let pane = &mut self.panes[self.focused_pane_index];

        pane.update(
            &mut self.doc_list,
            command_palette,
            window,
            line_pool,
            text_buffer,
            config,
            time,
            dt,
        );

        if pane.tabs_len() == 0 {
            self.close_pane(config, line_pool, time);
        }

        window.clear_inputs();
    }

    pub fn draw(&mut self, config: &Config, gfx: &mut Gfx, is_focused: bool) {
        for (i, pane) in self.panes.iter_mut().enumerate() {
            let is_focused = is_focused && i == self.focused_pane_index;

            pane.draw(&mut self.doc_list, config, gfx, is_focused);
        }
    }

    fn clamp_focused_pane(&mut self) {
        if self.focused_pane_index >= self.panes.len() {
            if self.panes.is_empty() {
                self.focused_pane_index = 0;
            } else {
                self.focused_pane_index = self.panes.len() - 1;
            }
        }
    }

    fn add_pane(&mut self, config: &Config, line_pool: &mut LinePool, time: f32) {
        let pane = Pane::new(&mut self.doc_list, config, line_pool, time);

        if self.focused_pane_index >= self.panes.len() {
            self.panes.push(pane);
        } else {
            self.panes.insert(self.focused_pane_index + 1, pane);
            self.focused_pane_index += 1;
        }
    }

    fn close_pane(&mut self, config: &Config, line_pool: &mut LinePool, time: f32) {
        if self.panes.len() == 1 {
            return;
        }

        if !self.panes[self.focused_pane_index].close_all_tabs(
            &mut self.doc_list,
            config,
            line_pool,
            time,
        ) {
            return;
        }

        self.panes.remove(self.focused_pane_index);
        self.clamp_focused_pane();
    }

    pub fn on_close(&mut self, config: &Config, line_pool: &mut LinePool, time: f32) {
        self.doc_list
            .confirm_close_all("exiting", config, line_pool, time);
    }

    pub fn get_focused_pane_and_doc_list(&mut self) -> (&mut Pane, &mut DocList) {
        (&mut self.panes[self.focused_pane_index], &mut self.doc_list)
    }
}
