use std::ops::Range;
use std::time::Instant;

use eframe::egui;
use egui::{
    text::LayoutJob,
    Color32,
    Context,
    FontId,
    Id,
    Key,
    Rect,
    Response,
    ScrollArea,
    Sense,
    Ui,
    Vec2,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::Theme;
use syntect::parsing::{SyntaxReference, SyntaxSet};

const FONT_SIZE: f32 = 14.0;
const TEXT_LEFT_PADDING: f32 = 7.0;
const TEXT_TOP_PADDING: f32 = 4.0;
const GUTTER_RIGHT_PADDING: f32 = 7.0;
const INDENT: &str = "    ";

const BACKGROUND: Color32 = Color32::from_rgb(30, 30, 30);
const GUTTER_BACKGROUND: Color32 = Color32::from_rgb(27, 27, 27);
const CURRENT_LINE_BACKGROUND: Color32 = Color32::from_rgb(43, 45, 52);
const SELECTION_BACKGROUND: Color32 = Color32::from_rgb(63, 92, 140);
const LINE_NUMBER_COLOR: Color32 = Color32::from_rgb(105, 105, 105);
const CURSOR_COLOR: Color32 = Color32::WHITE;
const SEPARATOR_COLOR: Color32 = Color32::from_rgb(40, 40, 40);

#[derive(Clone, Copy)]
struct StyleRange {
    start: usize,
    end: usize,
    color: Color32,
}

#[derive(Default)]
struct CachedLine {
    ranges: Vec<StyleRange>,
}

pub struct Highlighter {
    syntax: SyntaxReference,
    theme: Theme,
    lines: Vec<Option<CachedLine>>,
    revisions: Vec<u64>,
    revision: u64,
}

impl Highlighter {
    pub fn new(
        syntax_set: &SyntaxSet,
        theme_set: &syntect::highlighting::ThemeSet,
        language: &str,
    ) -> Self {
        let syntax = syntax_set
            .find_syntax_by_name(language)
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

        let theme = theme_set.themes["base16-ocean.dark"].clone();

        Self {
            syntax: syntax.clone(),
            theme,
            lines: Vec::new(),
            revisions: Vec::new(),
            revision: 0,
        }
    }

    fn sync_line_count(&mut self, count: usize) {
        self.lines.resize_with(count, || None);
        self.revisions.resize(count, 0);
    }

    fn invalidate_from(&mut self, line: usize, count: usize) {
        self.revision = self.revision.wrapping_add(1);
        self.sync_line_count(count);

        for index in line..self.lines.len() {
            self.lines[index] = None;
            self.revisions[index] = 0;
        }
    }

    fn ensure_range(
        &mut self,
        text: &str,
        line_starts: &[usize],
        start_line: usize,
        end_line: usize,
        syntax_set: &SyntaxSet,
    ) {
        if line_starts.is_empty() || start_line > end_line {
            return;
        }

        let last_line = end_line.min(line_starts.len() - 1);

        let first_missing = (0..=last_line).find(|line| {
            *line >= start_line
                && (
                    self.lines[*line].is_none()
                        || self.revisions[*line] != self.revision
                )
        });

        let Some(first_missing) = first_missing else {
            return;
        };

        let syntax = self.syntax.clone();
        let theme = self.theme.clone();

        let mut highlighter = HighlightLines::new(
            &syntax,
            &theme,
        );

        let mut line = 0usize;

        while line <= last_line {
            let line_start = line_starts[line];

            let line_end = if line + 1 < line_starts.len() {
                line_starts[line + 1]
            } else {
                text.len()
            };

            let line_text = &text[line_start..line_end];

            let should_cache =
                line >= first_missing
                    && (
                        self.lines[line].is_none()
                            || self.revisions[line] != self.revision
                    );

            if should_cache {
                let mut cached = CachedLine::default();

                match highlighter.highlight_line(
                    line_text,
                    syntax_set,
                ) {
                    Ok(ranges) => {
                        let mut offset = 0usize;

                        for (style, chunk) in ranges {
                            if chunk.is_empty() {
                                continue;
                            }

                            let start = offset;
                            offset += chunk.len();

                            cached.ranges.push(StyleRange {
                                start,
                                end: offset,
                                color: Color32::from_rgb(
                                    style.foreground.r,
                                    style.foreground.g,
                                    style.foreground.b,
                                ),
                            });
                        }
                    }
                    Err(_) => {
                        let visible_end = display_end(line_text);

                        if visible_end > 0 {
                            cached.ranges.push(StyleRange {
                                start: 0,
                                end: visible_end,
                                color: Color32::WHITE,
                            });
                        }
                    }
                }

                self.lines[line] = Some(cached);
                self.revisions[line] = self.revision;
            }

            line += 1;
        }
    }

    fn line_job(
        &self,
        text: &str,
        line: usize,
        line_starts: &[usize],
        font_id: &FontId,
    ) -> LayoutJob {
        let mut job = LayoutJob::default();

        if line >= line_starts.len() {
            return job;
        }

        let start = line_starts[line];

        let end = if line + 1 < line_starts.len() {
            line_starts[line + 1]
        } else {
            text.len()
        };

        let line_text = &text[start..end];
        let visible_end = display_end(line_text);
        let visible_text = &line_text[..visible_end];

        let default_format = egui::TextFormat {
            font_id: font_id.clone(),
            color: Color32::WHITE,
            background: Color32::TRANSPARENT,
            ..Default::default()
        };

        let Some(Some(cached)) = self.lines.get(line) else {
            job.append(
                visible_text,
                0.0,
                default_format,
            );

            return job;
        };

        let mut cursor = 0usize;

        for range in &cached.ranges {
            let range_start = range.start.min(visible_end);
            let range_end = range.end.min(visible_end);

            if range_start > cursor {
                job.append(
                    &visible_text[cursor..range_start],
                    0.0,
                    default_format.clone(),
                );
            }

            if range_end > range_start {
                job.append(
                    &visible_text[range_start..range_end],
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: range.color,
                        background: Color32::TRANSPARENT,
                        ..Default::default()
                    },
                );

                cursor = range_end;
            }
        }

        if cursor < visible_end {
            job.append(
                &visible_text[cursor..visible_end],
                0.0,
                default_format,
            );
        }

        job
    }
}

