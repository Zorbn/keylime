use std::{
    iter,
    mem::swap,
    ops::{Range, RangeInclusive},
};

use crate::{
    ctx::Ctx,
    geometry::{position::Position, rect::Rect},
    input::{
        action::{action_keybind, action_name, ActionName},
        editing_actions::handle_copy,
        key::Key,
        keybind::{MOD_ALT, MOD_CTRL, MOD_SHIFT},
    },
    platform::{gfx::Gfx, pty::Pty},
    text::{
        cursor::Cursor,
        doc::Doc,
        grapheme::{CharCursor, CharIterator},
        syntax_highlighter::TerminalHighlightKind,
    },
    ui::{tab::Tab, widget::Widget, Ui},
};

use super::TerminalDocs;

const MAX_SCROLLBACK_LINES: usize = 100;
const MIN_GRID_WIDTH: usize = 1;
const MIN_GRID_HEIGHT: usize = 1;

#[cfg(target_os = "windows")]
const SHELLS: &[&str] = &[
    "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
    "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
    "C:\\Windows\\system32\\cmd.exe",
];

#[cfg(target_os = "macos")]
const SHELLS: &[&str] = &["zsh", "bash", "sh"];

struct ColoredGridLine {
    is_dirty: bool,
    colors: Vec<(TerminalHighlightKind, TerminalHighlightKind)>,
}

impl ColoredGridLine {
    fn new() -> Self {
        Self {
            is_dirty: false,
            colors: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.colors.len()
    }

    fn splice(
        &mut self,
        range: Range<usize>,
        color_pair: impl Iterator<Item = (TerminalHighlightKind, TerminalHighlightKind)>,
    ) {
        self.colors.splice(range, color_pair);
        self.is_dirty = true;
    }

    fn clear(&mut self) {
        self.colors.clear();
        self.is_dirty = true;
    }

    fn expand(&mut self, grid_width: usize) {
        while self.colors.len() < grid_width {
            self.colors.push((
                TerminalHighlightKind::Foreground,
                TerminalHighlightKind::Background,
            ));

            self.is_dirty = true;
        }
    }
}

pub struct TerminalEmulator {
    pty: Option<Pty>,

    // The position of the terminal's cursor, which follows different rules
    // compared to the document's cursor for compatibility reasons, and may be
    // different from the document's cursor position is the user is selecting text.
    pub grid_cursor: Position,
    pub grid_width: usize,
    pub grid_height: usize,
    colored_grid_lines: Vec<ColoredGridLine>,
    empty_line_text: String,

    maintain_cursor_positions: bool,

    // Data for either the normal buffer or the alternate buffer,
    // depending on which one isn't currently being used.
    saved_grid_cursor: Position,
    saved_colored_grid_lines: Vec<ColoredGridLine>,
    saved_maintain_cursor_positions: bool,

    pub is_cursor_visible: bool,
    pub foreground_color: TerminalHighlightKind,
    pub background_color: TerminalHighlightKind,
    pub are_colors_swapped: bool,
    pub are_colors_bright: bool,
    pub scroll_top: usize,
    pub scroll_bottom: usize,
    is_in_alternate_buffer: bool,
    excess_lines_trimmed: usize,
}

impl TerminalEmulator {
    pub fn new() -> Self {
        Self {
            pty: None,

            grid_cursor: Position::zero(),
            grid_width: 0,
            grid_height: 0,
            colored_grid_lines: Vec::new(),
            empty_line_text: String::new(),

            maintain_cursor_positions: false,

            saved_grid_cursor: Position::zero(),
            saved_colored_grid_lines: Vec::new(),
            saved_maintain_cursor_positions: false,

            is_cursor_visible: true,
            foreground_color: TerminalHighlightKind::Foreground,
            background_color: TerminalHighlightKind::Background,
            are_colors_swapped: false,
            are_colors_bright: false,
            scroll_top: 0,
            scroll_bottom: 0,
            is_in_alternate_buffer: false,
            excess_lines_trimmed: 0,
        }
    }

