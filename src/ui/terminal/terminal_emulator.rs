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
        mods::{Mod, Mods},
    },
    platform::{
        gfx::Gfx,
        process::{Process, ProcessKind},
    },
    pool::{format_pooled, STRING_POOL},
    text::{
        doc::Doc,
        grapheme::{CharCursor, CharIterator},
        syntax_highlighter::{HighlightKind, TerminalHighlightKind},
    },
    ui::{
        camera::CameraRecenterKind,
        core::WidgetId,
        msg::Msg,
        tab::Tab,
        terminal::{
            escape_sequences::{parse_escape_sequences, EscapeSequence},
            TERMINAL_DISPLAY_NAME,
        },
    },
};

use super::TerminalDocs;

const MAX_SCROLLBACK_LINES: usize = 100;
const MIN_GRID_WIDTH: usize = 1;
const MIN_GRID_HEIGHT: usize = 1;

#[cfg(target_os = "windows")]
const SHELLS: &[&str] = &["pwsh.exe", "powershell.exe", "cmd.exe"];

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
    pty: Option<Process>,
    pending_escape_sequences: Option<Vec<EscapeSequence<'static>>>,

    // The position of the terminal's cursor, which follows different rules
    // compared to the document's cursor for compatibility reasons, and may be
    // different from the document's cursor position is the user is selecting text.
    grid_cursor: Position,
    grid_width: usize,
    grid_height: usize,
    colored_grid_lines: Vec<ColoredGridLine>,
    empty_line_text: String,
    did_doc_cursors_move: bool,

    // Data for either the normal buffer or the alternate buffer,
    // depending on which one isn't currently being used.
    saved_grid_cursor: Position,
    saved_colored_grid_lines: Vec<ColoredGridLine>,

    is_cursor_visible: bool,
    foreground_color: TerminalHighlightKind,
    background_color: TerminalHighlightKind,
    are_colors_swapped: bool,
    are_colors_bright: bool,
    scroll_top: usize,
    scroll_bottom: usize,
    is_in_alternate_buffer: bool,
    excess_lines_trimmed: usize,
}