fn display_end(line: &str) -> usize {
    line.strip_suffix('\n')
        .map(|value| value.len())
        .unwrap_or(line.len())
}

pub struct CodeEditor {
    text: String,
    cursor: usize,
    selection: Option<Range<usize>>,
    selection_anchor: usize,

    line_starts: Vec<usize>,
    line_char_counts: Vec<usize>,
    max_line_chars: usize,
    char_count: usize,

    highlighter: Highlighter,
}

impl CodeEditor {
    pub fn new(
        syntax_set: &SyntaxSet,
        theme_set: &syntect::highlighting::ThemeSet,
        language: &str,
        initial_text: &str,
    ) -> Self {
        let mut editor = Self {
            text: initial_text.to_string(),
            cursor: 0,
            selection: None,
            selection_anchor: 0,
            line_starts: Vec::new(),
            line_char_counts: Vec::new(),
            max_line_chars: 1,
            char_count: initial_text.chars().count(),
            highlighter: Highlighter::new(
                syntax_set,
                theme_set,
                language,
            ),
        };

        editor.rebuild_line_index();

        editor
            .highlighter
            .sync_line_count(editor.line_count());

        editor
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        id: Id,
        syntax_set: &SyntaxSet,
    ) -> Response {
        let font_id = FontId::monospace(FONT_SIZE);

        let row_height = ui.fonts(|fonts| {
            fonts.row_height(&font_id)
        });

        let digit_width = ui.fonts(|fonts| {
            fonts.glyph_width(&font_id, '0')
        });

        let gutter_width = self.gutter_width(
            ui,
            &font_id,
        );

        let text_x =
            gutter_width + TEXT_LEFT_PADDING;

        let total_lines = self.line_count();

        let total_height =
            total_lines as f32 * row_height
                + TEXT_TOP_PADDING * 2.0;

        let content_width =
            ui.available_width().max(
                text_x
                    + self.max_line_chars as f32
                        * digit_width
                    + 32.0,
            );

        ScrollArea::both()
            .id_source(id)
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                let content_rect = Rect::from_min_size(
                    ui.min_rect().min,
                    Vec2::new(
                        content_width,
                        total_height,
                    ),
                );

                ui.allocate_rect(
                    content_rect,
                    Sense::hover(),
                );

                let visible_start = (
                    (viewport.min.y
                        - TEXT_TOP_PADDING)
                        / row_height
                )
                    .floor()
                    .max(0.0) as usize;

                let visible_end = (
                    (viewport.max.y
                        - TEXT_TOP_PADDING)
                        / row_height
                )
                    .ceil()
                    .max(0.0) as usize;

                let visible_start = visible_start.min(
                    total_lines.saturating_sub(1),
                );

                let visible_end = visible_end.min(
                    total_lines.saturating_sub(1),
                );

                self.highlighter.ensure_range(
                    &self.text,
                    &self.line_starts,
                    visible_start,
                    visible_end,
                    syntax_set,
                );

                let response = ui.interact(
                    content_rect,
                    id.with("interaction"),
                    Sense::click_and_drag(),
                );

                if response.clicked() {
                    response.request_focus();

                    if let Some(position) =
                        response.interact_pointer_pos()
                    {
                        let cursor =
                            self.position_to_cursor(
                                ui,
                                position,
                                content_rect,
                                gutter_width,
                                row_height,
                                &font_id,
                            );

                        self.cursor = cursor;
                        self.selection = None;
                        self.selection_anchor = cursor;
                    }
                }

                if response.dragged() {
                    if let Some(position) =
                        response.interact_pointer_pos()
                    {
                        let cursor =
                            self.position_to_cursor(
                                ui,
                                position,
                                content_rect,
                                gutter_width,
                                row_height,
                                &font_id,
                            );

                        self.cursor = cursor;

                        self.set_selection(
                            self.selection_anchor,
                            self.cursor,
                        );
                    }
                }

                if response.has_focus() {
                    self.handle_input(ui);
                }

                let painter = ui.painter();

                painter.rect_filled(
                    content_rect,
                    0.0,
                    BACKGROUND,
                );

                let gutter_rect = Rect::from_min_max(
                    content_rect.min,
                    egui::pos2(
                        content_rect.min.x + gutter_width,
                        content_rect.max.y,
                    ),
                );

                painter.rect_filled(
                    gutter_rect,
                    0.0,
                    GUTTER_BACKGROUND,
                );

                self.paint_current_line(
                    &painter,
                    content_rect,
                    gutter_width,
                    row_height,
                    visible_start,
                    visible_end,
                );

                self.paint_selection(
                    &painter,
                    content_rect,
                    gutter_width,
                    row_height,
                    &font_id,
                    visible_start,
                    visible_end,
                );

                self.paint_code(
                    &painter,
                    content_rect,
                    text_x,
                    row_height,
                    &font_id,
                    visible_start,
                    visible_end,
                );

                self.paint_line_numbers(
                    &painter,
                    content_rect,
                    gutter_width,
                    row_height,
                    visible_start,
                    visible_end,
                    &font_id,
                );

                if response.has_focus()
                    && self.selection.is_none()
                {
                    self.paint_cursor(
                        &painter,
                        content_rect,
                        text_x,
                        row_height,
                        &font_id,
                    );
                }

                response
            })
            .inner
    }

    fn handle_input(
        &mut self,
        ui: &mut Ui,
    ) {
        let events = ui.input(|input| {
            input.events.clone()
        });

        for event in events {
            match event {
                egui::Event::Text(text) => {
                    self.insert_text(&text);
                }

                egui::Event::Paste(text) => {
                    self.insert_pasted_text(&text);
                }

                egui::Event::Copy => {
                    self.copy_selection(ui);
                }

                egui::Event::Cut => {
                    self.cut_selection(ui);
                }

                egui::Event::Key {
                    key,
                    pressed,
                    modifiers,
                    ..
                } => {
                    if !pressed {
                        continue;
                    }

                    if modifiers.ctrl {
                        match key {
                            Key::A => {
                                self.select_all();
                                continue;
                            }
                            Key::C => {
                                self.copy_selection(ui);
                                continue;
                            }
                            Key::X => {
                                self.cut_selection(ui);
                                continue;
                            }
                            Key::V => {
                                continue;
                            }
                            _ => {}
                        }
                    }

                    self.handle_key(
                        key,
                        modifiers.shift,
                    );
                }

                _ => {}
            }
        }
    }

    fn paint_code(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        text_x: f32,
        row_height: f32,
        font_id: &FontId,
        first_line: usize,
        last_line: usize,
    ) {
        for line in first_line..=last_line {
            let job = self.highlighter.line_job(
                &self.text,
                line,
                &self.line_starts,
                font_id,
            );

            let galley =
                painter.layout_job(job);

            let y = rect.min.y
                + TEXT_TOP_PADDING
                + line as f32 * row_height;

            painter.galley(
                egui::pos2(
                    rect.min.x + text_x,
                    y,
                ),
                galley,
                Color32::WHITE,
            );
        }
    }

    fn paint_current_line(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        gutter_width: f32,
        row_height: f32,
        first_line: usize,
        last_line: usize,
    ) {
        let line = self.cursor_line();

        if line < first_line
            || line > last_line
        {
            return;
        }

        let y = rect.min.y
            + TEXT_TOP_PADDING
            + line as f32 * row_height;

        painter.rect_filled(
            Rect::from_min_size(
                egui::pos2(
                    rect.min.x,
                    y,
                ),
                egui::vec2(
                    rect.width(),
                    row_height,
                ),
            ),
            0.0,
            CURRENT_LINE_BACKGROUND,
        );

        let separator_x =
            rect.min.x + gutter_width;

        painter.line_segment(
            [
                egui::pos2(
                    separator_x,
                    y,
                ),
                egui::pos2(
                    separator_x,
                    y + row_height,
                ),
            ],
            egui::Stroke::new(
                1.0_f32,
                SEPARATOR_COLOR,
            ),
        );
    }

    fn paint_line_numbers(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        gutter_width: f32,
        row_height: f32,
        first_line: usize,
        last_line: usize,
        font_id: &FontId,
    ) {
        for line in first_line..=last_line {
            let number =
                (line + 1).to_string();

            let galley =
                painter.layout_no_wrap(
                    number,
                    font_id.clone(),
                    LINE_NUMBER_COLOR,
                );

            let y =
                rect.min.y
                    + TEXT_TOP_PADDING
                    + line as f32
                        * row_height;

            let x =
                rect.min.x
                    + gutter_width
                    - GUTTER_RIGHT_PADDING
                    - galley.size().x;

            painter.galley(
                egui::pos2(
                    x,
                    y,
                ),
                galley,
                LINE_NUMBER_COLOR,
            );
        }
    }

    fn paint_selection(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        gutter_width: f32,
        row_height: f32,
        font_id: &FontId,
        first_line: usize,
        last_line: usize,
    ) {
        let Some(selection) =
            self.selection.as_ref()
        else {
            return;
        };

        if selection.start >= selection.end {
            return;
        }

        let selection_start_line =
            self.line_from_index(
                selection.start,
            );

        let selection_end_line =
            self.line_from_index(
                selection.end,
            );

        let start_line =
            selection_start_line.max(first_line);

        let end_line =
            selection_end_line.min(last_line);

        for line in start_line..=end_line {
            let line_start =
                self.line_start(line);

            let line_end =
                self.line_end(line);

            let start =
                if line == selection_start_line {
                    selection.start.max(line_start)
                } else {
                    line_start
                };

            let end =
                if line == selection_end_line {
                    selection.end.min(line_end)
                } else {
                    line_end
                };

            let start_column =
                self.column_from_index(
                    line_start,
                    start,
                );

            let end_column =
                self.column_from_index(
                    line_start,
                    end,
                );

            let line_text =
                &self.text[line_start..line_end];

            let start_byte =
                char_to_byte(
                    line_text,
                    start_column,
                );

            let end_byte =
                char_to_byte(
                    line_text,
                    end_column,
                );

            let start_width =
                painter
                    .layout_no_wrap(
                        line_text[..start_byte].to_string(),
                        font_id.clone(),
                        Color32::WHITE,
                    )
                    .size()
                    .x;

            let end_width =
                painter
                    .layout_no_wrap(
                        line_text[..end_byte].to_string(),
                        font_id.clone(),
                        Color32::WHITE,
                    )
                    .size()
                    .x;

            let mut width =
                end_width - start_width;

            if width <= 0.0 {
                width =
                    painter
                        .layout_no_wrap(
                            " ".to_string(),
                            font_id.clone(),
                            Color32::WHITE,
                        )
                        .size()
                        .x;
            }

            let x =
                rect.min.x
                    + gutter_width
                    + TEXT_LEFT_PADDING
                    + start_width;

            let y =
                rect.min.y
                    + TEXT_TOP_PADDING
                    + line as f32 * row_height;

            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(
                        x,
                        y,
                    ),
                    egui::vec2(
                        width,
                        row_height,
                    ),
                ),
                0.0,
                SELECTION_BACKGROUND,
            );
        }
    }

    fn paint_cursor(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        text_x: f32,
        row_height: f32,
        font_id: &FontId,
    ) {
        let line =
            self.cursor_line();

        let line_start =
            self.line_start(line);

        let prefix =
            &self.text[
                line_start..self.cursor
            ];

        let width =
            painter
                .layout_no_wrap(
                    prefix.to_string(),
                    font_id.clone(),
                    Color32::WHITE,
                )
                .size()
                .x;

        let x =
            rect.min.x
                + text_x
                + width;

        let y =
            rect.min.y
                + TEXT_TOP_PADDING
                + line as f32
                    * row_height;

        painter.rect_filled(
            Rect::from_min_size(
                egui::pos2(
                    x,
                    y,
                ),
                egui::vec2(
                    1.5,
                    row_height,
                ),
            ),
            0.0,
            CURSOR_COLOR,
        );
    }

    fn position_to_cursor(
        &self,
        ui: &Ui,
        position: egui::Pos2,
        rect: Rect,
        gutter_width: f32,
        row_height: f32,
        font_id: &FontId,
    ) -> usize {
        let text_x =
            rect.min.x
                + gutter_width
                + TEXT_LEFT_PADDING;

        let text_y =
            rect.min.y
                + TEXT_TOP_PADDING;

        let line = (
            (position.y - text_y)
                / row_height
        )
            .floor()
            .max(0.0) as usize;

        let line =
            line.min(
                self.line_count()
                    .saturating_sub(1),
            );

        let line_start =
            self.line_start(line);

        let line_end =
            self.line_end(line);

        let line_text =
            &self.text[line_start..line_end];

        if line_text.is_empty() {
            return line_start;
        }

        let target_x =
            position.x - text_x;

        if target_x <= 0.0 {
            return line_start;
        }

        let mut previous_width =
            0.0_f32;

        let mut column = 0usize;

        for byte_end in
            char_boundaries(line_text)
        {
            let width =
                ui.fonts(|fonts| {
                    fonts
                        .layout_no_wrap(
                            line_text[..byte_end].to_string(),
                            font_id.clone(),
                            Color32::WHITE,
                        )
                        .size()
                        .x
                });

            let midpoint =
                (previous_width + width)
                    * 0.5_f32;

            if target_x < midpoint {
                break;
            }

            previous_width = width;
            column += 1;
        }

        self.byte_index_from_column(
            line_start,
            line_end,
            column,
        )
    }

    fn rebuild_line_index(
        &mut self,
    ) {
        self.line_starts.clear();
        self.line_char_counts.clear();

        self.line_starts.push(0);

        let mut line_start = 0usize;

        for (index, byte) in
            self.text.bytes().enumerate()
        {
            if byte == b'\n' {
                let count =
                    self.text[
                        line_start..index
                    ]
                    .chars()
                    .count();

                self.line_char_counts
                    .push(count);

                self.line_starts
                    .push(index + 1);

                line_start = index + 1;
            }
        }

        self.line_char_counts.push(
            self.text[line_start..]
                .chars()
                .count(),
        );

        self.max_line_chars =
            self.line_char_counts
                .iter()
                .copied()
                .max()
                .unwrap_or(1)
                .max(1);

        self.char_count =
            self.text.chars().count();
    }

    fn text_changed(
        &mut self,
        edit_line: usize,
    ) {
        self.rebuild_line_index();

        self.highlighter.invalidate_from(
            edit_line,
            self.line_count(),
        );
    }

    fn line_count(
        &self,
    ) -> usize {
        self.line_starts.len().max(1)
    }

    fn line_from_index(
        &self,
        index: usize,
    ) -> usize {
        let index =
            index.min(self.text.len());

        match self.line_starts
            .binary_search(&index)
        {
            Ok(value) => value,
            Err(value) => value.saturating_sub(1),
        }
    }

    fn line_start(
        &self,
        line: usize,
    ) -> usize {
        *self
            .line_starts
            .get(line)
            .unwrap_or(&self.text.len())
    }

    fn line_end(
        &self,
        line: usize,
    ) -> usize {
        if line + 1 < self.line_starts.len() {
            self.line_starts[line + 1]
                .saturating_sub(1)
        } else {
            self.text.len()
        }
    }

    fn cursor_line(
        &self,
    ) -> usize {
        self.line_from_index(self.cursor)
    }

    fn cursor_position(
        &self,
    ) -> usize {
        let line =
            self.cursor_line();

        self.column_from_index(
            self.line_start(line),
            self.cursor,
        )
    }

    fn column_from_index(
        &self,
        line_start: usize,
        index: usize,
    ) -> usize {
        self.text[
            line_start
                ..index.min(self.text.len())
        ]
        .chars()
        .count()
    }

    fn byte_index_from_column(
        &self,
        line_start: usize,
        line_end: usize,
        column: usize,
    ) -> usize {
        if column == 0 {
            return line_start;
        }

        let mut current = 0usize;

        for (offset, character) in
            self.text[
                line_start..line_end
            ]
            .char_indices()
        {
            if current == column {
                return line_start + offset;
            }

            current += 1;

            if current == column {
                return line_start
                    + offset
                    + character.len_utf8();
            }
        }

        line_end
    }

    fn gutter_width(
        &self,
        ui: &Ui,
        font_id: &FontId,
    ) -> f32 {
        let number =
            self.line_count().to_string();

        let width =
            ui.fonts(|fonts| {
                fonts
                    .layout_no_wrap(
                        number,
                        font_id.clone(),
                        LINE_NUMBER_COLOR,
                    )
                    .size()
                    .x
            });

        width + GUTTER_RIGHT_PADDING
    }

    fn set_selection(
        &mut self,
        anchor: usize,
        cursor: usize,
    ) {
        if anchor == cursor {
            self.selection = None;
            return;
        }

        self.selection =
            if anchor < cursor {
                Some(anchor..cursor)
            } else {
                Some(cursor..anchor)
            };
    }

    fn select_all(
        &mut self,
    ) {
        if self.text.is_empty() {
            self.selection = None;
            return;
        }

        self.selection =
            Some(0..self.text.len());

        self.selection_anchor = 0;
        self.cursor = self.text.len();
    }

    fn selected_text(
        &self,
    ) -> Option<&str> {
        let range =
            self.selection.as_ref()?;

        if range.start >= range.end {
            return None;
        }

        self.text.get(
            range.start..range.end,
        )
    }

    fn copy_selection(
        &self,
        ui: &mut Ui,
    ) {
        if let Some(text) =
            self.selected_text()
        {
            ui.output_mut(|output| {
                output.copied_text =
                    text.to_string();
            });
        }
    }

    fn cut_selection(
        &mut self,
        ui: &mut Ui,
    ) {
        let Some(text) =
            self.selected_text()
        else {
            return;
        };

        ui.output_mut(|output| {
            output.copied_text =
                text.to_string();
        });

        self.delete_selection();
    }

    fn delete_selection(
        &mut self,
    ) -> bool {
        let Some(range) =
            self.selection.take()
        else {
            return false;
        };

        if range.start >= range.end {
            return false;
        }

        let line =
            self.line_from_index(
                range.start,
            );

        self.text.replace_range(
            range.start..range.end,
            "",
        );

        self.cursor = range.start;
        self.selection_anchor = self.cursor;

        self.text_changed(line);

        true
    }

    fn current_char(
        &self,
    ) -> Option<char> {
        self.text[self.cursor..]
            .chars()
            .next()
    }

    fn previous_char(
        &self,
    ) -> Option<char> {
        if self.cursor == 0 {
            None
        } else {
            self.text[..self.cursor]
                .chars()
                .next_back()
        }
    }

    fn previous_char_boundary(
        &self,
        index: usize,
    ) -> usize {
        if index == 0 {
            return 0;
        }

        let mut position =
            index - 1;

        while position > 0
            && !self
                .text
                .is_char_boundary(position)
        {
            position -= 1;
        }

        position
    }

    fn next_char_boundary(
        &self,
        index: usize,
    ) -> usize {
        if index >= self.text.len() {
            return self.text.len();
        }

        let mut position =
            index + 1;

        while position < self.text.len()
            && !self
                .text
                .is_char_boundary(position)
        {
            position += 1;
        }

        position
    }

    fn insert_raw(
        &mut self,
        text: &str,
        edit_line: usize,
    ) {
        self.text.insert_str(
            self.cursor,
            text,
        );

        self.cursor += text.len();
        self.selection_anchor =
            self.cursor;
        self.selection = None;

        self.text_changed(edit_line);
    }

    fn insert_text(
        &mut self,
        text: &str,
    ) {
        if text.is_empty() {
            return;
        }

        if text == "\n" {
            self.insert_newline();
            return;
        }

        if text.chars().count() == 1 {
            let character =
                text.chars().next().unwrap();

            match character {
                '(' => {
                    self.insert_pair(
                        '(',
                        ')',
                    );
                    return;
                }
                '[' => {
                    self.insert_pair(
                        '[',
                        ']',
                    );
                    return;
                }
                '{' => {
                    self.insert_pair(
                        '{',
                        '}',
                    );
                    return;
                }
                ')' => {
                    self.insert_closing_bracket(
                        ')',
                    );
                    return;
                }
                ']' => {
                    self.insert_closing_bracket(
                        ']',
                    );
                    return;
                }
                '}' => {
                    self.insert_closing_bracket(
                        '}',
                    );
                    return;
                }
                '"' => {
                    self.insert_quote('"');
                    return;
                }
                _ => {}
            }
        }

        let line =
            self.cursor_line();

        self.delete_selection();

        self.insert_raw(
            text,
            line,
        );
    }

    fn insert_pasted_text(
        &mut self,
        text: &str,
    ) {
        if text.is_empty() {
            return;
        }

        let line =
            self.cursor_line();

        self.delete_selection();

        let normalized =
            text.replace("\r\n", "\n")
                .replace('\r', "\n");

        self.insert_raw(
            &normalized,
            line,
        );
    }

    fn insert_pair(
        &mut self,
        open: char,
        close: char,
    ) {
        let line =
            self.cursor_line();

        if let Some(range) =
            self.selection.take()
        {
            let selected =
                self.text[
                    range.start..range.end
                ]
                .to_string();

            self.text.replace_range(
                range.start..range.end,
                "",
            );

            self.cursor = range.start;

            self.text.insert(
                self.cursor,
                open,
            );

            self.cursor += open.len_utf8();

            self.text.insert_str(
                self.cursor,
                &selected,
            );

            self.cursor +=
                selected.len();

            self.text.insert(
                self.cursor,
                close,
            );

            self.selection_anchor =
                self.cursor;

            self.text_changed(line);
            return;
        }

        self.text.insert(
            self.cursor,
            open,
        );

        self.cursor += open.len_utf8();

        self.text.insert(
            self.cursor,
            close,
        );

        self.selection_anchor =
            self.cursor;

        self.text_changed(line);
    }

    fn insert_closing_bracket(
        &mut self,
        close: char,
    ) {
        if self.current_char() == Some(close) {
            self.cursor =
                self.next_char_boundary(
                    self.cursor,
                );

            self.selection_anchor =
                self.cursor;

            self.selection = None;
            return;
        }

        let line =
            self.cursor_line();

        self.delete_selection();

        self.insert_raw(
            &close.to_string(),
            line,
        );
    }

    fn insert_quote(
        &mut self,
        quote: char,
    ) {
        if self.current_char() == Some(quote) {
            self.cursor =
                self.next_char_boundary(
                    self.cursor,
                );

            self.selection_anchor =
                self.cursor;

            self.selection = None;
            return;
        }

        if self.previous_char() == Some('\\') {
            let line =
                self.cursor_line();

            self.delete_selection();

            self.insert_raw(
                &quote.to_string(),
                line,
            );

            return;
        }

        self.insert_pair(
            quote,
            quote,
        );
    }

    fn insert_newline(
        &mut self,
    ) {
        let line =
            self.cursor_line();

        self.delete_selection();

        let line_start =
            self.line_start(line);

        let prefix =
            &self.text[
                line_start..self.cursor
            ];

        let indentation: String =
            prefix
                .chars()
                .take_while(|value| {
                    *value == ' '
                        || *value == '\t'
                })
                .collect();

        let previous =
            self.previous_char();

        let next =
            self.current_char();

        if previous == Some('{')
            && next == Some('}')
        {
            let insertion = format!(
                "\n{}{}\n{}",
                indentation,
                INDENT,
                indentation,
            );

            self.text.insert_str(
                self.cursor,
                &insertion,
            );

            self.cursor +=
                indentation.len()
                    + 1
                    + INDENT.len();

            self.selection_anchor =
                self.cursor;

            self.text_changed(line);
            return;
        }

        let insertion =
            if previous == Some('{') {
                format!(
                    "\n{}{}",
                    indentation,
                    INDENT,
                )
            } else {
                format!(
                    "\n{}",
                    indentation,
                )
            };

        self.text.insert_str(
            self.cursor,
            &insertion,
        );

        self.cursor +=
            insertion.len();

        self.selection_anchor =
            self.cursor;

        self.text_changed(line);
    }

    fn handle_key(
        &mut self,
        key: Key,
        shift: bool,
    ) {
        match key {
            Key::Enter =>
                self.insert_newline(),

            Key::Tab =>
                self.insert_text(INDENT),

            Key::Backspace =>
                self.backspace(),

            Key::Delete =>
                self.delete_forward(),

            Key::ArrowLeft =>
                self.move_horizontal(
                    -1,
                    shift,
                ),

            Key::ArrowRight =>
                self.move_horizontal(
                    1,
                    shift,
                ),

            Key::ArrowUp =>
                self.move_vertical(
                    -1,
                    shift,
                ),

            Key::ArrowDown =>
                self.move_vertical(
                    1,
                    shift,
                ),

            Key::Home =>
                self.move_to(
                    self.line_start(
                        self.cursor_line(),
                    ),
                    shift,
                ),

            Key::End =>
                self.move_to(
                    self.line_end(
                        self.cursor_line(),
                    ),
                    shift,
                ),

            _ => {}
        }
    }

    fn backspace(
        &mut self,
    ) {
        if self.delete_selection() {
            return;
        }

        if self.cursor == 0 {
            return;
        }

        let previous =
            self.previous_char();

        let current =
            self.current_char();

        if matches!(
            (previous, current),
            (Some('('), Some(')'))
                | (Some('['), Some(']'))
                | (Some('{'), Some('}'))
        ) {
            let line =
                self.cursor_line();

            let next =
                self.next_char_boundary(
                    self.cursor,
                );

            let previous_cursor =
                self.previous_char_boundary(
                    self.cursor,
                );

            self.text.replace_range(
                self.cursor..next,
                "",
            );

            self.text.replace_range(
                previous_cursor..self.cursor,
                "",
            );

            self.cursor =
                previous_cursor;

            self.selection_anchor =
                self.cursor;

            self.text_changed(line);
            return;
        }

        let line =
            self.cursor_line();

        let previous_cursor =
            self.previous_char_boundary(
                self.cursor,
            );

        self.text.replace_range(
            previous_cursor..self.cursor,
            "",
        );

        self.cursor =
            previous_cursor;

        self.selection_anchor =
            self.cursor;

        self.text_changed(line);
    }

    fn delete_forward(
        &mut self,
    ) {
        if self.delete_selection() {
            return;
        }

        if self.cursor >= self.text.len() {
            return;
        }

        let line =
            self.cursor_line();

        let next =
            self.next_char_boundary(
                self.cursor,
            );

        self.text.replace_range(
            self.cursor..next,
            "",
        );

        self.text_changed(line);
    }

    fn move_horizontal(
        &mut self,
        direction: i32,
        shift: bool,
    ) {
        if !shift && self.selection.is_some() {
            let range =
                self.selection.take().unwrap();

            self.cursor =
                if direction < 0 {
                    range.start
                } else {
                    range.end
                };

            self.selection_anchor =
                self.cursor;

            return;
        }

        self.cursor =
            if direction < 0 {
                self.previous_char_boundary(
                    self.cursor,
                )
            } else {
                self.next_char_boundary(
                    self.cursor,
                )
            };

        if shift {
            self.set_selection(
                self.selection_anchor,
                self.cursor,
            );
        } else {
            self.selection_anchor =
                self.cursor;

            self.selection = None;
        }
    }

    fn move_vertical(
        &mut self,
        direction: i32,
        shift: bool,
    ) {
        let current_line =
            self.cursor_line();

        let target_line =
            if direction < 0 {
                current_line.checked_sub(1)
            } else if current_line + 1
                < self.line_count()
            {
                Some(current_line + 1)
            } else {
                None
            };

        let Some(target_line) = target_line else {
            return;
        };

        let column =
            self.cursor_position();

        let target_start =
            self.line_start(target_line);

        let target_end =
            self.line_end(target_line);

        let target_column =
            column.min(
                self.line_char_counts[target_line],
            );

        self.cursor =
            self.byte_index_from_column(
                target_start,
                target_end,
                target_column,
            );

        if shift {
            self.set_selection(
                self.selection_anchor,
                self.cursor,
            );
        } else {
            self.selection_anchor =
                self.cursor;

            self.selection = None;
        }
    }

    fn move_to(
        &mut self,
        target: usize,
        shift: bool,
    ) {
        self.cursor = target;

        if shift {
            self.set_selection(
                self.selection_anchor,
                self.cursor,
            );
        } else {
            self.selection_anchor =
                self.cursor;

            self.selection = None;
        }
    }
}

