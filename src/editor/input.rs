use eframe::egui::{self, Key, Ui};

use super::state::BRACKET_PAIRS;
use super::CodeEditor;
use crate::config::INDENT;

impl CodeEditor {
    pub(super) fn handle_input(&mut self, ui: &mut Ui) {
        let events = ui.input(|input| input.events.clone());

        for event in events {
            match event {
                egui::Event::Text(text) => self.insert_text(&text),
                egui::Event::Paste(text) => self.insert_pasted_text(&text),
                egui::Event::Copy => self.copy_selection(ui),
                egui::Event::Cut => self.cut_selection(ui),
                egui::Event::Key {
                    key,
                    pressed,
                    modifiers,
                    ..
                } => {
                    if !pressed {
                        continue;
                    }

                    ui.input_mut(|input| {
                        input.consume_key(modifiers, key);
                    });

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
                            Key::V => continue,
                            Key::ArrowLeft => {
                                self.move_word(-1, modifiers.shift);
                                continue;
                            }
                            Key::ArrowRight => {
                                self.move_word(1, modifiers.shift);
                                continue;
                            }
                            Key::Home => {
                                self.move_to(0, modifiers.shift);
                                continue;
                            }
                            Key::End => {
                                self.move_to(self.text.len(), modifiers.shift);
                                continue;
                            }
                            Key::Backspace => {
                                self.delete_word_backward();
                                continue;
                            }
                            Key::Delete => {
                                self.delete_word_forward();
                                continue;
                            }
                            _ => {}
                        }
                    }

                    self.handle_key(key, modifiers.shift);
                }
                _ => {}
            }
        }
    }

    fn copy_selection(&self, ui: &mut Ui) {
        if let Some(text) = self.selected_text() {
            ui.output_mut(|output| {
                output.copied_text = text.to_string();
            });
        }
    }

    fn cut_selection(&mut self, ui: &mut Ui) {
        let Some(text) = self.selected_text() else {
            return;
        };

        ui.output_mut(|output| {
            output.copied_text = text.to_string();
        });

        self.delete_selection();
    }

    fn handle_key(&mut self, key: Key, shift: bool) {
        match key {
            Key::Enter => self.insert_newline(),
            Key::Tab => {
                if shift {
                    self.unindent_selection();
                } else {
                    self.indent_selection();
                }
            }
            Key::Backspace => self.backspace(),
            Key::Delete => self.delete_forward(),
            Key::ArrowLeft => self.move_horizontal(-1, shift),
            Key::ArrowRight => self.move_horizontal(1, shift),
            Key::ArrowUp => self.move_vertical(-1, shift),
            Key::ArrowDown => self.move_vertical(1, shift),
            Key::Home => self.move_to(self.line_start(self.cursor_line()), shift),
            Key::End => self.move_to(self.line_end(self.cursor_line()), shift),
            Key::PageUp => {
                for _ in 0..20 {
                    self.move_vertical(-1, shift);
                }
            }
            Key::PageDown => {
                for _ in 0..20 {
                    self.move_vertical(1, shift);
                }
            }
            _ => {}
        }
    }

    fn move_word(&mut self, direction: i32, shift: bool) {
        if !shift && self.selection.is_some() {
            let range = self.selection.take().unwrap();
            self.cursor = if direction < 0 { range.start } else { range.end };
            self.selection_anchor = self.cursor;
            return;
        }

        if direction < 0 {
            while self.cursor > 0 {
                let previous = self.previous_char_boundary(self.cursor);
                let character = self.text[previous..self.cursor].chars().next().unwrap_or(' ');
                if character.is_whitespace() {
                    self.cursor = previous;
                } else {
                    break;
                }
            }

            while self.cursor > 0 {
                let previous = self.previous_char_boundary(self.cursor);
                let character = self.text[previous..self.cursor].chars().next().unwrap_or(' ');
                if character.is_alphanumeric() || character == '_' {
                    self.cursor = previous;
                } else {
                    break;
                }
            }
        } else {
            while self.cursor < self.text.len() {
                let character = self.text[self.cursor..].chars().next().unwrap_or(' ');
                if character.is_whitespace() {
                    self.cursor = self.next_char_boundary(self.cursor);
                } else {
                    break;
                }
            }

            while self.cursor < self.text.len() {
                let character = self.text[self.cursor..].chars().next().unwrap_or(' ');
                if character.is_alphanumeric() || character == '_' {
                    self.cursor = self.next_char_boundary(self.cursor);
                } else {
                    break;
                }
            }
        }

        if shift {
            self.set_selection(self.selection_anchor, self.cursor);
        } else {
            self.selection_anchor = self.cursor;
            self.selection = None;
        }
    }

    fn delete_word_backward(&mut self) {
        if self.delete_selection() || self.cursor == 0 {
            return;
        }

        let end = self.cursor;
        self.move_word(-1, false);
        let start = self.cursor;
        if start < end {
            let line = self.line_from_index(start);
            self.text.replace_range(start..end, "");
            self.cursor = start;
            self.selection_anchor = start;
            self.text_changed(line);
        }
    }

    fn delete_word_forward(&mut self) {
        if self.delete_selection() || self.cursor >= self.text.len() {
            return;
        }

        let start = self.cursor;
        let mut end = start;
        while end < self.text.len() {
            let character = self.text[end..].chars().next().unwrap_or(' ');
            if character.is_whitespace() {
                end = self.next_char_boundary(end);
            } else {
                break;
            }
        }
        while end < self.text.len() {
            let character = self.text[end..].chars().next().unwrap_or(' ');
            if character.is_alphanumeric() || character == '_' {
                end = self.next_char_boundary(end);
            } else {
                break;
            }
        }

        if start < end {
            let line = self.line_from_index(start);
            self.text.replace_range(start..end, "");
            self.text_changed(line);
        }
    }

    fn indent_selection(&mut self) {
        let Some(range) = self.selection.clone() else {
            self.insert_text_raw(INDENT);
            return;
        };

        let start_line = self.line_from_index(range.start);
        let end_line = self.line_from_index(range.end);
        let mut inserted = 0usize;

        for line in (start_line..=end_line).rev() {
            let position = self.line_start(line);
            self.text.insert_str(position, INDENT);
            inserted += INDENT.len();

            if position <= self.cursor {
                self.cursor += INDENT.len();
            }

            if position <= self.selection_anchor {
                self.selection_anchor += INDENT.len();
            }
        }

        self.selection = Some(
            range.start..range.end + inserted,
        );
        self.text_changed(start_line);
    }

    fn unindent_selection(&mut self) {
        let Some(range) = self.selection.clone() else {
            self.remove_indent_at_cursor();
            return;
        };

        let start_line = self.line_from_index(range.start);
        let end_line = self.line_from_index(range.end);
        let mut removed_from_start = 0usize;
        let mut removed_from_end = 0usize;

        for line in (start_line..=end_line).rev() {
            let line_start = self.line_start(line);
            let line_end = self.line_end(line);
            let remove = if self.text[line_start..line_end].starts_with(INDENT) {
                INDENT.len()
            } else if self.text[line_start..line_end].starts_with('\t') {
                1
            } else {
                0
            };

            if remove == 0 {
                continue;
            }

            self.text.replace_range(
                line_start..line_start + remove,
                "",
            );

            if line_start < range.start {
                removed_from_start += remove;
            }

            if line_start < range.end {
                removed_from_end += remove;
            }

            if self.cursor > line_start {
                self.cursor = self.cursor.saturating_sub(
                    remove.min(self.cursor - line_start),
                );
            }

            if self.selection_anchor > line_start {
                self.selection_anchor = self.selection_anchor.saturating_sub(
                    remove.min(self.selection_anchor - line_start),
                );
            }
        }

        let new_start = range
            .start
            .saturating_sub(removed_from_start);
        let new_end = range
            .end
            .saturating_sub(removed_from_end);

        self.selection = if new_start < new_end {
            Some(new_start..new_end.min(self.text.len()))
        } else {
            None
        };

        self.text_changed(start_line);
    }

    fn remove_indent_at_cursor(&mut self) {
        let line = self.cursor_line();
        let line_start = self.line_start(line);
        let offset = self.cursor.saturating_sub(line_start);

        if offset >= INDENT.len()
            && self.text[line_start..self.cursor].ends_with(INDENT)
        {
            self.text.replace_range(
                self.cursor - INDENT.len()..self.cursor,
                "",
            );
            self.cursor -= INDENT.len();
            self.selection_anchor = self.cursor;
            self.text_changed(line);
        } else if offset > 0
            && self.text[line_start..self.cursor].ends_with('\t')
        {
            self.text.replace_range(
                self.cursor - 1..self.cursor,
                "",
            );
            self.cursor -= 1;
            self.selection_anchor = self.cursor;
            self.text_changed(line);
        }
    }

    fn insert_text_raw(&mut self, text: &str) {
        let line = self.cursor_line();
        self.delete_selection();
        self.insert_raw(text, line);
    }

    fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        if text == "\n" {
            self.insert_newline();
            return;
        }

        if text.chars().count() == 1 {
            match text.chars().next().unwrap() {
                '(' => return self.insert_pair('(', ')'),
                '[' => return self.insert_pair('[', ']'),
                '{' => return self.insert_pair('{', '}'),
                ')' => return self.insert_closing_bracket(')'),
                ']' => return self.insert_closing_bracket(']'),
                '}' => return self.insert_closing_bracket('}'),
                '"' => return self.insert_quote('"'),
                _ => {}
            }
        }

        let line = self.cursor_line();
        self.delete_selection();
        self.insert_raw(text, line);
    }

    fn insert_pasted_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let line = self.cursor_line();
        self.delete_selection();
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        self.insert_raw(&normalized, line);
    }

    fn insert_pair(&mut self, open: char, close: char) {
        let line = self.cursor_line();

        if let Some(range) = self.selection.take() {
            let selected = self.text[range.start..range.end].to_string();
            self.text.replace_range(range.start..range.end, "");
            self.cursor = range.start;
            self.text.insert(self.cursor, open);
            self.cursor += open.len_utf8();
            self.text.insert_str(self.cursor, &selected);
            self.cursor += selected.len();
            self.text.insert(self.cursor, close);
            self.selection_anchor = self.cursor;
            self.text_changed(line);
            return;
        }

        self.text.insert(self.cursor, open);
        self.cursor += open.len_utf8();
        self.text.insert(self.cursor, close);
        self.selection_anchor = self.cursor;
        self.text_changed(line);
    }

    fn insert_closing_bracket(&mut self, close: char) {
        if self.current_char() == Some(close) {
            self.cursor = self.next_char_boundary(self.cursor);
            self.selection_anchor = self.cursor;
            self.selection = None;
            return;
        }

        let line = self.cursor_line();
        self.delete_selection();
        self.insert_raw(&close.to_string(), line);
    }

    fn insert_quote(&mut self, quote: char) {
        if self.current_char() == Some(quote) {
            self.cursor = self.next_char_boundary(self.cursor);
            self.selection_anchor = self.cursor;
            self.selection = None;
            return;
        }

        if self.previous_char() == Some('\\') {
            let line = self.cursor_line();
            self.delete_selection();
            self.insert_raw(&quote.to_string(), line);
            return;
        }

        self.insert_pair(quote, quote);
    }

    fn insert_newline(&mut self) {
        let line = self.cursor_line();
        self.delete_selection();

        let line_start = self.line_start(line);
        let prefix = &self.text[line_start..self.cursor];
        let indentation: String = prefix
            .chars()
            .take_while(|value| *value == ' ' || *value == '\t')
            .collect();

        let previous = self.previous_char();
        let next = self.current_char();

        if previous == Some('{') && next == Some('}') {
            let insertion = format!("\n{}{}\n{}", indentation, INDENT, indentation);
            self.text.insert_str(self.cursor, &insertion);
            self.cursor += indentation.len() + 1 + INDENT.len();
            self.selection_anchor = self.cursor;
            self.text_changed(line);
            return;
        }

        let insertion = if previous == Some('{') {
            format!("\n{}{}", indentation, INDENT)
        } else {
            format!("\n{}", indentation)
        };

        self.text.insert_str(self.cursor, &insertion);
        self.cursor += insertion.len();
        self.selection_anchor = self.cursor;
        self.text_changed(line);
    }

    fn backspace(&mut self) {
        if self.delete_selection() || self.cursor == 0 {
            return;
        }

        let previous = self.previous_char();
        let current = self.current_char();

        if BRACKET_PAIRS.iter().any(|pair| {
            pair.open == previous.unwrap_or('\0')
                && pair.close == current.unwrap_or('\0')
        }) {
            let line = self.cursor_line();
            let next = self.next_char_boundary(self.cursor);
            let previous_cursor = self.previous_char_boundary(self.cursor);
            self.text.replace_range(self.cursor..next, "");
            self.text.replace_range(previous_cursor..self.cursor, "");
            self.cursor = previous_cursor;
            self.selection_anchor = self.cursor;
            self.text_changed(line);
            return;
        }

        let line = self.cursor_line();
        let previous_cursor = self.previous_char_boundary(self.cursor);
        self.text.replace_range(previous_cursor..self.cursor, "");
        self.cursor = previous_cursor;
        self.selection_anchor = self.cursor;
        self.text_changed(line);
    }

    fn delete_forward(&mut self) {
        if self.delete_selection() || self.cursor >= self.text.len() {
            return;
        }

        let line = self.cursor_line();
        let next = self.next_char_boundary(self.cursor);
        self.text.replace_range(self.cursor..next, "");
        self.text_changed(line);
    }

    fn move_horizontal(&mut self, direction: i32, shift: bool) {
        if !shift && self.selection.is_some() {
            let range = self.selection.take().unwrap();
            self.cursor = if direction < 0 { range.start } else { range.end };
            self.selection_anchor = self.cursor;
            return;
        }

        self.cursor = if direction < 0 {
            self.previous_char_boundary(self.cursor)
        } else {
            self.next_char_boundary(self.cursor)
        };

        if shift {
            self.set_selection(self.selection_anchor, self.cursor);
        } else {
            self.selection_anchor = self.cursor;
            self.selection = None;
        }
    }

    fn move_vertical(&mut self, direction: i32, shift: bool) {
        let current_line = self.cursor_line();
        let target_line = if direction < 0 {
            current_line.checked_sub(1)
        } else if current_line + 1 < self.line_count() {
            Some(current_line + 1)
        } else {
            None
        };

        let Some(target_line) = target_line else {
            return;
        };

        let column = self.cursor_position();
        let target_start = self.line_start(target_line);
        let target_end = self.line_end(target_line);
        let target_column = column.min(self.line_char_counts[target_line] as usize);

        self.cursor = self.byte_index_from_column(
            target_start,
            target_end,
            target_column,
        );

        if shift {
            self.set_selection(self.selection_anchor, self.cursor);
        } else {
            self.selection_anchor = self.cursor;
            self.selection = None;
        }
    }

    fn move_to(&mut self, target: usize, shift: bool) {
        self.cursor = target;

        if shift {
            self.set_selection(self.selection_anchor, self.cursor);
        } else {
            self.selection_anchor = self.cursor;
            self.selection = None;
        }
    }
}