    pub fn update_input(
        &mut self,
        widget: &mut Widget,
        ui: &mut Ui,
        docs: &mut TerminalDocs,
        tab: &mut Tab,
        ctx: &mut Ctx,
    ) {
        let Some(mut pty) = self.pty.take() else {
            return;
        };

        let doc = self.get_doc_mut(docs);

        let mut action_handler = widget.get_action_handler(ui, ctx.window);

        while let Some(action) = action_handler.next(ctx.window) {
            match action {
                action_keybind!(key: Enter) => {
                    pty.input().push(b'\r');
                }
                action_keybind!(key: Escape) => {
                    pty.input().push(0x1B);
                }
                action_keybind!(key: Tab) => {
                    pty.input().push(b'\t');
                }
                action_keybind!(key: Backspace, mods) => {
                    let key_byte = if mods & MOD_CTRL != 0 { 0x8 } else { 0x7F };

                    pty.input().extend_from_slice(&[key_byte]);
                }
                action_keybind!(keys: key @ (Key::Up | Key::Down | Key::Left | Key::Right | Key::Home | Key::End), mods) =>
                {
                    let key_byte = match key {
                        Key::Up => b'A',
                        Key::Down => b'B',
                        Key::Left => b'D',
                        Key::Right => b'C',
                        Key::Home => b'H',
                        Key::End => b'F',
                        _ => unreachable!(),
                    };

                    pty.input().extend_from_slice(&[0x1B, b'[']);

                    if mods != 0 {
                        pty.input().extend_from_slice(b"1;");
                    }

                    if mods & MOD_SHIFT != 0 && mods & MOD_CTRL != 0 {
                        pty.input().push(b'6');
                    } else if mods & MOD_SHIFT != 0 && mods & MOD_ALT != 0 {
                        pty.input().push(b'4');
                    } else if mods & MOD_SHIFT != 0 {
                        pty.input().push(b'2');
                    } else if mods & MOD_CTRL != 0 {
                        pty.input().push(b'5');
                    } else if mods & MOD_ALT != 0 {
                        pty.input().push(b'3');
                    }

                    pty.input().push(key_byte);
                }
                action_name!(names: Some(ActionName::Copy | ActionName::Cut))
                    if doc.has_selection() =>
                {
                    handle_copy(doc, ctx);
                }
                action_name!(Paste) => {
                    let text = ctx.buffers.text.get_mut();
                    let _ = ctx.window.get_clipboard(text);

                    pty.input().extend(text.bytes());
                }
                action_keybind!(key, mods: MOD_CTRL) => {
                    const KEY_A: u8 = Key::A as u8;
                    const KEY_Z: u8 = Key::Z as u8;

                    let key = key as u8;

                    if matches!(key, KEY_A..=KEY_Z) {
                        pty.input().push(key & 0x1F);
                    }
                }
                _ => {}
            }
        }

        let mut grapheme_handler = widget.get_grapheme_handler(ui, ctx.window);

        while let Some(grapheme) = grapheme_handler.next(ctx.window) {
            pty.input().extend(grapheme.bytes());
        }

        pty.flush();

        self.pty = Some(pty);

        tab.update(widget, ui, doc, ctx);
    }

    pub fn update_output(&mut self, docs: &mut TerminalDocs, tab: &mut Tab, ctx: &mut Ctx) {
        self.resize_grid(ctx.gfx, tab);

        let Some(mut pty) = self.pty.take() else {
            return;
        };

        self.maintain_cursor_positions = true;

        let doc = self.get_doc_mut(docs);
        let mut cursor_buffer = ctx.buffers.cursors.take_mut();

        let backup_doc_len = doc.lines().len();
        self.backup_doc_cursor_positions(doc, &mut cursor_buffer);
        self.expand_to_grid_size(docs, ctx);

        let (input, output) = pty.input_output();

        if let Ok(mut output) = output.try_lock() {
            self.handle_escape_sequences(docs, input, &output, ctx);

            output.clear();
        }

        let doc = self.get_doc_mut(docs);

        if self.maintain_cursor_positions {
            self.restore_doc_cursor_positions(doc, &mut cursor_buffer, backup_doc_len);
        } else {
            tab.recenter_cursor();
        }

        ctx.buffers.cursors.replace(cursor_buffer);

        self.pty = Some(pty);

        tab.camera.horizontal.is_locked = true;

        if self.is_in_alternate_buffer {
            // The alternate buffer is always the size of the camera and doesn't need to scroll.
            tab.camera.vertical.is_locked = true;
        } else {
            tab.camera.vertical.is_locked = false;

            tab.camera.vertical.position -=
                self.excess_lines_trimmed as f32 * ctx.gfx.line_height();
            self.excess_lines_trimmed = 0;
        }
    }