fn char_boundaries(
    text: &str,
) -> Vec<usize> {
    let mut result =
        Vec::with_capacity(
            text.chars().count(),
        );

    for (index, character) in text.char_indices() {
        result.push(
            index + character.len_utf8(),
        );
    }

    result
}

fn char_to_byte(
    text: &str,
    column: usize,
) -> usize {
    if column == 0 {
        return 0;
    }

    let mut current = 0usize;

    for (index, character) in text.char_indices() {
        if current == column {
            return index;
        }

        current += 1;

        if current == column {
            return index + character.len_utf8();
        }
    }

    text.len()
}

struct CodeApp {
    syntax_set: SyntaxSet,
    theme_set: syntect::highlighting::ThemeSet,
    editors: Vec<CodeEditor>,
    current_tab: usize,
    show_panel: bool,
    tab_names: Vec<String>,
    new_tab_language: String,
    startup_time: Instant,
    startup_logged: bool,
}

impl CodeApp {
    fn new(
        _cc: &eframe::CreationContext<'_>,
        startup_time: Instant,
    ) -> Self {
        let syntax_set =
            SyntaxSet::load_defaults_newlines();

        let theme_set =
            syntect::highlighting::ThemeSet::load_defaults();

        let initial_text = r#"fn main() {
    println!("Hello, world!");
}"#;

