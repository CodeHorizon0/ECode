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
            Key::Tab => self.insert_text(INDENT),
            Key::Backspace => self.backspace(),
            Key::Delete => self.delete_forward(),
            Key::ArrowLeft => self.move_horizontal(-1, shift),
            Key::ArrowRight => self.move_horizontal(1, shift),
            Key::ArrowUp => self.move_vertical(-1, shift),
            Key::ArrowDown => self.move_vertical(1, shift),
            Key::Home => {
                self.move_to(self.line_start(self.cursor_line()), shift)
            }
            Key::End => self.move_to(self.line_end(self.cursor_line()), shift),
            _ => {}
        }
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
        if self.delete_selection() {
            return;
        }

        if self.cursor == 0 {
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
        if self.delete_selection() {
            return;
        }

        if self.cursor >= self.text.len() {
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
        let target_column = column.min(self.line_char_counts[target_line]);

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
