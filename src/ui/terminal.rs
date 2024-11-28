use crate::{
    config::Config, geometry::rect::Rect, platform::gfx::Gfx, temp_buffer::TempBuffer,
    text::line_pool::LinePool,
};

use super::{
    doc_list::DocList,
    terminal_emulator::{TerminalEmulator, COLOR_BLACK},
    terminal_pane::TerminalPane,
    widget::Widget,
    Ui, UiHandle,
};

pub struct Terminal {
    pane: TerminalPane,
    doc_list: DocList,
    emulators: Vec<TerminalEmulator>,

    pub widget: Widget,
}

impl Terminal {
    pub fn new(ui: &mut Ui, line_pool: &mut LinePool) -> Self {
        let mut doc_list = DocList::new();
        let mut emulators = Vec::new();

        let pane = TerminalPane::new(&mut doc_list, &mut emulators, line_pool);

        Self {
            pane,
            doc_list,
            emulators,

            widget: Widget::new(ui, true),
        }
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        let bounds = Rect::new(0.0, 0.0, bounds.width, gfx.line_height() * 15.0)
            .at_bottom_of(bounds)
            .floor();

        self.pane.layout(bounds, gfx, &mut self.doc_list);

        self.widget.layout(&[bounds]);
    }

    pub fn update(
        &mut self,
        ui: &mut UiHandle,
        line_pool: &mut LinePool,
        text_buffer: &mut TempBuffer<char>,
        config: &Config,
        time: f32,
        dt: f32,
    ) {
        self.pane.update(
            &self.widget,
            ui,
            &mut self.doc_list,
            &mut self.emulators,
            line_pool,
        );

        let focused_tab_index = self.pane.focused_tab_index();

        if let Some((tab, doc)) = self
            .pane
            .get_tab_with_doc_mut(focused_tab_index, &mut self.doc_list)
        {
            let emulator = &mut self.emulators[tab.doc_index()];

            emulator.update_input(
                &self.widget,
                ui,
                doc,
                tab,
                line_pool,
                text_buffer,
                config,
                time,
            );
        }

        for tab in &mut self.pane.tabs {
            let doc_index = tab.doc_index();

            let Some(doc) = self.doc_list.get_mut(doc_index) else {
                continue;
            };

            let emulator = &mut self.emulators[doc_index];

            emulator.update_output(ui, doc, tab, line_pool, time, dt);
        }
    }

    pub fn draw(&mut self, ui: &mut UiHandle, config: &Config) {
        let is_focused = self.widget.is_focused(ui);
        let gfx = ui.gfx();

        self.pane.draw(
            Some(COLOR_BLACK),
            &mut self.doc_list,
            config,
            gfx,
            is_focused,
        );
    }

    pub fn on_close(&mut self) {
        for emulator in &mut self.emulators {
            emulator.on_close();
        }
    }

    pub fn bounds(&self) -> Rect {
        self.widget.bounds()
    }

    pub fn is_animating(&self) -> bool {
        self.pane.is_animating()
    }

    pub fn emulators(&self) -> &[TerminalEmulator] {
        &self.emulators
    }
}