        let editor = CodeEditor::new(
            &syntax_set,
            &theme_set,
            "Rust",
            initial_text,
        );

        Self {
            syntax_set,
            theme_set,
            editors: vec![editor],
            current_tab: 0,
            show_panel: true,
            tab_names: vec![
                "Tab 1".to_string(),
            ],
            new_tab_language: "Rust".to_string(),
            startup_time,
            startup_logged: false,
        }
    }
}

impl eframe::App for CodeApp {
    fn update(
        &mut self,
        ctx: &Context,
        _frame: &mut eframe::Frame,
    ) {
        if !self.startup_logged {
            let elapsed_ms =
                self.startup_time.elapsed().as_secs_f64()
                    * 1000.0;

            println!(
                "[startup] GUI ready in {:.2} ms",
                elapsed_ms
            );

            self.startup_logged = true;
        }

        egui::TopBottomPanel::top("menu")
            .exact_height(38.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    if ui.button("New Tab").clicked() {
                        let editor = CodeEditor::new(
                            &self.syntax_set,
                            &self.theme_set,
                            &self.new_tab_language,
                            "// new file",
                        );

                        self.editors.push(editor);

                        self.tab_names.push(format!(
                            "Tab {}",
                            self.editors.len()
                        ));

                        self.current_tab =
                            self.editors.len() - 1;
                    }

                    if ui
                        .add_enabled(
                            self.editors.len() > 1,
                            egui::Button::new("Close Tab"),
                        )
                        .clicked()
                    {
                        self.editors.remove(self.current_tab);
                        self.tab_names.remove(self.current_tab);

                        if self.current_tab >= self.editors.len() {
                            self.current_tab =
                                self.editors.len() - 1;
                        }
                    }

                    ui.separator();
                    ui.label("Language:");

                    egui::ComboBox::from_id_source("lang_combo")
                        .width(120.0)
                        .selected_text(
                            &self.new_tab_language,
                        )
                        .show_ui(ui, |ui| {
                            for language in &[
                                "Rust",
                                "Python",
                                "JavaScript",
                                "C",
                                "C++",
                                "Plain Text",
                            ] {
                                ui.selectable_value(
                                    &mut self.new_tab_language,
                                    language.to_string(),
                                    *language,
                                );
                            }
                        });

                    ui.separator();

                    if ui.button("Toggle Panel").clicked() {
                        self.show_panel = !self.show_panel;
                    }
                });
            });

        if self.show_panel {
            egui::SidePanel::left("side_panel")
                .resizable(true)
                .default_width(170.0)
                .min_width(120.0)
                .max_width(280.0)
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.heading("Tabs");
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);

                    for (index, name) in
                        self.tab_names.iter().enumerate()
                    {
                        if ui
                            .add_sized(
                                [ui.available_width(), 28.0],
                                egui::SelectableLabel::new(
                                    self.current_tab == index,
                                    name,
                                ),
                            )
                            .clicked()
                        {
                            self.current_tab = index;
                        }

                        ui.add_space(2.0);
                    }
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(BACKGROUND)
                    .inner_margin(0.0),
            )
            .show(ctx, |ui| {
                if let Some(editor) =
                    self.editors.get_mut(self.current_tab)
                {
                    let editor_id =
                        ui.id().with("code_editor");

                    editor.ui(
                        ui,
                        editor_id,
                        &self.syntax_set,
                    );
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("No editor");
                    });
                }
            });

        egui::TopBottomPanel::bottom("status")
            .exact_height(26.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;

                    ui.label(format!(
                        "Tab {}",
                        self.current_tab + 1
                    ));

                    if let Some(editor) =
                        self.editors.get(self.current_tab)
                    {
                        ui.label(format!(
                            "Lines: {}",
                            editor.line_count()
                        ));

                        ui.label(format!(
                            "Chars: {}",
                            editor.char_count
                        ));

                        if let Some(selection) =
                            &editor.selection
                        {
                            let selected =
                                editor.text[
                                    selection.start..selection.end
                                ]
                                .chars()
                                .count();

                            if selected > 0 {
                                ui.label(format!(
                                    "Selected: {}",
                                    selected
                                ));
                            }
                        }

                        ui.with_layout(
                            egui::Layout::right_to_left(
                                egui::Align::Center,
                            ),
                            |ui| {
                                ui.label(format!(
                                    "Cursor: {}:{}",
                                    editor.cursor_line() + 1,
                                    editor.cursor_position() + 1
                                ));
                            },
                        );
                    }
                });
            });
    }
}

fn main() -> eframe::Result<()> {
    let startup_time = Instant::now();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([700.0, 450.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ECode",
        options,
        Box::new(move |cc| {
            Box::new(CodeApp::new(
                cc,
                startup_time,
            ))
        }),
    )
}