    fn expand_to_grid_size(&mut self, docs: &mut TerminalDocs, ctx: &mut Ctx) {
        self.empty_line_text.truncate(self.grid_width);

        while self.empty_line_text.len() < self.grid_width {
            self.empty_line_text.push(' ');
        }

        self.expand_doc_to_grid_size(&mut docs.normal, ctx);
        self.expand_doc_to_grid_size(&mut docs.alternate, ctx);

        Self::expand_colored_grid_lines_to_grid_size(
            self.grid_width,
            self.grid_height,
            &mut self.colored_grid_lines,
        );
        Self::expand_colored_grid_lines_to_grid_size(
            self.grid_width,
            self.grid_height,
            &mut self.saved_colored_grid_lines,
        );
    }

    fn expand_doc_to_grid_size(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        while doc.lines().len() < self.grid_height {
            let start = doc.end();

            doc.insert(start, "\n", ctx);
        }

        for y in 0..self.grid_height {
            let y = self.grid_y_to_doc_y(y, doc);
            let line_len = doc.get_line_len(y);

            if line_len < self.grid_width {
                doc.insert(
                    doc.get_line_end(y),
                    &self.empty_line_text[line_len..self.grid_width],
                    ctx,
                );
            }
        }
    }

    fn expand_colored_grid_lines_to_grid_size(
        grid_width: usize,
        grid_height: usize,
        colored_grid_lines: &mut Vec<ColoredGridLine>,
    ) {
        while colored_grid_lines.len() < grid_height {
            colored_grid_lines.push(ColoredGridLine::new());
        }

        for colored_grid_line in colored_grid_lines {
            colored_grid_line.expand(grid_width);
        }
    }

    fn resize_grid(&mut self, gfx: &mut Gfx, tab: &Tab) {
        let (grid_width, grid_height) = Self::get_grid_size(gfx, tab);

        if grid_width != self.grid_width || grid_height != self.grid_height {
            if let Some(pty) = self.pty.as_mut() {
                pty.resize(grid_width, grid_height);
            } else {
                self.pty = Pty::new(grid_width, grid_height, SHELLS).ok();
            }

            self.grid_width = grid_width;
            self.grid_height = grid_height;

            self.scroll_top = 0;
            self.scroll_bottom = grid_height - 1;
        }
    }

    fn get_grid_size(gfx: &mut Gfx, tab: &Tab) -> (usize, usize) {
        let Rect {
            width: doc_width,
            height: doc_height,
            ..
        } = tab.doc_bounds();

        let grid_width = (doc_width / gfx.glyph_width()).floor() as usize;
        let grid_width = grid_width.max(MIN_GRID_WIDTH);

        let grid_height = (doc_height / gfx.line_height()).floor() as usize;
        let grid_height = grid_height.max(MIN_GRID_HEIGHT);

        (grid_width, grid_height)
    }

    pub fn trim_excess_lines(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        let max_lines = self.grid_height + MAX_SCROLLBACK_LINES;

        if doc.lines().len() <= max_lines {
            return;
        }

        let excess_lines = doc.lines().len() - max_lines;
        self.excess_lines_trimmed += excess_lines;

        let start = Position::zero();
        let end = Position::new(0, excess_lines);

        doc.delete(start, end, ctx);
        doc.scroll_highlighted_lines(0..=doc.lines().len() - 1, excess_lines as isize);
    }

    fn scroll_highlighted_lines(
        &mut self,
        region: RangeInclusive<usize>,
        delta_y: isize,
        doc: &mut Doc,
    ) {
        let start = self.grid_y_to_doc_y(*region.start(), doc);
        let end = self.grid_y_to_doc_y(*region.end(), doc);

        doc.scroll_highlighted_lines(start..=end, delta_y);
    }

    // Scrolls the text in the region down, giving the impression that the camera is panning up.
    pub fn scroll_grid_region_down(
        &mut self,
        region: RangeInclusive<usize>,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) {
        self.highlight_lines(doc);

        self.grid_cursor.x = self.grid_position_byte_to_char(self.grid_cursor, doc);

        let scroll_top = *region.start();
        let scroll_bottom = *region.end();

        self.scroll_highlighted_lines(region, -1, doc);

        let delete_start =
            self.grid_position_to_doc_position(self.get_line_end(scroll_bottom - 1, doc), doc);

        let delete_end =
            self.grid_position_to_doc_position(self.get_line_end(scroll_bottom, doc), doc);

        let insert_start = self.grid_position_to_doc_position(Position::new(0, scroll_top), doc);

        doc.delete(delete_start, delete_end, ctx);
        doc.insert(insert_start, "\n", ctx);
        doc.insert(insert_start, &self.empty_line_text, ctx);

        let mut bottom_grid_line = self.colored_grid_lines.remove(scroll_bottom);
        bottom_grid_line.clear();
        bottom_grid_line.expand(self.grid_width);

        self.colored_grid_lines.insert(scroll_top, bottom_grid_line);

        self.grid_cursor = self.grid_position_char_to_byte(self.grid_cursor, doc);

        self.highlight_lines(doc);
    }

