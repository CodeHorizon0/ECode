use eframe::egui::{self, Color32, FontId, Id, Rect, Response, ScrollArea, Sense, Ui, Vec2};
use syntect::parsing::SyntaxSet;

use crate::config::{
    BACKGROUND, CURSOR_COLOR, CURRENT_LINE_BACKGROUND, FONT_SIZE, GUTTER_BACKGROUND,
    GUTTER_RIGHT_PADDING, LINE_NUMBER_COLOR, SELECTION_BACKGROUND, SEPARATOR_COLOR,
    TEXT_BOTTOM_PADDING, TEXT_LEFT_PADDING, TEXT_TOP_PADDING,
};

use super::geometry::{char_boundaries, char_to_byte};
use super::CodeEditor;

pub fn render(
    editor: &mut CodeEditor,
    ui: &mut Ui,
    id: Id,
    syntax_set: &SyntaxSet,
) -> Response {
    if editor.focus_requested {
        ui.memory_mut(|memory| {
            memory.request_focus(id.with("interaction"));
        });
        editor.focus_requested = false;
    }

    let font_id = FontId::monospace(FONT_SIZE);
    let row_height = ui.fonts(|fonts| fonts.row_height(&font_id));
    let digit_width = ui.fonts(|fonts| fonts.glyph_width(&font_id, '0'));
    let gutter_width = gutter_width(editor, ui, &font_id);
    let text_x = gutter_width + TEXT_LEFT_PADDING;
    let total_lines = editor.line_count();
    let total_height =
        total_lines as f32 * row_height + TEXT_TOP_PADDING + TEXT_BOTTOM_PADDING;
    let content_width = ui.available_width().max(
        text_x + editor.max_line_chars as f32 * digit_width + 32.0,
    );

    ScrollArea::both()
        .id_source(id)
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            let content_rect = Rect::from_min_size(
                ui.min_rect().min,
                Vec2::new(content_width, total_height),
            );

            ui.allocate_rect(content_rect, Sense::hover());

            let visible_start = ((viewport.min.y - TEXT_TOP_PADDING) / row_height)
                .floor()
                .max(0.0) as usize;
            let visible_end = ((viewport.max.y - TEXT_TOP_PADDING) / row_height)
                .ceil()
                .max(0.0) as usize;

            let visible_start = visible_start.min(total_lines.saturating_sub(1));
            let visible_end = visible_end.min(total_lines.saturating_sub(1));

            editor.highlighter.ensure_range(
                &editor.text,
                &editor.line_starts,
                visible_start,
                visible_end,
                syntax_set,
            );

            let response = ui.interact(
                content_rect,
                id.with("interaction"),
                Sense::click_and_drag(),
            );

            let modifiers = ui.input(|input| input.modifiers);

            if response.double_clicked() {
                response.request_focus();
                if let Some(position) = response.interact_pointer_pos() {
                    let cursor = position_to_cursor(
                        editor,
                        ui,
                        position,
                        content_rect,
                        gutter_width,
                        row_height,
                        &font_id,
                    );
                    editor.cursor = cursor;
                    editor.select_word_at(cursor);
                }
            } else if response.triple_clicked() {
                response.request_focus();
                if let Some(position) = response.interact_pointer_pos() {
                    let cursor = position_to_cursor(
                        editor,
                        ui,
                        position,
                        content_rect,
                        gutter_width,
                        row_height,
                        &font_id,
                    );
                    editor.cursor = cursor;
                    editor.select_line_at(cursor);
                }
            } else if response.drag_started() {
                response.request_focus();
                if let Some(position) = response.interact_pointer_pos() {
                    let cursor = position_to_cursor(
                        editor,
                        ui,
                        position,
                        content_rect,
                        gutter_width,
                        row_height,
                        &font_id,
                    );
                    editor.cursor = cursor;
                    if !modifiers.shift {
                        editor.selection_anchor = cursor;
                        editor.selection = None;
                    }
                }
            } else if response.clicked() {
                response.request_focus();
                if let Some(position) = response.interact_pointer_pos() {
                    let cursor = position_to_cursor(
                        editor,
                        ui,
                        position,
                        content_rect,
                        gutter_width,
                        row_height,
                        &font_id,
                    );
                    editor.cursor = cursor;

                    if modifiers.shift {
                        editor.set_selection(editor.selection_anchor, cursor);
                    } else {
                        editor.selection_anchor = cursor;
                        editor.selection = None;
                    }
                }
            }

            if response.dragged() {
                if let Some(position) = response.interact_pointer_pos() {
                    let cursor = position_to_cursor(
                        editor,
                        ui,
                        position,
                        content_rect,
                        gutter_width,
                        row_height,
                        &font_id,
                    );
                    editor.cursor = cursor;
                    editor.set_selection(editor.selection_anchor, cursor);
                }
            }

            if response.has_focus() {
                editor.handle_input(ui);
            }

            let painter = ui.painter();
            painter.rect_filled(content_rect, 0.0, BACKGROUND);

            let gutter_rect = Rect::from_min_max(
                content_rect.min,
                egui::pos2(
                    content_rect.min.x + gutter_width,
                    content_rect.max.y,
                ),
            );
            painter.rect_filled(gutter_rect, 0.0, GUTTER_BACKGROUND);

            paint_current_line(
                editor,
                painter,
                content_rect,
                gutter_width,
                row_height,
                visible_start,
                visible_end,
            );
            paint_selection(
                editor,
                painter,
                content_rect,
                gutter_width,
                row_height,
                &font_id,
                visible_start,
                visible_end,
            );
            paint_code(
                editor,
                painter,
                content_rect,
                text_x,
                row_height,
                &font_id,
                visible_start,
                visible_end,
            );
            paint_line_numbers(
                painter,
                content_rect,
                gutter_width,
                row_height,
                visible_start,
                visible_end,
                &font_id,
            );

            if response.has_focus() && editor.selection.is_none() {
                paint_cursor(
                    editor,
                    painter,
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

fn gutter_width(editor: &CodeEditor, ui: &Ui, font_id: &FontId) -> f32 {
    let number = editor.line_count().to_string();
    let width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(number, font_id.clone(), LINE_NUMBER_COLOR)
            .size()
            .x
    });
    width + GUTTER_RIGHT_PADDING
}

fn paint_code(
    editor: &CodeEditor,
    painter: &egui::Painter,
    rect: Rect,
    text_x: f32,
    row_height: f32,
    font_id: &FontId,
    first_line: usize,
    last_line: usize,
) {
    for line in first_line..=last_line {
        let job = editor
            .highlighter
            .line_job(&editor.text, line, &editor.line_starts, font_id);
        let galley = painter.layout_job(job);
        let y = rect.min.y + TEXT_TOP_PADDING + line as f32 * row_height;

        painter.galley(
            egui::pos2(rect.min.x + text_x, y),
            galley,
            Color32::WHITE,
        );
    }
}

fn paint_current_line(
    editor: &CodeEditor,
    painter: &egui::Painter,
    rect: Rect,
    gutter_width: f32,
    row_height: f32,
    first_line: usize,
    last_line: usize,
) {
    let line = editor.cursor_line();
    if line < first_line || line > last_line {
        return;
    }

    let y = rect.min.y + TEXT_TOP_PADDING + line as f32 * row_height;

    painter.rect_filled(
        Rect::from_min_size(
            egui::pos2(rect.min.x, y),
            egui::vec2(rect.width(), row_height),
        ),
        0.0,
        CURRENT_LINE_BACKGROUND,
    );

    let separator_x = rect.min.x + gutter_width;
    painter.line_segment(
        [
            egui::pos2(separator_x, y),
            egui::pos2(separator_x, y + row_height),
        ],
        egui::Stroke::new(1.0_f32, SEPARATOR_COLOR),
    );
}

fn paint_line_numbers(
    painter: &egui::Painter,
    rect: Rect,
    gutter_width: f32,
    row_height: f32,
    first_line: usize,
    last_line: usize,
    font_id: &FontId,
) {
    for line in first_line..=last_line {
        let number = (line + 1).to_string();
        let galley = painter.layout_no_wrap(
            number,
            font_id.clone(),
            LINE_NUMBER_COLOR,
        );
        let y = rect.min.y + TEXT_TOP_PADDING + line as f32 * row_height;
        let x = rect.min.x + gutter_width - GUTTER_RIGHT_PADDING - galley.size().x;
        painter.galley(egui::pos2(x, y), galley, LINE_NUMBER_COLOR);
    }
}

fn paint_selection(
    editor: &CodeEditor,
    painter: &egui::Painter,
    rect: Rect,
    gutter_width: f32,
    row_height: f32,
    font_id: &FontId,
    first_line: usize,
    last_line: usize,
) {
    let Some(selection) = editor.selection.as_ref() else {
        return;
    };

    if selection.start >= selection.end {
        return;
    }

    let selection_start_line = editor.line_from_index(selection.start);
    let selection_end_line = editor.line_from_index(selection.end);
    let start_line = selection_start_line.max(first_line);
    let end_line = selection_end_line.min(last_line);

    for line in start_line..=end_line {
        let line_start = editor.line_start(line);
        let line_end = editor.line_end(line);
        let start = if line == selection_start_line {
            selection.start.max(line_start)
        } else {
            line_start
        };
        let end = if line == selection_end_line {
            selection.end.min(line_end)
        } else {
            line_end
        };

        let line_text = &editor.text[line_start..line_end];
        let start_column = editor.column_from_index(line_start, start);
        let end_column = editor.column_from_index(line_start, end);
        let start_byte = char_to_byte(line_text, start_column);
        let end_byte = char_to_byte(line_text, end_column);

        let start_width = painter
            .layout_no_wrap(
                line_text[..start_byte].to_string(),
                font_id.clone(),
                Color32::WHITE,
            )
            .size()
            .x;

        let mut width = painter
            .layout_no_wrap(
                line_text[start_byte..end_byte].to_string(),
                font_id.clone(),
                Color32::WHITE,
            )
            .size()
            .x;

        if width <= 0.0 {
            width = painter
                .layout_no_wrap(
                    " ".to_string(),
                    font_id.clone(),
                    Color32::WHITE,
                )
                .size()
                .x;
        }

        let x = rect.min.x + gutter_width + TEXT_LEFT_PADDING + start_width;
        let y = rect.min.y + TEXT_TOP_PADDING + line as f32 * row_height;

        painter.rect_filled(
            Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(width, row_height),
            ),
            0.0,
            SELECTION_BACKGROUND,
        );
    }
}

