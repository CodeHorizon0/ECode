mod geometry;
mod highlighting;
mod input;
mod render;
mod state;

use std::ops::Range;
use std::path::{Path, PathBuf};

use eframe::egui::{Id, Response, Ui};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use self::highlighting::Highlighter;
use self::state::EditorStats;

pub struct CodeEditor {
    pub(super) uid: u64,
    pub(super) text: String,
    pub(super) cursor: usize,
    pub(super) selection: Option<Range<usize>>,
    pub(super) selection_anchor: usize,
    pub(super) line_starts: Vec<usize>,
    pub(super) line_char_counts: Vec<usize>,
    pub(super) max_line_chars: usize,
    pub(super) char_count: usize,
    pub(super) highlighter: Highlighter,
    pub(super) path: Option<PathBuf>,
    pub(super) untitled_name: String,
    pub(super) dirty: bool,
    pub(super) focus_requested: bool,
}

impl CodeEditor {
    pub fn new(
        syntax_set: &SyntaxSet,
        theme_set: &ThemeSet,
        language: &str,
        initial_text: &str,
    ) -> Self {
        Self::new_with_name(
            syntax_set,
            theme_set,
            language,
            initial_text,
            "Untitled".to_string(),
        )
    }

    pub fn new_with_name(
        syntax_set: &SyntaxSet,
        theme_set: &ThemeSet,
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
            highlighter: Highlighter::new(
                syntax_set,
                theme_set,
                language,
            ),
            path: None,
            untitled_name,
            dirty: false,
            focus_requested: true,
        };

        editor.rebuild_line_index();
        editor.highlighter.sync_line_count(editor.line_count());
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
        syntax_set: &SyntaxSet,
        theme_set: &ThemeSet,
        path: PathBuf,
        language: &str,
        text: String,
    ) -> Self {
        let mut editor = Self::new(
            syntax_set,
            theme_set,
            language,
            &text,
        );
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
        syntax_set: &SyntaxSet,
        theme_set: &ThemeSet,
        language: &str,
    ) {
        self.highlighter = Highlighter::new(
            syntax_set,
            theme_set,
            language,
        );
        self.highlighter.sync_line_count(self.line_count());
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
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

    fn select_all(&mut self) {
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