    // Scrolls the text in the region up, giving the impression that the camera is panning down.
    pub fn scroll_grid_region_up(
        &mut self,
        region: RangeInclusive<usize>,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) {
        self.highlight_lines(doc);

        self.grid_cursor.x = self.grid_position_byte_to_char(self.grid_cursor, doc);

        let scroll_top = *region.start();
        let scroll_bottom = *region.end();

        let should_use_scrollback = scroll_top == 0 && !self.is_in_alternate_buffer;

        let insert_start = if should_use_scrollback {
            self.grid_position_to_doc_position(self.get_line_end(scroll_bottom, doc), doc)
        } else {
            self.scroll_highlighted_lines(region, 1, doc);

            // We need to delete the line that got scrolled out:
            let delete_start =
                self.grid_position_to_doc_position(Position::new(0, scroll_top), doc);
            let delete_end = Position::new(0, delete_start.y + 1);

            let insert_start =
                self.grid_position_to_doc_position(self.get_line_end(scroll_bottom - 1, doc), doc);

            doc.delete(delete_start, delete_end, ctx);

            insert_start
        };

        doc.insert(insert_start, &self.empty_line_text, ctx);
        doc.insert(insert_start, "\n", ctx);

        let mut top_grid_line = self.colored_grid_lines.remove(scroll_top);
        top_grid_line.clear();
        top_grid_line.expand(self.grid_width);

        self.colored_grid_lines.insert(scroll_bottom, top_grid_line);

        self.grid_cursor = self.grid_position_char_to_byte(self.grid_cursor, doc);

        self.trim_excess_lines(doc, ctx);
        self.highlight_lines(doc);
    }

    pub fn switch_to_alternate_buffer(&mut self, doc: &mut Doc) {
        if self.is_in_alternate_buffer {
            return;
        }

        self.switch_buffer(doc);
    }

    pub fn switch_to_normal_buffer(&mut self, doc: &mut Doc) {
        if !self.is_in_alternate_buffer {
            return;
        }

        self.switch_buffer(doc);
    }

    pub fn switch_buffer(&mut self, doc: &mut Doc) {
        self.highlight_lines(doc);

        swap(&mut self.grid_cursor, &mut self.saved_grid_cursor);
        swap(
            &mut self.colored_grid_lines,
            &mut self.saved_colored_grid_lines,
        );
        swap(
            &mut self.maintain_cursor_positions,
            &mut self.saved_maintain_cursor_positions,
        );

        self.is_in_alternate_buffer = !self.is_in_alternate_buffer;
    }

    fn clamp_position(&self, position: Position, doc: &Doc) -> Position {
        let y = position.y.clamp(0, self.grid_height - 1);
        let x = position.x.clamp(0, self.get_line_len(y, doc));

        Position::new(x, y)
    }

    fn get_line_len(&self, y: usize, doc: &Doc) -> usize {
        let y = self.grid_y_to_doc_y(y, doc);

        doc.get_line_len(y)
    }

    pub fn get_line_end(&self, y: usize, doc: &Doc) -> Position {
        let doc_y = self.grid_y_to_doc_y(y, doc);
        let doc_position = doc.get_line_end(doc_y);

        self.doc_position_to_grid_position(doc_position, doc)
    }

    fn move_position_right(&self, position: Position, distance: usize, doc: &mut Doc) -> Position {
        let mut doc_position = self.grid_position_to_doc_position(position, doc);

        let Some(line) = doc.get_line(doc_position.y) else {
            return position;
        };

        let mut char_cursor = CharCursor::new(doc_position.x, line.len());

        for _ in 0..distance {
            match char_cursor.next_boundary(line) {
                Some(new_x) => {
                    doc_position.x = new_x;
                }
                _ => break,
            }
        }

        self.doc_position_to_grid_position(doc_position, doc)
    }