fn paint_cursor(
    editor: &CodeEditor,
    painter: &egui::Painter,
    rect: Rect,
    text_x: f32,
    row_height: f32,
    font_id: &FontId,
) {
    let line = editor.cursor_line();
    let line_start = editor.line_start(line);
    let prefix = &editor.text[line_start..editor.cursor];

    let width = painter
        .layout_no_wrap(
            prefix.to_string(),
            font_id.clone(),
            Color32::WHITE,
        )
        .size()
        .x;

    let x = rect.min.x + text_x + width;
    let y = rect.min.y + TEXT_TOP_PADDING + line as f32 * row_height;

    painter.rect_filled(
        Rect::from_min_size(
            egui::pos2(x, y),
            egui::vec2(1.5, row_height),
        ),
        0.0,
        CURSOR_COLOR,
    );
}

fn position_to_cursor(
    editor: &CodeEditor,
    ui: &Ui,
    position: egui::Pos2,
    rect: Rect,
    gutter_width: f32,
    row_height: f32,
    font_id: &FontId,
) -> usize {
    let text_x = rect.min.x + gutter_width + TEXT_LEFT_PADDING;
    let text_y = rect.min.y + TEXT_TOP_PADDING;

    let line = ((position.y - text_y) / row_height).floor().max(0.0) as usize;
    let line = line.min(editor.line_count().saturating_sub(1));
    let line_start = editor.line_start(line);
    let line_end = editor.line_end(line);
    let line_text = &editor.text[line_start..line_end];

    if line_text.is_empty() {
        return line_start;
    }

    let target_x = position.x - text_x;
    if target_x <= 0.0 {
        return line_start;
    }

    let mut previous_width = 0.0_f32;
    let mut column = 0usize;

    for byte_end in char_boundaries(line_text) {
        let width = ui.fonts(|fonts| {
            fonts
                .layout_no_wrap(
                    line_text[..byte_end].to_string(),
                    font_id.clone(),
                    Color32::WHITE,
                )
                .size()
                .x
        });

        let midpoint = (previous_width + width) * 0.5_f32;
        if target_x < midpoint {
            break;
        }

        previous_width = width;
        column += 1;
    }

    editor.byte_index_from_column(line_start, line_end, column)
}