impl TerminalEmulator {
    pub fn new() -> Self {
        Self {
            pty: None,
            pending_escape_sequences: Some(Vec::new()),

            grid_cursor: Position::ZERO,
            grid_width: MIN_GRID_WIDTH,
            grid_height: MIN_GRID_HEIGHT,
            colored_grid_lines: Vec::new(),
            empty_line_text: String::new(),
            did_doc_cursors_move: false,

            saved_grid_cursor: Position::ZERO,
            saved_colored_grid_lines: Vec::new(),

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

    // TODO: This is a hack.
    pub fn receive_msgs(&mut self, tab: &mut Tab, docs: &mut TerminalDocs, ctx: &mut Ctx) {
        let widget_id = tab.widget_id();

        let Some(mut pty) = self.pty.take() else {
            while let Some(msg) = ctx.ui.msg(widget_id) {
                ctx.ui.skip(widget_id, msg);
            }

            return;
        };

        let doc = self.doc_mut(docs);

        while let Some(msg) = ctx.ui.msg(widget_id) {
            match msg {
                Msg::Grapheme(grapheme) => {
                    pty.input().extend(grapheme.bytes());
                }
                Msg::Action(action_keybind!(key: Enter)) => {
                    pty.input().push(b'\r');
                }
                Msg::Action(action_keybind!(key: Escape)) => {
                    pty.input().push(0x1B);
                }
                Msg::Action(action_keybind!(key: Tab)) => {
                    pty.input().push(b'\t');
                }
                Msg::Action(action_keybind!(key: Backspace, mods)) => {
                    let key_byte = if mods.contains(Mod::Ctrl) { 0x8 } else { 0x7F };

                    pty.input().extend_from_slice(&[key_byte]);
                }
                Msg::Action(
                    action_keybind!(keys: key @ (Key::Up | Key::Down | Key::Left | Key::Right | Key::Home | Key::End), mods),
                ) if !mods.contains(Mod::Cmd) => {
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

                    if mods != Mods::NONE {
                        pty.input().extend_from_slice(b"1;");
                    }

                    if mods.contains(Mod::Shift) && mods.contains(Mod::Ctrl) {
                        pty.input().push(b'6');
                    } else if mods.contains(Mod::Shift) && mods.contains(Mod::Alt) {
                        pty.input().push(b'4');
                    } else if mods.contains(Mod::Shift) {
                        pty.input().push(b'2');
                    } else if mods.contains(Mod::Ctrl) {
                        pty.input().push(b'5');
                    } else if mods.contains(Mod::Alt) {
                        pty.input().push(b'3');
                    }

                    pty.input().push(key_byte);
                }
                Msg::Action(action_name!(names: Some(ActionName::Copy | ActionName::Cut)))
                    if doc.has_selection() =>
                {
                    handle_copy(doc, ctx);
                }
                Msg::Action(action_name!(Paste)) => {
                    let mut text = STRING_POOL.new_item();
                    let _ = ctx.window.get_clipboard(&mut text);

                    pty.input().extend(text.bytes());
                }
                Msg::Action(action_keybind!(key, mods: Mods::CTRL)) => {
                    const KEY_A: u8 = Key::A as u8;
                    const KEY_Z: u8 = Key::Z as u8;

                    let key = key as u8;

                    if matches!(key, KEY_A..=KEY_Z) {
                        pty.input().push(key & 0x1F);
                    }
                }
                _ => tab.receive_msg(msg, doc, ctx),
            }
        }

        pty.flush();

        self.pty = Some(pty);
    }

    pub fn update_input(&mut self, docs: &mut TerminalDocs, tab: &mut Tab, ctx: &mut Ctx) {
        let doc = self.doc_mut(docs);

        tab.update(doc, ctx);
    }

    pub fn update_output(&mut self, docs: &mut TerminalDocs, tab: &mut Tab, ctx: &mut Ctx) {
        let last_grid_height = self.grid_height;
        self.resize_grid(tab, ctx);

        let Some(mut pty) = self.pty.take() else {
            return;
        };

        self.expand_to_grid_size(docs, last_grid_height, ctx);

        let (input, output) = pty.input_output();

        if let Ok(mut output) = output.lock() {
            self.handle_escape_sequences(docs, tab, input, &output, ctx);

            output.clear();
        }

        self.pty = Some(pty);

        tab.camera
            .vertical
            .jump_visual_distance(self.excess_lines_trimmed as f32 * -ctx.gfx.line_height());
        self.excess_lines_trimmed = 0;

        if self.did_doc_cursors_move {
            tab.camera
                .vertical
                .recenter(CameraRecenterKind::OnScrollBorder);

            self.did_doc_cursors_move = false;
        }

        tab.camera.horizontal.set_locked(true);
    }

    fn handle_escape_sequences(
        &mut self,
        docs: &mut TerminalDocs,
        tab: &mut Tab,
        input: &mut Vec<u8>,
        output: &[u8],
        ctx: &mut Ctx,
    ) {
        let Some(mut result) = self.pending_escape_sequences.take() else {
            return;
        };

        parse_escape_sequences(output, &mut result);

        for sequence in result.drain(..) {
            self.handle_escape_sequence(docs, tab, input, sequence, ctx);
        }

        let doc = self.doc_mut(docs);

        self.highlight_lines(doc);

        // In-place collect:
        self.pending_escape_sequences = Some(result.into_iter().map(|_| unreachable!()).collect());
    }

    fn handle_escape_sequence(
        &mut self,
        docs: &mut TerminalDocs,
        tab: &mut Tab,
        input: &mut Vec<u8>,
        sequence: EscapeSequence,
        ctx: &mut Ctx,
    ) {
        let doc = self.doc_mut(docs);

        match sequence {
            EscapeSequence::Plain(text) => self.insert_at_cursor(text, doc, ctx),
            EscapeSequence::Backspace => self.move_cursor(-1, 0, doc, ctx.gfx),
            EscapeSequence::Tab => {
                let next_tab_stop = (self.grid_cursor.x / 8 + 1) * 8;

                for _ in 0..(next_tab_stop - self.grid_cursor.x) {
                    self.insert_at_cursor(" ", doc, ctx);
                }
            }
            EscapeSequence::CarriageReturn => {
                self.jump_cursor(Position::new(0, self.grid_cursor.y), doc, ctx.gfx);
            }
            EscapeSequence::Newline => self.newline_cursor(doc, ctx),
            EscapeSequence::ReverseNewline => self.reverse_newline_cursor(doc, ctx),
            EscapeSequence::HideCursor => self.is_cursor_visible = false,
            EscapeSequence::ShowCursor => {
                self.is_cursor_visible = true;
                self.jump_doc_cursors_to_grid_cursor(doc, ctx.gfx);
            }
            EscapeSequence::SwitchToNormalBuffer => self.switch_to_normal_buffer(docs, tab, ctx),
            EscapeSequence::SwitchToAlternateBuffer => self.switch_to_alternate_buffer(doc, tab),
            EscapeSequence::QueryModifyKeyboard
            | EscapeSequence::QueryModifyCursorKeys
            | EscapeSequence::QueryModifyFunctionKeys
            | EscapeSequence::QueryModifyOtherKeys => {
                let default_value = match sequence {
                    EscapeSequence::QueryModifyKeyboard => 0,
                    EscapeSequence::QueryModifyFunctionKeys => 2,
                    EscapeSequence::QueryModifyOtherKeys => 2,
                    EscapeSequence::QueryDeviceAttributes => 0,
                    _ => unreachable!(),
                };

                let response = format_pooled!("\x1B[>{}m", default_value);

                input.extend(response.bytes());
            }
            EscapeSequence::QueryDeviceAttributes => {
                // According to invisible-island.net/xterm 41 corresponds to a VT420 which is the default.
                let response = "\x1B[>41;0;0c";
                input.extend(response.bytes());
            }
            EscapeSequence::ResetFormatting => {
                self.foreground_color = TerminalHighlightKind::Foreground;
                self.background_color = TerminalHighlightKind::Background;
                self.are_colors_swapped = false;
                self.are_colors_bright = false;
            }
            EscapeSequence::SetColorsBright(flag) => self.are_colors_bright = flag,
            EscapeSequence::SetColorsSwapped(flag) => self.are_colors_swapped = flag,
            EscapeSequence::SetForegroundColor(color) => self.foreground_color = color,
            EscapeSequence::SetBackgroundColor(color) => self.background_color = color,
            EscapeSequence::SetCursorX(x) => {
                let position =
                    self.grid_position_char_to_byte(Position::new(x, self.grid_cursor.y), doc);

                self.jump_cursor(position, doc, ctx.gfx);
            }
            EscapeSequence::SetCursorY(y) => {
                let char_x = self.grid_position_byte_to_char(self.grid_cursor, doc);
                let position = self.grid_position_char_to_byte(Position::new(char_x, y), doc);

                self.jump_cursor(position, doc, ctx.gfx);
            }
            EscapeSequence::SetCursorPosition(x, y) => {
                let position = self.grid_position_char_to_byte(Position::new(x, y), doc);

                self.jump_cursor(position, doc, ctx.gfx);
            }
            EscapeSequence::MoveCursorX(distance) => self.move_cursor(distance, 0, doc, ctx.gfx),
            EscapeSequence::MoveCursorY(distance) => self.move_cursor(0, distance, doc, ctx.gfx),
            EscapeSequence::MoveCursorYAndResetX(distance) => {
                let y = self.grid_cursor.y.saturating_add_signed(distance);

                self.jump_cursor(Position::new(0, y), doc, ctx.gfx);
            }
            EscapeSequence::ClearToScreenEnd => {
                let start = self.grid_cursor;
                let end = self.line_end(self.grid_height - 1, doc);

                self.delete(start, end, doc, ctx);
            }
            EscapeSequence::ClearToScreenStart => {
                let start = Position::ZERO;
                let end = self.grid_cursor;

                self.delete(start, end, doc, ctx);
            }
            EscapeSequence::ClearScreen => {
                let start = Position::ZERO;
                let end = self.line_end(self.grid_height - 1, doc);

                self.delete(start, end, doc, ctx);
            }
            EscapeSequence::ClearScrollbackLines => {
                self.clear_scrollback_lines(doc, ctx);
            }
            EscapeSequence::ClearToLineEnd => {
                let start = self.grid_cursor;
                let end = self.line_end(start.y, doc);

                self.delete(start, end, doc, ctx);
            }
            EscapeSequence::ClearToLineStart => {
                let start = Position::new(0, self.grid_cursor.y);
                let end = Position::new(self.grid_cursor.x, self.grid_cursor.y);

                self.delete(start, end, doc, ctx);
            }
            EscapeSequence::ClearLine => {
                let start = Position::new(0, self.grid_cursor.y);
                let end = self.line_end(start.y, doc);

                self.delete(start, end, doc, ctx);
            }
            EscapeSequence::InsertLines(count) => {
                let scroll_top = self.scroll_top.max(self.grid_cursor.y);
                let scroll_bottom = self.scroll_bottom;

                if scroll_top > scroll_bottom {
                    return;
                }

                for _ in 0..count {
                    self.scroll_grid_region_down(scroll_top..=scroll_bottom, doc, ctx);
                }
            }
            EscapeSequence::DeleteLines(count) => {
                let scroll_top = self.scroll_top.max(self.grid_cursor.y);
                let scroll_bottom = self.scroll_bottom;

                if scroll_top > scroll_bottom {
                    return;
                }

                for _ in 0..count {
                    self.scroll_grid_region_up(scroll_top..=scroll_bottom, doc, ctx);
                }
            }
            EscapeSequence::ScrollUp(distance) => {
                for _ in 0..distance {
                    self.scroll_grid_region_up(self.scroll_top..=self.scroll_bottom, doc, ctx);
                }
            }
            EscapeSequence::ScrollDown(distance) => {
                for _ in 0..distance {
                    self.scroll_grid_region_down(self.scroll_top..=self.scroll_bottom, doc, ctx);
                }
            }
            EscapeSequence::ClearCharsAfterCursor(distance) => {
                let start = self.grid_cursor;
                let end = self.move_position(start, distance as isize, 0, doc);

                self.delete(start, end, doc, ctx);
            }
            EscapeSequence::DeleteCharsAfterCursor(distance) => {
                let start = self.grid_cursor;
                let end = self.move_position(start, 1, 0, doc);

                let start = self.grid_position_to_doc_position(start, doc);
                let end = self.grid_position_to_doc_position(end, doc);

                if start.y != end.y {
                    return;
                }

                for _ in 0..distance {
                    doc.delete(start, end, ctx);
                    doc.insert(doc.line_end(start.y), " ", ctx);
                }
            }
            EscapeSequence::SetScrollRegion { top, bottom } => {
                self.scroll_bottom = bottom.clamp(0, self.grid_height - 1);
                self.scroll_top = top.clamp(0, self.scroll_bottom);
            }
            EscapeSequence::QueryDeviceStatus => {
                let char_x = self.grid_position_byte_to_char(self.grid_cursor, doc);

                let response = format_pooled!("\x1B[{};{}R", self.grid_cursor.y + 1, char_x + 1);

                input.extend(response.bytes());
            }
            EscapeSequence::QueryTerminalId => {
                // Claim to be a "VT101 with no options".
                input.extend("\x1B[?1;0c".bytes());
            }
            EscapeSequence::SetTitle(title) => doc.set_display_name(Some(title.into())),
            EscapeSequence::ResetTitle => doc.set_display_name(Some(TERMINAL_DISPLAY_NAME.into())),
            EscapeSequence::QueryForegroundColor | EscapeSequence::QueryBackgroundColor => {
                let (color, kind) = if matches!(sequence, EscapeSequence::QueryForegroundColor) {
                    (self.foreground_color, 10)
                } else {
                    (self.background_color, 11)
                };

                let theme = &ctx.config.theme;
                let color = theme.highlight_kind_to_color(HighlightKind::Terminal(color));

                let response = format_pooled!(
                    "\x1B]{};rgb:{:2X}{:2X}{:2X}\x07",
                    kind,
                    color.r,
                    color.g,
                    color.b
                );

                input.extend(response.bytes());
            }
        }
    }

    fn expand_to_grid_size(
        &mut self,
        docs: &mut TerminalDocs,
        last_grid_height: usize,
        ctx: &mut Ctx,
    ) {
        self.empty_line_text.truncate(self.grid_width);

        while self.empty_line_text.len() < self.grid_width {
            self.empty_line_text.push(' ');
        }

        Self::expand_colored_grid_lines_to_grid_size(
            self.grid_width,
            self.grid_height,
            last_grid_height,
            &mut self.colored_grid_lines,
        );
        Self::expand_colored_grid_lines_to_grid_size(
            self.grid_width,
            self.grid_height,
            last_grid_height,
            &mut self.saved_colored_grid_lines,
        );

        self.expand_doc_to_grid_size(&mut docs.normal, last_grid_height, ctx);
        self.expand_doc_to_grid_size(&mut docs.alternate, last_grid_height, ctx);
    }

    fn expand_doc_to_grid_size(&mut self, doc: &mut Doc, last_grid_height: usize, ctx: &mut Ctx) {
        if self.grid_height < last_grid_height {
            let start_y = doc.lines().len().saturating_sub(last_grid_height) + self.grid_cursor.y;
            let start_y = start_y.max(self.grid_height - 1);
            let start = doc.line_end(start_y);

            doc.delete(start, doc.end(), ctx);
        }

        if self.grid_height > last_grid_height {
            for _ in last_grid_height..self.grid_height {
                doc.insert(doc.end(), "\n", ctx);
                doc.insert(doc.end(), &self.empty_line_text, ctx);
            }

            self.highlight_lines(doc);
        }

        for y in 0..self.grid_height {
            let y = self.grid_y_to_doc_y(y, doc);
            let line_len = doc.line_len(y);

            if line_len < self.grid_width {
                doc.insert(
                    doc.line_end(y),
                    &self.empty_line_text[line_len..self.grid_width],
                    ctx,
                );
            }
        }
    }

    fn expand_colored_grid_lines_to_grid_size(
        grid_width: usize,
        grid_height: usize,
        last_grid_height: usize,
        colored_grid_lines: &mut Vec<ColoredGridLine>,
    ) {
        if last_grid_height > grid_height {
            colored_grid_lines.truncate(grid_height);
        }

        if grid_height >= last_grid_height {
            while colored_grid_lines.len() < grid_height {
                colored_grid_lines.push(ColoredGridLine::new());
            }
        }

        for colored_grid_line in colored_grid_lines {
            colored_grid_line.expand(grid_width);
        }
    }

    fn resize_grid(&mut self, tab: &Tab, ctx: &Ctx) {
        let (grid_width, grid_height) = Self::grid_size(ctx, tab);

        if grid_width == self.grid_width && grid_height == self.grid_height {
            return;
        }

        if let Some(pty) = self.pty.as_mut() {
            pty.resize(grid_width, grid_height);
        } else {
            self.pty = Process::new(
                SHELLS,
                ProcessKind::Pty {
                    width: grid_width,
                    height: grid_height,
                },
            )
            .ok();
        }

        self.grid_width = grid_width;
        self.grid_height = grid_height;

        self.scroll_top = 0;
        self.scroll_bottom = grid_height - 1;
    }

    fn grid_size(ctx: &Ctx, tab: &Tab) -> (usize, usize) {
        let Rect {
            width: doc_width,
            height: doc_height,
            ..
        } = tab.doc_bounds(ctx.ui);

        let grid_width = (doc_width / ctx.gfx.glyph_width()).floor() as usize;
        let grid_width = grid_width.max(MIN_GRID_WIDTH);

        let grid_height = (doc_height / ctx.gfx.line_height()).floor() as usize;
        let grid_height = grid_height.max(MIN_GRID_HEIGHT);

        (grid_width, grid_height)
    }

    fn trim_scrollback_lines(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        self.trim_excess_lines(MAX_SCROLLBACK_LINES, doc, ctx);
    }

    fn clear_scrollback_lines(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        self.trim_excess_lines(0, doc, ctx);
    }

    fn trim_excess_lines(&mut self, max_scrollback_lines: usize, doc: &mut Doc, ctx: &mut Ctx) {
        let max_lines = self.grid_height + max_scrollback_lines;

        if doc.lines().len() <= max_lines {
            return;
        }

        let excess_lines = doc.lines().len() - max_lines;
        self.excess_lines_trimmed += excess_lines;

        let start = Position::ZERO;
        let end = Position::new(0, excess_lines);

        doc.delete(start, end, ctx);
        doc.scroll_highlighted_lines(0..=doc.lines().len() - 1, excess_lines as isize);
    }

    fn scroll_highlighted_lines(
        &self,
        region: RangeInclusive<usize>,
        delta_y: isize,
        doc: &mut Doc,
    ) {
        let start = self.grid_y_to_doc_y(*region.start(), doc);
        let end = self.grid_y_to_doc_y(*region.end(), doc);

        doc.scroll_highlighted_lines(start..=end, delta_y);
    }

    // Scrolls the text in the region down, giving the impression that the camera is panning up.
    fn scroll_grid_region_down(
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
            self.grid_position_to_doc_position(self.line_end(scroll_bottom - 1, doc), doc);

        let delete_end = self.grid_position_to_doc_position(self.line_end(scroll_bottom, doc), doc);

        let insert_start = self.grid_position_to_doc_position(Position::new(0, scroll_top), doc);

        doc.delete(delete_start, delete_end, ctx);
        doc.insert(insert_start, "\n", ctx);
        doc.insert(insert_start, &self.empty_line_text, ctx);

        let mut bottom_grid_line = self.colored_grid_lines.remove(scroll_bottom);
        bottom_grid_line.clear();
        bottom_grid_line.expand(self.grid_width);

        self.colored_grid_lines.insert(scroll_top, bottom_grid_line);

        self.grid_cursor = self.grid_position_char_to_byte(self.grid_cursor, doc);
        self.jump_doc_cursors_to_grid_cursor(doc, ctx.gfx);

        self.highlight_lines(doc);
    }

    // Scrolls the text in the region up, giving the impression that the camera is panning down.
    fn scroll_grid_region_up(
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
            self.grid_position_to_doc_position(self.line_end(scroll_bottom, doc), doc)
        } else {
            self.scroll_highlighted_lines(region, 1, doc);

            // We need to delete the line that got scrolled out:
            let delete_start =
                self.grid_position_to_doc_position(Position::new(0, scroll_top), doc);
            let delete_end = Position::new(0, delete_start.y + 1);

            let insert_start =
                self.grid_position_to_doc_position(self.line_end(scroll_bottom, doc), doc);

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
        self.jump_doc_cursors_to_grid_cursor(doc, ctx.gfx);

        self.trim_scrollback_lines(doc, ctx);
        self.highlight_lines(doc);
    }

    fn switch_to_alternate_buffer(&mut self, doc: &mut Doc, tab: &mut Tab) {
        if self.is_in_alternate_buffer {
            return;
        }

        self.switch_buffer(doc, tab);
    }

    fn switch_to_normal_buffer(&mut self, docs: &mut TerminalDocs, tab: &mut Tab, ctx: &mut Ctx) {
        if !self.is_in_alternate_buffer {
            return;
        }

        self.switch_buffer(&mut docs.alternate, tab);
        tab.skip_camera_animations(&docs.normal, ctx);
    }

    fn switch_buffer(&mut self, doc: &mut Doc, tab: &mut Tab) {
        self.highlight_lines(doc);

        swap(&mut self.grid_cursor, &mut self.saved_grid_cursor);
        swap(
            &mut self.colored_grid_lines,
            &mut self.saved_colored_grid_lines,
        );

        self.is_in_alternate_buffer = !self.is_in_alternate_buffer;

        // The alternate buffer is always the size of the camera and doesn't need to scroll.
        tab.camera.vertical.set_locked(self.is_in_alternate_buffer);
    }

    fn clamp_position(&self, position: Position, doc: &Doc) -> Position {
        let y = position.y.clamp(0, self.grid_height - 1);
        let x = position.x.clamp(0, self.line_len(y, doc));

        Position::new(x, y)
    }

    fn line_len(&self, y: usize, doc: &Doc) -> usize {
        let y = self.grid_y_to_doc_y(y, doc);

        doc.line_len(y)
    }

    fn line_end(&self, y: usize, doc: &Doc) -> Position {
        let doc_y = self.grid_y_to_doc_y(y, doc);
        let doc_position = doc.line_end(doc_y);

        self.doc_position_to_grid_position(doc_position, doc)
    }

    fn move_position_right(&self, position: Position, distance: usize, doc: &Doc) -> Position {
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

    fn move_position(
        &self,
        mut position: Position,
        delta_x: isize,
        delta_y: isize,
        doc: &Doc,
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

    fn grid_position_char_to_byte(&self, position: Position, doc: &Doc) -> Position {
        self.move_position_right(Position::new(0, position.y), position.x, doc)
    }

    fn grid_position_byte_to_char(&self, position: Position, doc: &Doc) -> usize {
        let position = self.grid_position_to_doc_position(position, doc);

        CharIterator::new(&doc.lines()[position.y][..position.x]).count()
    }

    fn grid_position_to_doc_position(&self, position: Position, doc: &Doc) -> Position {
        Position::new(position.x, self.grid_y_to_doc_y(position.y, doc))
    }

    fn doc_position_to_grid_position(&self, position: Position, doc: &Doc) -> Position {
        Position::new(position.x, self.doc_y_to_grid_y(position.y, doc))
    }

    fn grid_y_to_doc_y(&self, y: usize, doc: &Doc) -> usize {
        doc.lines().len().saturating_sub(self.grid_height) + y
    }

    fn doc_y_to_grid_y(&self, y: usize, doc: &Doc) -> usize {
        y.saturating_sub(doc.lines().len().saturating_sub(self.grid_height))
    }

    fn jump_doc_cursors_to_grid_cursor(&mut self, doc: &mut Doc, gfx: &mut Gfx) {
        if !self.is_cursor_visible {
            return;
        }

        self.did_doc_cursors_move = true;

        let doc_position =
            self.grid_position_to_doc_position(self.clamp_position(self.grid_cursor, doc), doc);
        doc.jump_cursors(doc_position, false, gfx);
    }

    fn move_cursor(&mut self, delta_x: isize, delta_y: isize, doc: &mut Doc, gfx: &mut Gfx) {
        self.grid_cursor = self.move_position(self.grid_cursor, delta_x, delta_y, doc);
        self.jump_doc_cursors_to_grid_cursor(doc, gfx);
    }

    fn jump_cursor(&mut self, position: Position, doc: &mut Doc, gfx: &mut Gfx) {
        self.grid_cursor = self.clamp_position(position, doc);
        self.jump_doc_cursors_to_grid_cursor(doc, gfx);
    }

    fn newline_cursor(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        if self.grid_cursor.y == self.scroll_bottom {
            self.scroll_grid_region_up(self.scroll_top..=self.scroll_bottom, doc, ctx);
        } else {
            self.move_cursor(0, 1, doc, ctx.gfx);
        }
    }

    fn reverse_newline_cursor(&mut self, doc: &mut Doc, ctx: &mut Ctx) {
        if self.grid_cursor.y == self.scroll_top {
            self.scroll_grid_region_down(self.scroll_top..=self.scroll_bottom, doc, ctx);
        } else {
            self.move_cursor(0, -1, doc, ctx.gfx);
        }
    }

    fn insert_at_cursor(&mut self, text: &str, doc: &mut Doc, ctx: &mut Ctx) {
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

    fn highlight_lines(&mut self, doc: &mut Doc) {
        for y in 0..self.colored_grid_lines.len() {
            let doc_y = self.grid_y_to_doc_y(y, doc);
            let colored_line = &mut self.colored_grid_lines[y];

            if !colored_line.is_dirty {
                continue;
            }

            let line_len = colored_line.colors.len().min(doc.line_len(doc_y));
            let colors = &colored_line.colors[..line_len];

            doc.highlight_line_from_terminal_colors(colors, doc_y);

            colored_line.is_dirty = false;
        }
    }

    fn delete(&mut self, start: Position, end: Position, doc: &mut Doc, ctx: &mut Ctx) {
        for y in start.y..=end.y {
            let start_x = if y == start.y { start.x } else { 0 };

            let end_x = if y == end.y {
                end.x
            } else {
                self.line_len(y, doc)
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
                iter::repeat_n(colors, c.len()),
            );

            position = self.move_position(position, 1, 0, doc);
            c = "\u{200B}";
        }

        position
    }

    pub fn doc<'a>(&self, docs: &'a TerminalDocs) -> &'a Doc {
        if self.is_in_alternate_buffer {
            &docs.alternate
        } else {
            &docs.normal
        }
    }

    pub fn doc_mut<'a>(&self, docs: &'a mut TerminalDocs) -> &'a mut Doc {
        if self.is_in_alternate_buffer {
            &mut docs.alternate
        } else {
            &mut docs.normal
        }
    }

    pub fn pty(&mut self) -> Option<&mut Process> {
        self.pty.as_mut()
    }
}