    fn move_position_left(&self, position: Position, distance: usize, doc: &Doc) -> Position {
        let mut doc_position = self.grid_position_to_doc_position(position, doc);

        let Some(line) = doc.get_line(doc_position.y) else {
            return position;
        };

        let mut char_cursor = CharCursor::new(doc_position.x, line.len());

        for _ in 0..distance {
            match char_cursor.previous_boundary(line) {
                Some(new_x) => {
                    doc_position.x = new_x;
                }
                _ => break,
            }
        }

        self.doc_position_to_grid_position(doc_position, doc)
    }

    pub fn move_position(
        &mut self,
        mut position: Position,
        delta_x: isize,
        delta_y: isize,
        doc: &mut Doc,
    ) -> Position {
        if delta_y != 0 {
            position.x = self.grid_position_byte_to_char(position, doc);
            position.y = position.y.saturating_add_signed(delta_y);
            position = self.grid_position_char_to_byte(position, doc);
        }

        if delta_x < 0 {
            self.move_position_left(position, delta_x.unsigned_abs(), doc)
        } else {
            self.move_position_right(position, delta_x as usize, doc)
        }
    }

    pub fn grid_position_char_to_byte(&self, position: Position, doc: &mut Doc) -> Position {
        self.move_position_right(Position::new(0, position.y), position.x, doc)
    }

    pub fn grid_position_byte_to_char(&self, position: Position, doc: &Doc) -> usize {
        let position = self.grid_position_to_doc_position(position, doc);

        CharIterator::new(&doc.lines()[position.y][..position.x]).count()
    }

    pub fn grid_position_to_doc_position(&self, position: Position, doc: &Doc) -> Position {
        Position::new(position.x, self.grid_y_to_doc_y(position.y, doc))
    }

    fn doc_position_to_grid_position(&self, position: Position, doc: &Doc) -> Position {
        Position::new(position.x, self.doc_y_to_grid_y(position.y, doc))
    }

    pub fn grid_y_to_doc_y(&self, y: usize, doc: &Doc) -> usize {
        doc.lines().len().saturating_sub(self.grid_height) + y
    }

    fn doc_y_to_grid_y(&self, y: usize, doc: &Doc) -> usize {
        y.saturating_sub(doc.lines().len().saturating_sub(self.grid_height))
    }

    fn backup_doc_cursor_positions(&mut self, doc: &Doc, cursor_buffer: &mut Vec<Cursor>) {
        doc.backup_cursors(cursor_buffer);
    }

    fn restore_doc_cursor_positions(
        &mut self,
        doc: &mut Doc,
        cursor_buffer: &mut [Cursor],
        backup_doc_len: usize,
    ) {
        let offset_y = doc.lines().len() as isize - backup_doc_len as isize;

        for cursor in cursor_buffer.iter_mut() {
            cursor.position.y = cursor.position.y.saturating_add_signed(offset_y);

            if let Some(selection_anchor) = &mut cursor.selection_anchor {
                selection_anchor.y = selection_anchor.y.saturating_add_signed(offset_y);
            }
        }

        doc.restore_cursors(cursor_buffer);
    }

    pub fn jump_doc_cursors_to_grid_cursor(&mut self, doc: &mut Doc, gfx: &mut Gfx) {
        if !self.is_cursor_visible {
            return;
        }

        self.maintain_cursor_positions = false;

        let doc_position =
            self.grid_position_to_doc_position(self.clamp_position(self.grid_cursor, doc), doc);
        doc.jump_cursors(doc_position, false, gfx);
    }

    pub fn move_cursor(&mut self, delta_x: isize, delta_y: isize, doc: &mut Doc, gfx: &mut Gfx) {
        self.grid_cursor = self.move_position(self.grid_cursor, delta_x, delta_y, doc);
        self.jump_doc_cursors_to_grid_cursor(doc, gfx);
    }

    pub fn jump_cursor(&mut self, position: Position, doc: &mut Doc, gfx: &mut Gfx) {
        self.grid_cursor = self.clamp_position(position, doc);

        self.jump_doc_cursors_to_grid_cursor(doc, gfx);
    }

    pub fn newline_cursor(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        if self.grid_cursor.y == self.scroll_bottom {
            self.scroll_grid_region_up(self.scroll_top..=self.scroll_bottom, doc, ctx);
        } else {
            self.move_cursor(0, 1, doc, ctx.gfx);
        }
    }

