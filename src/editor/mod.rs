mod geometry;
mod highlighting;
mod input;
mod render;
mod state;

use std::collections::VecDeque;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{Id, Response, Ui};
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

use self::highlighting::Highlighter;
use self::state::{EditorSnapshot, EditorStats};

pub struct CodeEditor {
    pub(super) uid: u64,
    pub(super) text: String,
    pub(super) cursor: usize,
    pub(super) selection: Option<Range<usize>>,
    pub(super) selection_anchor: usize,
    pub(super) line_starts: Vec<usize>,
    pub(super) line_char_counts: Vec<u32>,
    pub(super) max_line_chars: usize,
    pub(super) char_count: usize,
    pub(super) highlighter: Highlighter,
    pub(super) path: Option<PathBuf>,
    pub(super) untitled_name: String,
    pub(super) dirty: bool,
    pub(super) focus_requested: bool,
    pub(super) undo_stack: VecDeque<EditorSnapshot>,
    pub(super) redo_stack: VecDeque<EditorSnapshot>,
    pub(super) undo_bytes: usize,
    pub(super) redo_bytes: usize,
}

impl CodeEditor {
    pub fn new(
        theme: &Arc<Theme>,
        language: &str,
        initial_text: &str,
    ) -> Self {
        Self::new_with_name(theme, language, initial_text, "Untitled".to_string())
    }

    pub fn new_with_name(
        theme: &Arc<Theme>,
        language: &str,
        initial_text: &str,
        untitled_name: String,
    ) -> Self {
        let mut editor = Self {
            uid: 0,
            text: initial_text.to_string(),
            cursor: 0,
            selection: None,
            selection_anchor: 0,
            line_starts: Vec::new(),
            line_char_counts: Vec::new(),
            max_line_chars: 1,
            char_count: 0,
            highlighter: Highlighter::new(theme, language),
            path: None,
            untitled_name,
            dirty: false,
            focus_requested: true,
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            undo_bytes: 0,
            redo_bytes: 0,
        };

        editor.rebuild_line_index();
        editor.highlighter.sync_line_count(editor.line_count());
        editor.push_snapshot();
        editor
    }

    pub fn with_uid(mut self, uid: u64) -> Self {
        self.uid = uid;
        self
    }

    pub fn uid(&self) -> u64 {
        self.uid
    }

    pub fn from_file(
        theme: &Arc<Theme>,
        path: PathBuf,
        language: &str,
        text: String,
    ) -> Self {
        let mut editor = Self::new(theme, language, &text);
        editor.path = Some(path);
        editor.dirty = false;
        editor.focus_requested = true;
        editor
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        id: Id,
        syntax_set: &SyntaxSet,
    ) -> Response {
        render::render(self, ui, id, syntax_set)
    }

    pub fn stats(&self) -> EditorStats {
        EditorStats {
            lines: self.line_count(),
            chars: self.char_count,
            selected: self.selected_char_count(),
            cursor_line: self.cursor_line() + 1,
            cursor_column: self.cursor_position() + 1,
        }
    }

    pub fn file_name(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.untitled_name.clone())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn mark_saved(&mut self, path: PathBuf) {
        self.path = Some(path);
        self.dirty = false;
    }

    pub fn rename_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    pub fn request_focus(&mut self) {
        self.focus_requested = true;
    }

    pub fn set_language(
        &mut self,
        theme: &Arc<Theme>,
        language: &str,
    ) {
        self.highlighter = Highlighter::new(theme, language);
        self.highlighter.sync_line_count(self.line_count());
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_search_selection(&mut self, start: usize, end: usize) {
        let start = start.min(self.text.len());
        let end = end.min(self.text.len());
        if start >= end {
            self.selection = None;
            self.cursor = start;
            self.selection_anchor = start;
            return;
        }
        self.selection = Some(start..end);
        self.selection_anchor = start;
        self.cursor = end;
        self.request_focus();
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.text.len());
        self.selection = None;
        self.selection_anchor = self.cursor;
    }

    pub fn go_to_line(&mut self, line: usize) {
        let line_index = line.saturating_sub(1).min(self.line_count().saturating_sub(1));
        self.cursor = self.line_start(line_index);
        self.selection = None;
        self.selection_anchor = self.cursor;
        self.request_focus();
    }

    pub fn mark_text_changed(&mut self, edit_line: usize) {
        self.text_changed(edit_line);
    }

    fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            text: self.text.clone(),
            cursor: self.cursor,
            selection: self.selection.clone(),
            selection_anchor: self.selection_anchor,
        }
    }

    fn push_snapshot(&mut self) {
        const MAX_UNDO_ENTRIES: usize = 200;
        const MAX_HISTORY_BYTES: usize = 16 * 1024 * 1024;

        let snapshot = self.snapshot();
        if self.undo_stack.back().map(|value| value.text.as_str()) == Some(snapshot.text.as_str()) {
            return;
        }

        self.undo_bytes += snapshot.text.len();
        self.undo_stack.push_back(snapshot);

        while self.undo_stack.len() > MAX_UNDO_ENTRIES || self.undo_bytes > MAX_HISTORY_BYTES {
            if self.undo_stack.len() <= 1 {
                break;
            }
            if let Some(oldest) = self.undo_stack.pop_front() {
                self.undo_bytes = self.undo_bytes.saturating_sub(oldest.text.len());
            }
        }
    }

    pub fn undo(&mut self) {
        if self.undo_stack.len() <= 1 {
            return;
        }

        let current = self.undo_stack.pop_back().unwrap();
        self.undo_bytes = self.undo_bytes.saturating_sub(current.text.len());
        self.redo_bytes += current.text.len();
        self.redo_stack.push_back(current);

        if let Some(snapshot) = self.undo_stack.back().cloned() {
            self.restore_snapshot(snapshot);
        }
    }

    pub fn redo(&mut self) {
        let Some(snapshot) = self.redo_stack.pop_back() else {
            return;
        };

        self.redo_bytes = self.redo_bytes.saturating_sub(snapshot.text.len());
        self.undo_bytes += snapshot.text.len();
        self.undo_stack.push_back(snapshot.clone());
        self.restore_snapshot(snapshot);
    }

    fn restore_snapshot(&mut self, snapshot: EditorSnapshot) {
        self.text = snapshot.text;
        self.cursor = snapshot.cursor.min(self.text.len());
        self.selection = snapshot.selection.filter(|range| range.end <= self.text.len());
        self.selection_anchor = snapshot.selection_anchor.min(self.text.len());
        self.rebuild_line_index();
        self.highlighter.sync_line_count(self.line_count());
        self.highlighter.invalidate_from(0, self.line_count());
        self.dirty = true;
    }

    pub fn select_word(&mut self) {
        self.select_word_at(self.cursor);
    }

    pub fn select_current_line(&mut self) {
        self.select_line_at(self.cursor);
    }

    pub fn delete_current_line(&mut self) {
        let line = self.cursor_line();
        let start = self.line_start(line);
        let end = if line + 1 < self.line_starts.len() { self.line_starts[line + 1] } else { self.text.len() };
        self.selection = Some(start..end);
        self.cursor = end;
        self.delete_selection();
    }

    pub fn move_current_line(&mut self, direction: i32) {
        let line = self.cursor_line();
        let target = if direction < 0 { line.checked_sub(1) } else if line + 1 < self.line_count() { Some(line + 1) } else { None };
        let Some(target) = target else { return; };

        let current_start = self.line_start(line);
        let current_end = if line + 1 < self.line_starts.len() { self.line_starts[line + 1] } else { self.text.len() };
        let target_start = self.line_start(target);
        let target_end = if target + 1 < self.line_starts.len() { self.line_starts[target + 1] } else { self.text.len() };

        let current = self.text[current_start..current_end].to_string();
        let target_text = self.text[target_start..target_end].to_string();
        let old_cursor = self.cursor;
        let column = old_cursor.saturating_sub(current_start);

        let current_len = current.len();
        let target_len = target_text.len();

        if direction < 0 {
            let replacement = format!("{}{}", current, target_text);
            self.text.replace_range(target_start..current_end, &replacement);
            self.cursor = target_start + column.min(current_len);
        } else {
            let replacement = format!("{}{}", target_text, current);
            self.text.replace_range(current_start..target_end, &replacement);
            let new_start = current_start + target_len;
            self.cursor = new_start + column.min(current_len);
        }

        self.selection = None;
        self.selection_anchor = self.cursor;
        self.text_changed(line.min(target));
    }

    pub fn toggle_line_comment(&mut self) {
        let line = self.cursor_line();
        let start = self.line_start(line);
        let end = self.line_end(line);
        let content = &self.text[start..end];

        let comment = match self.highlighter.language_name() {
            "Rust" | "C" | "C++" | "JavaScript" | "TypeScript" => "// ",
            "Python" | "Shell Script" => "# ",
            _ => "// ",
        };

        let leading = content.len() - content.trim_start_matches([' ', '\t']).len();
        let insert_at = start + leading;

        if content[leading..].starts_with(comment) {
            self.text.replace_range(insert_at..insert_at + comment.len(), "");
            self.cursor = self.cursor.saturating_sub(comment.len());
            self.selection_anchor = self.selection_anchor.saturating_sub(comment.len());
        } else {
            self.text.insert_str(insert_at, comment);
            if insert_at <= self.cursor { self.cursor += comment.len(); }
            if insert_at <= self.selection_anchor { self.selection_anchor += comment.len(); }
        }

        self.text_changed(line);
    }

    fn selected_char_count(&self) -> usize {
        match &self.selection {
            Some(range) if range.start < range.end => {
                self.text[range.start..range.end].chars().count()
            }
            _ => 0,
        }
    }

    fn rebuild_line_index(&mut self) {
        geometry::rebuild_line_index(self);
    }

    fn text_changed(&mut self, edit_line: usize) {
        self.rebuild_line_index();
        self.highlighter
            .invalidate_from(edit_line, self.line_count());
        self.dirty = true;
        self.redo_stack.clear();
        self.redo_bytes = 0;
        self.push_snapshot();
    }

    fn line_count(&self) -> usize {
        self.line_starts.len().max(1)
    }

    fn line_from_index(&self, index: usize) -> usize {
        geometry::line_from_index(self, index)
    }

    fn line_start(&self, line: usize) -> usize {
        *self.line_starts.get(line).unwrap_or(&self.text.len())
    }

    fn line_end(&self, line: usize) -> usize {
        geometry::line_end(self, line)
    }

    fn cursor_line(&self) -> usize {
        self.line_from_index(self.cursor)
    }

    fn cursor_position(&self) -> usize {
        let line = self.cursor_line();
        self.column_from_index(self.line_start(line), self.cursor)
    }

    fn column_from_index(&self, line_start: usize, index: usize) -> usize {
        self.text[line_start..index.min(self.text.len())]
            .chars()
            .count()
    }

    fn byte_index_from_column(
        &self,
        line_start: usize,
        line_end: usize,
        column: usize,
    ) -> usize {
        geometry::byte_index_from_column(
            self,
            line_start,
            line_end,
            column,
        )
    }

    fn set_selection(&mut self, anchor: usize, cursor: usize) {
        if anchor == cursor {
            self.selection = None;
            return;
        }

        self.selection = if anchor < cursor {
            Some(anchor..cursor)
        } else {
            Some(cursor..anchor)
        };
    }

    pub fn select_all(&mut self) {
        if self.text.is_empty() {
            self.selection = None;
            return;
        }

        self.selection = Some(0..self.text.len());
        self.selection_anchor = 0;
        self.cursor = self.text.len();
    }

    fn selected_text(&self) -> Option<&str> {
        let range = self.selection.as_ref()?;
        if range.start >= range.end {
            return None;
        }
        self.text.get(range.start..range.end)
    }

    fn delete_selection(&mut self) -> bool {
        let Some(range) = self.selection.take() else {
            return false;
        };

        if range.start >= range.end {
            return false;
        }

        let line = self.line_from_index(range.start);
        self.text.replace_range(range.start..range.end, "");
        self.cursor = range.start;
        self.selection_anchor = self.cursor;
        self.text_changed(line);
        true
    }

    fn current_char(&self) -> Option<char> {
        self.text[self.cursor..].chars().next()
    }

    fn previous_char(&self) -> Option<char> {
        if self.cursor == 0 {
            None
        } else {
            self.text[..self.cursor].chars().next_back()
        }
    }

    fn previous_char_boundary(&self, index: usize) -> usize {
        let mut position = index.saturating_sub(1);
        while position > 0 && !self.text.is_char_boundary(position) {
            position -= 1;
        }
        position
    }

    fn next_char_boundary(&self, index: usize) -> usize {
        if index >= self.text.len() {
            return self.text.len();
        }

        let mut position = index + 1;
        while position < self.text.len() && !self.text.is_char_boundary(position) {
            position += 1;
        }
        position
    }

    fn select_word_at(&mut self, cursor: usize) {
        if self.text.is_empty() {
            self.selection = None;
            return;
        }

        let cursor = cursor.min(self.text.len());
        let mut start = cursor;
        let mut end = cursor;

        let character_at = |index: usize| self.text[index..].chars().next();
        let is_word = |character: char| character.is_alphanumeric() || character == '_';

        if let Some(character) = character_at(cursor) {
            if is_word(character) {
                while start > 0 {
                    let previous = self.previous_char_boundary(start);
                    match self.text[previous..start].chars().next() {
                        Some(character) if is_word(character) => start = previous,
                        _ => break,
                    }
                }

                while end < self.text.len() {
                    match self.text[end..].chars().next() {
                        Some(character) if is_word(character) => {
                            end = self.next_char_boundary(end);
                        }
                        _ => break,
                    }
                }
            } else {
                start = self.previous_char_boundary(cursor.saturating_add(1));
                end = self.next_char_boundary(cursor);
            }
        } else {
            start = self.previous_char_boundary(cursor);
            end = cursor;
        }

        if start < end {
            self.selection = Some(start..end);
            self.selection_anchor = start;
        } else {
            self.selection = None;
            self.selection_anchor = cursor;
        }
    }

    fn select_line_at(&mut self, cursor: usize) {
        let line = self.line_from_index(cursor);
        let start = self.line_start(line);
        let end = self.line_end(line);

        if start < end {
            self.selection = Some(start..end);
            self.selection_anchor = start;
            self.cursor = end;
        } else {
            self.selection = None;
            self.selection_anchor = start;
            self.cursor = start;
        }
    }

    fn insert_raw(&mut self, text: &str, edit_line: usize) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.selection_anchor = self.cursor;
        self.selection = None;
        self.text_changed(edit_line);
    }
}