    pub fn insert_at_cursor(&mut self, text: &str, doc: &mut Doc, ctx: &mut Ctx) {
        for c in CharIterator::new(text) {
            if self.grid_position_byte_to_char(self.grid_cursor, doc) >= self.grid_width {
                self.jump_cursor(Position::new(0, self.grid_cursor.y), doc, ctx.gfx);
                self.newline_cursor(doc, ctx);
            }

            self.grid_cursor = self.raw_insert_char(self.grid_cursor, c, doc, ctx);
        }

        self.jump_doc_cursors_to_grid_cursor(doc, ctx.gfx);
    }

    fn color_to_bright(color: TerminalHighlightKind) -> TerminalHighlightKind {
        match color {
            TerminalHighlightKind::Foreground => TerminalHighlightKind::BrightForeground,
            TerminalHighlightKind::Background => TerminalHighlightKind::BrightBackground,
            TerminalHighlightKind::Red => TerminalHighlightKind::BrightRed,
            TerminalHighlightKind::Green => TerminalHighlightKind::BrightGreen,
            TerminalHighlightKind::Yellow => TerminalHighlightKind::BrightYellow,
            TerminalHighlightKind::Blue => TerminalHighlightKind::BrightBlue,
            TerminalHighlightKind::Magenta => TerminalHighlightKind::BrightMagenta,
            TerminalHighlightKind::Cyan => TerminalHighlightKind::BrightCyan,
            _ => color,
        }
    }

    pub fn highlight_lines(&mut self, doc: &mut Doc) {
        for y in 0..self.colored_grid_lines.len() {
            let doc_y = self.grid_y_to_doc_y(y, doc);
            let colored_line = &mut self.colored_grid_lines[y];

            if !colored_line.is_dirty {
                continue;
            }

            doc.highlight_line_from_terminal_colors(&colored_line.colors, doc_y);

            colored_line.is_dirty = false;
        }
    }

    pub fn delete(&mut self, start: Position, end: Position, doc: &mut Doc, ctx: &mut Ctx) {
        for y in start.y..=end.y {
            let start_x = if y == start.y { start.x } else { 0 };

            let end_x = if y == end.y {
                end.x
            } else {
                self.get_line_len(y, doc)
            };

            for x in start_x..end_x {
                self.raw_insert_char(Position::new(x, y), " ", doc, ctx);
            }
        }

        self.jump_doc_cursors_to_grid_cursor(doc, ctx.gfx);
    }

    // Should be used indirectly by delete, insert_at_cursor, etc.
    // Doesn't update the doc cursors or handle multiple chars.
    fn raw_insert_char(
        &mut self,
        start: Position,
        mut c: &str,
        doc: &mut Doc,
        ctx: &mut Ctx,
    ) -> Position {
        let mut position = start;

        let colors = if self.are_colors_swapped {
            (self.background_color, self.foreground_color)
        } else {
            (self.foreground_color, self.background_color)
        };

        let colors = if self.are_colors_bright {
            (Self::color_to_bright(colors.0), colors.1)
        } else {
            colors
        };

        for _ in 0..ctx.gfx.measure_text(c) {
            let delete_end = self.move_position(position, 1, 0, doc);

            let insert_start = self.grid_position_to_doc_position(position, doc);
            let delete_end = self.grid_position_to_doc_position(delete_end, doc);

            doc.delete(insert_start, delete_end, ctx);
            doc.insert(insert_start, c, ctx);

            let colored_line = &mut self.colored_grid_lines[position.y];
            let colored_line_len = colored_line.len();

            colored_line.splice(
                insert_start.x..delete_end.x.min(colored_line_len),
                iter::repeat(colors).take(c.len()),
            );

            position = self.move_position(position, 1, 0, doc);
            c = "\u{200B}";
        }

        position
    }

    pub fn get_doc<'a>(&self, docs: &'a TerminalDocs) -> &'a Doc {
        if self.is_in_alternate_buffer {
            &docs.alternate
        } else {
            &docs.normal
        }
    }

    pub fn get_doc_mut<'a>(&self, docs: &'a mut TerminalDocs) -> &'a mut Doc {
        if self.is_in_alternate_buffer {
            &mut docs.alternate
        } else {
            &mut docs.normal
        }
    }

    pub fn pty(&mut self) -> Option<&mut Pty> {
        self.pty.as_mut()
    }
}
