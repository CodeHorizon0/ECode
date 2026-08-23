// app.rs
use std::path::{Path, PathBuf};
use std::time::Instant;

use eframe::egui::{self, Context, RichText};
use eframe::CreationContext;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::bindings::{default_bindings, Action, Binding};
use crate::config::{BACKGROUND, LANGUAGES};
use crate::settings::{EditorSettings, EditorTheme};
use crate::editor::CodeEditor;
use crate::fs;
use crate::workspace::{NodeKind, Workspace};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExplorerDialogKind {
    NewFile,
    NewFolder,
    Rename,
    Delete,
}

pub struct CodeApp {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    editors: Vec<CodeEditor>,
    current_tab: usize,
    explorer_visible: bool,
    workspace: Workspace,
    new_tab_language: String,
    startup_time: Instant,
    startup_logged: bool,
    last_error: Option<String>,
    close_confirmation: Option<u64>,
    next_editor_uid: u64,
    explorer_dialog: Option<ExplorerDialogKind>,
    explorer_dialog_name: String,
    bindings: Vec<Binding>,
    find_open: bool,
    replace_open: bool,
    search_query: String,
    replace_query: String,
    go_to_line_open: bool,
    go_to_line_input: String,
    settings_open: bool,
    settings: EditorSettings,
}

impl CodeApp {
    pub fn new(_cc: &CreationContext<'_>, startup_time: Instant) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();

        let editor = CodeEditor::new_with_name(
            &syntax_set,
            &theme_set,
            "Rust",
            r#"fn main() {
    println!("Hello, world!");
}"#,
            "Untitled 1".to_string(),
        ).with_uid(1);

        Self {
            syntax_set,
            theme_set,
            editors: vec![editor],
            current_tab: 0,
            explorer_visible: true,
            workspace: Workspace::new(),
            new_tab_language: "Rust".to_string(),
            startup_time,
            startup_logged: false,
            last_error: None,
            close_confirmation: None,
            next_editor_uid: 2,
            explorer_dialog: None,
            explorer_dialog_name: String::new(),
            bindings: default_bindings(),
            find_open: false,
            replace_open: false,
            search_query: String::new(),
            replace_query: String::new(),
            go_to_line_open: false,
            go_to_line_input: String::new(),
            settings_open: false,
            settings: EditorSettings::default(),
        }
    }

    fn new_tab(&mut self) {
        let uid = self.next_editor_uid;
        self.next_editor_uid = self.next_editor_uid.wrapping_add(1);

        let editor = CodeEditor::new_with_name(
            &self.syntax_set,
            &self.theme_set,
            &self.new_tab_language,
            "",
            format!("Untitled {}", self.untitled_index()),
        ).with_uid(uid);

        self.editors.push(editor);
        self.current_tab = self.editors.len() - 1;
        self.last_error = None;
        self.request_editor_focus();
    }

    fn untitled_index(&self) -> usize {
        let mut index = self.editors.len() + 1;

        loop {
            let name = format!("Untitled {}", index);
            if !self.editors.iter().any(|editor| editor.file_name() == name) {
                return index;
            }
            index += 1;
        }
    }

    fn open_project(&mut self) {
        match self.workspace.open_dialog() {
            Ok(Some(path)) => {
                println!("[workspace] opened {}", path.display());
                self.last_error = None;
            }
            Ok(None) => {}
            Err(error) => self.set_error(format!(
                "Failed to open project: {}",
                error
            )),
        }
    }

    fn open_file_dialog(&mut self) {
        match fs::open_file_dialog() {
            Ok(Some(file)) => self.open_file_data(file),
            Ok(None) => {}
            Err(error) => self.set_error(format!(
                "Failed to open file: {}",
                error
            )),
        }
    }

    fn open_file_path(&mut self, path: &Path) {
        match fs::load_file(path) {
            Ok(file) => self.open_file_data(file),
            Err(error) => self.set_error(format!(
                "Failed to open {}: {}",
                path.display(),
                error
            )),
        }
    }

    fn open_file_data(&mut self, file: fs::LoadedFile) {
        if let Some(index) = self.find_open_path(&file.path) {
            self.current_tab = index;
            self.request_editor_focus();
            return;
        }

        let uid = self.next_editor_uid;
        self.next_editor_uid = self.next_editor_uid.wrapping_add(1);

        let editor = CodeEditor::from_file(
            &self.syntax_set,
            &self.theme_set,
            file.path,
            &file.language,
            file.text,
        ).with_uid(uid);

        self.editors.push(editor);
        self.current_tab = self.editors.len() - 1;
        self.last_error = None;
        self.request_editor_focus();
    }

    fn save_current(&mut self) {
        let current_tab = self.current_tab;
        let mut error_message = None;
        let mut saved = false;

        {
            let Some(editor) = self.editors.get_mut(current_tab) else {
                return;
            };

            if let Some(path) = editor.path().map(Path::to_path_buf) {
                match fs::save_file(&path, &editor.text) {
                    Ok(()) => {
                        editor.mark_saved(path);
                        saved = true;
                    }
                    Err(error) => {
                        error_message = Some(format!(
                            "Failed to save {}: {}",
                            path.display(),
                            error
                        ));
                    }
                }
            } else {
                let name = editor.file_name();
                let text = editor.text.clone();

                match fs::save_file_dialog(&name, &text) {
                    Ok(Some(path)) => {
                        let language = path
                            .extension()
                            .and_then(|value| value.to_str())
                            .map(crate::config::language_for_extension)
                            .unwrap_or("Plain Text")
                            .to_string();

                        editor.mark_saved(path);
                        editor.set_language(
                            &self.syntax_set,
                            &self.theme_set,
                            &language,
                        );
                        saved = true;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        error_message = Some(format!(
                            "Failed to save file: {}",
                            error
                        ));
                    }
                }
            }
        }

        if let Some(error) = error_message {
            self.set_error(error);
        } else if saved {
            self.last_error = None;
        }

        self.request_editor_focus();
    }

    fn save_current_as(&mut self) {
        let current_tab = self.current_tab;
        let mut error_message = None;
        let mut saved = false;

        {
            let Some(editor) = self.editors.get_mut(current_tab) else {
                return;
            };

            let name = editor.file_name();
            let text = editor.text.clone();

            match fs::save_file_dialog(&name, &text) {
                Ok(Some(path)) => {
                    let language = path
                        .extension()
                        .and_then(|value| value.to_str())
                        .map(crate::config::language_for_extension)
                        .unwrap_or("Plain Text")
                        .to_string();

                    editor.mark_saved(path);
                    editor.set_language(
                        &self.syntax_set,
                        &self.theme_set,
                        &language,
                    );
                    saved = true;
                }
                Ok(None) => {}
                Err(error) => {
                    error_message = Some(format!(
                        "Failed to save file: {}",
                        error
                    ));
                }
            }
        }

        if let Some(error) = error_message {
            self.set_error(error);
        } else if saved {
            self.last_error = None;
        }

        self.request_editor_focus();
    }

    fn close_current_tab(&mut self) {
        if self.editors.len() <= 1 {
            return;
        }

        if self.editors[self.current_tab].is_dirty() {
            self.close_confirmation = self.editors.get(self.current_tab).map(CodeEditor::uid);
            return;
        }

        self.remove_tab(self.current_tab);
    }

    fn remove_tab(&mut self, index: usize) {
        if index >= self.editors.len() {
            return;
        }

        self.editors.remove(index);

        if self.editors.is_empty() {
            self.new_tab();
            return;
        }

        if self.current_tab >= self.editors.len() {
            self.current_tab = self.editors.len() - 1;
        } else if index < self.current_tab {
            self.current_tab = self.current_tab.saturating_sub(1);
        }

        self.request_editor_focus();
    }

    fn close_tab(&mut self, index: usize) {
        if index >= self.editors.len() {
            return;
        }

        if self.editors[index].is_dirty() {
            self.close_confirmation = self.editors.get(index).map(CodeEditor::uid);
            return;
        }

        self.remove_tab(index);
    }

    fn find_open_path(&self, path: &Path) -> Option<usize> {
        let normalized = normalize_path(path);

        self.editors.iter().position(|editor| {
            editor
                .path()
                .map(normalize_path)
                .as_deref()
                == Some(normalized.as_path())
        })
    }

    fn request_editor_focus(&mut self) {
        if let Some(editor) = self.editors.get_mut(self.current_tab) {
            editor.request_focus();
        }
    }

    fn set_error(&mut self, message: String) {
        eprintln!("[error] {message}");
        self.last_error = Some(message);
    }

    fn handle_global_shortcuts(&mut self, ctx: &Context) {
        let mut actions = Vec::new();

        for binding in &self.bindings {
            let consumed = ctx.input_mut(|input| {
                input.consume_shortcut(&binding.shortcut)
            });

            if consumed {
                actions.push(binding.action);
            }
        }

        for action in actions {
            self.execute_action(action);
        }
    }

    fn execute_action(&mut self, action: Action) {
        match action {
            Action::Save => self.save_current(),
            Action::SaveAs => self.save_current_as(),
            Action::SaveAll => self.save_all(),
            Action::OpenFile => self.open_file_dialog(),
            Action::OpenFolder => self.open_project(),
            Action::NewTab => self.new_tab(),
            Action::CloseTab => self.close_current_tab(),
            Action::CloseAllTabs => self.close_all_tabs(),
            Action::NextTab => self.select_relative_tab(1),
            Action::PreviousTab => self.select_relative_tab(-1),
            Action::Undo => self.with_current_editor(|editor| editor.undo()),
            Action::Redo => self.with_current_editor(|editor| editor.redo()),
            Action::Find => self.open_find(),
            Action::FindNext => self.find_next(),
            Action::FindPrevious => self.find_previous(),
            Action::Replace => self.open_replace(),
            Action::GoToLine => self.open_go_to_line(),
            Action::SelectAll => self.with_current_editor(|editor| editor.select_all()),
            Action::SelectWord => self.with_current_editor(|editor| editor.select_word()),
            Action::SelectLine => self.with_current_editor(|editor| editor.select_current_line()),
            Action::DeleteLine => self.with_current_editor(|editor| editor.delete_current_line()),
            Action::MoveLineUp => self.with_current_editor(|editor| editor.move_current_line(-1)),
            Action::MoveLineDown => self.with_current_editor(|editor| editor.move_current_line(1)),
            Action::ToggleLineComment => self.with_current_editor(|editor| editor.toggle_line_comment()),
            Action::RenameSelected => self.open_explorer_dialog(ExplorerDialogKind::Rename),
        }
    }

    fn with_current_editor<F>(&mut self, operation: F)
    where
        F: FnOnce(&mut CodeEditor),
    {
        let current_tab = self.current_tab;
        if let Some(editor) = self.editors.get_mut(current_tab) {
            operation(editor);
        }
        self.request_editor_focus();
    }

    fn save_all(&mut self) {
        let count = self.editors.len();
        for index in 0..count {
            if index == self.current_tab {
                self.save_current();
                continue;
            }

            let previous = self.current_tab;
            self.current_tab = index;
            self.save_current();
            self.current_tab = previous.min(self.editors.len().saturating_sub(1));
        }
        self.request_editor_focus();
    }

    fn close_all_tabs(&mut self) {
        let mut index = self.editors.len();
        while index > 1 {
            index -= 1;
            if self.editors[index].is_dirty() {
                self.current_tab = index;
                self.close_confirmation = Some(self.editors[index].uid());
                return;
            }
            self.editors.remove(index);
        }

        self.current_tab = 0;
        self.request_editor_focus();
    }

    fn select_relative_tab(&mut self, direction: isize) {
        if self.editors.is_empty() {
            return;
        }

        let count = self.editors.len() as isize;
        let current = self.current_tab as isize;
        self.current_tab = ((current + direction).rem_euclid(count)) as usize;
        self.request_editor_focus();
    }

    fn open_find(&mut self) {
        self.find_open = true;
        self.replace_open = false;
        self.request_editor_focus();
    }

    fn open_replace(&mut self) {
        self.find_open = true;
        self.replace_open = true;
        self.request_editor_focus();
    }

    fn open_go_to_line(&mut self) {
        self.go_to_line_open = true;
        self.go_to_line_input.clear();
    }

    fn find_next(&mut self) {
        if self.search_query.is_empty() {
            self.open_find();
            return;
        }

        let current_tab = self.current_tab;
        let query = self.search_query.clone();
        let mut found = false;
        {
            let Some(editor) = self.editors.get_mut(current_tab) else { return; };
            let start = editor.cursor;
            if let Some(offset) = editor.text[start..].find(&query) {
                let position = start + offset;
                editor.set_search_selection(position, position + query.len());
                found = true;
            } else if let Some(offset) = editor.text.find(&query) {
                editor.set_search_selection(offset, offset + query.len());
                found = true;
            }
        }
        if found {
            self.request_editor_focus();
        }
    }

    fn find_previous(&mut self) {
        if self.search_query.is_empty() {
            self.open_find();
            return;
        }

        let current_tab = self.current_tab;
        let query = self.search_query.clone();
        let mut found = false;
        {
            let Some(editor) = self.editors.get_mut(current_tab) else { return; };
            let before = editor.cursor.min(editor.text.len());
            if let Some(position) = editor.text[..before].rfind(&query) {
                editor.set_search_selection(position, position + query.len());
                found = true;
            }
        }
        if found {
            self.request_editor_focus();
        }
    }

    fn render_menu(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("menu")
            .exact_height(38.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    if ui.button("New").clicked() {
                        self.new_tab();
                    }
                    if ui.button("Open File").clicked() {
                        self.open_file_dialog();
                    }
                    if ui.button("Open Folder").clicked() {
                        self.open_project();
                    }
                    if ui.button("Save").clicked() {
                        self.save_current();
                    }
                    if ui.button("Save As").clicked() {
                        self.save_current_as();
                    }
                    if ui.button("Close").clicked() {
                        self.close_current_tab();
                    }

                    ui.separator();
                    ui.label("New file:");

                    egui::ComboBox::from_id_source("lang_combo")
                        .width(115.0)
                        .selected_text(&self.new_tab_language)
                        .show_ui(ui, |ui| {
                            for language in LANGUAGES {
                                ui.selectable_value(
                                    &mut self.new_tab_language,
                                    language.to_string(),
                                    *language,
                                );
                            }
                        });

                    ui.separator();

                    if ui.button("Settings").clicked() {
                        self.settings_open = true;
                    }

                    ui.separator();

                    let explorer_text = if self.explorer_visible {
                        "Hide Explorer"
                    } else {
                        "Show Explorer"
                    };

                    if ui.button(explorer_text).clicked() {
                        self.explorer_visible = !self.explorer_visible;
                        self.request_editor_focus();
                    }
                });
            });
    }

    fn render_tabs(&mut self, ctx: &Context) {
        let mut selected_tab = None;
        let mut close_tab = None;

        egui::TopBottomPanel::top("tabs")
            .exact_height(34.0)
            .show(ctx, |ui| {
                egui::ScrollArea::horizontal()
                    .id_source("tab_scroll")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;

                            for index in 0..self.editors.len() {
                                let selected = self.current_tab == index;
                                let name = self.editors[index].file_name();
                                let dirty = self.editors[index].is_dirty();

                                let frame = egui::Frame::none()
                                    .fill(if selected {
                                        self.settings.tab_active
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    })
                                    .inner_margin(egui::Margin::symmetric(8.0, 0.0));

                                frame.show(ui, |ui| {
                                    ui.set_height(32.0);

                                    ui.horizontal_centered(|ui| {
                                        let label = if dirty {
                                            format!("{} •", name)
                                        } else {
                                            name
                                        };

                                        let response = ui.add(
                                            egui::SelectableLabel::new(selected, label),
                                        );

                                        if response.hovered() && !selected {
                                            ui.painter().rect_filled(
                                                response.rect,
                                                0.0,
                                                self.settings.tab_hover,
                                            );
                                        }

                                        if response.clicked() {
                                            selected_tab = Some(index);
                                        }

                                        if ui.small_button("×").clicked() {
                                            close_tab = Some(index);
                                        }
                                    });
                                });
                            }
                        });
                    });
            });

        if let Some(index) = selected_tab {
            if index < self.editors.len() {
                self.current_tab = index;
                self.request_editor_focus();
            }
        }

        if let Some(index) = close_tab {
            self.close_tab(index);
        }
    }

    fn render_explorer(&mut self, ctx: &Context) {
        if !self.explorer_visible {
            return;
        }

        let mut open_file = None;
        let mut toggle_dir = None;
        let mut select_path = None;

        egui::SidePanel::left("explorer")
            .resizable(true)
            .default_width(250.0)
            .min_width(170.0)
            .max_width(420.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.strong("EXPLORER");

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui.small_button("Open").clicked() {
                                self.open_project();
                            }

                            if ui.small_button("↻").clicked() {
                                match self.workspace.refresh() {
                                    Ok(()) => self.last_error = None,
                                    Err(error) => self.set_error(format!(
                                        "Failed to refresh explorer: {}",
                                        error
                                    )),
                                }
                            }
                        },
                    );
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                if self.workspace.is_open() {
                    let root = self.workspace.root().map(Path::to_path_buf);

                    let Some(root) = root else {
                        return;
                    };
                    ui.label(
                        RichText::new(root.display().to_string())
                            .small()
                            .weak(),
                    );
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        if ui.small_button("+ File").clicked() {
                            self.open_explorer_dialog(ExplorerDialogKind::NewFile);
                        }
                        if ui.small_button("+ Folder").clicked() {
                            self.open_explorer_dialog(ExplorerDialogKind::NewFolder);
                        }
                        if self.workspace.selected().is_some()
                            && ui.small_button("Rename").clicked()
                        {
                            self.open_explorer_dialog(ExplorerDialogKind::Rename);
                        }
                        if self.workspace.selected().is_some()
                            && ui.small_button("Delete").clicked()
                        {
                            self.open_explorer_dialog(ExplorerDialogKind::Delete);
                        }
                    });

                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_source("explorer_tree")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let explorer_font = egui::FontId::proportional(13.0);
                            let explorer_row_height = ui
                                .fonts(|fonts| fonts.row_height(&explorer_font))
                                .max(20.0);

                            for node in self.workspace.visible_nodes() {
                                let selected = self
                                    .workspace
                                    .selected()
                                    .map(|path| path == node.path.as_path())
                                    .unwrap_or(false);

                                let row_height = explorer_row_height;
                                let row_width = ui.available_width();
                                let (row_rect, response) = ui.allocate_exact_size(
                                    egui::vec2(row_width, row_height),
                                    egui::Sense::click(),
                                );

                                if selected {
                                    ui.painter().rect_filled(
                                        row_rect,
                                        3.0,
                                        egui::Color32::from_rgb(38, 79, 120),
                                    );
                                } else if response.hovered() {
                                    ui.painter().rect_filled(
                                        row_rect,
                                        3.0,
                                        egui::Color32::from_rgb(29, 34, 44),
                                    );
                                }

                                let icon = match node.kind {
                                    NodeKind::Directory => {
                                        if node.expanded { "▾" } else { "▸" }
                                    }
                                    NodeKind::File => "•",
                                };

                                ui.allocate_ui_at_rect(row_rect, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add_space(8.0 + node.depth as f32 * 14.0);
                                        ui.label(RichText::new(icon).monospace());
                                        ui.add_space(4.0);
                                        ui.label(RichText::new(node.name.clone()).size(13.0));
                                    });
                                });

                                if response.clicked() {
                                    select_path = Some(node.path.clone());

                                    match node.kind {
                                        NodeKind::Directory => {
                                            toggle_dir = Some(node.path.clone());
                                        }
                                        NodeKind::File => {
                                            open_file = Some(node.path.clone());
                                        }
                                    }
                                }
                            }
                        });
                } else {
                    ui.add_space(12.0);
                    ui.vertical_centered(|ui| {
                        ui.label("No folder opened");
                        ui.add_space(6.0);
                        if ui.button("Open Project Folder").clicked() {
                            self.open_project();
                        }
                    });
                }
            });

        if let Some(path) = select_path {
            self.workspace.select(path);
        }

        if let Some(path) = toggle_dir {
            if let Err(error) = self.workspace.toggle_directory(&path) {
                self.set_error(format!(
                    "Failed to read {}: {}",
                    path.display(),
                    error
                ));
            }
        }

        if let Some(path) = open_file {
            self.open_file_path(&path);
        }
    }

    fn open_explorer_dialog(&mut self, kind: ExplorerDialogKind) {
        if kind != ExplorerDialogKind::Delete {
            self.explorer_dialog_name.clear();
        }

        if kind == ExplorerDialogKind::Rename {
            if let Some(path) = self.workspace.selected() {
                if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
                    self.explorer_dialog_name = name.to_string();
                }
            }
        }

        self.explorer_dialog = Some(kind);
    }

    fn render_explorer_dialog(&mut self, ctx: &Context) {
        let Some(kind) = self.explorer_dialog else {
            return;
        };

        let title = match kind {
            ExplorerDialogKind::NewFile => "New File",
            ExplorerDialogKind::NewFolder => "New Folder",
            ExplorerDialogKind::Rename => "Rename",
            ExplorerDialogKind::Delete => "Delete",
        };

        let mut close = false;
        let mut confirmed = false;

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if kind == ExplorerDialogKind::Delete {
                    if let Some(path) = self.workspace.selected() {
                        ui.label(format!(
                            "Delete {}?",
                            path.display()
                        ));
                    } else {
                        ui.label("Nothing is selected.");
                    }
                } else {
                    ui.label(match kind {
                        ExplorerDialogKind::NewFile => "File name",
                        ExplorerDialogKind::NewFolder => "Folder name",
                        ExplorerDialogKind::Rename => "New name",
                        ExplorerDialogKind::Delete => "",
                    });

                    let response = ui.text_edit_singleline(
                        &mut self.explorer_dialog_name,
                    );
                    response.request_focus();
                }

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }

                    let confirm_text = if kind == ExplorerDialogKind::Delete {
                        "Delete"
                    } else {
                        "Apply"
                    };

                    if ui.button(confirm_text).clicked() {
                        confirmed = true;
                    }
                });
            });

        if close {
            self.explorer_dialog = None;
            return;
        }

        if !confirmed {
            return;
        }

        match kind {
            ExplorerDialogKind::NewFile => {
                self.create_explorer_file();
            }
            ExplorerDialogKind::NewFolder => {
                self.create_explorer_folder();
            }
            ExplorerDialogKind::Rename => {
                self.rename_explorer_item();
            }
            ExplorerDialogKind::Delete => {
                self.delete_explorer_item();
            }
        }

        self.explorer_dialog = None;
    }

    fn create_explorer_file(&mut self) {
        let Some(parent) = self.workspace.parent_for_new_item() else {
            self.set_error("Open a project folder first".to_string());
            return;
        };

        let name = self.explorer_dialog_name.trim().to_string();

        match self.workspace.create_file(&parent, &name) {
            Ok(path) => {
                self.open_file_path(&path);
                self.last_error = None;
            }
            Err(error) => self.set_error(format!(
                "Failed to create {}: {}",
                name,
                error
            )),
        }
    }

    fn create_explorer_folder(&mut self) {
        let Some(parent) = self.workspace.parent_for_new_item() else {
            self.set_error("Open a project folder first".to_string());
            return;
        };

        let name = self.explorer_dialog_name.trim().to_string();

        match self.workspace.create_directory(&parent, &name) {
            Ok(_) => self.last_error = None,
            Err(error) => self.set_error(format!(
                "Failed to create {}: {}",
                name,
                error
            )),
        }
    }

    fn rename_explorer_item(&mut self) {
        let Some(old_path) = self.workspace.selected().map(Path::to_path_buf) else {
            return;
        };

        let name = self.explorer_dialog_name.trim().to_string();

        let was_directory = std::fs::metadata(&old_path)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false);
        let old_normalized = normalize_path(&old_path);

        match self.workspace.rename_selected(&name) {
            Ok(Some(new_path)) => {
                let new_normalized = normalize_path(&new_path);

                for editor in &mut self.editors {
                    let Some(editor_path) = editor.path().map(normalize_path) else {
                        continue;
                    };

                    if was_directory {
                        if let Ok(relative) = editor_path.strip_prefix(&old_normalized) {
                            editor.rename_path(new_normalized.join(relative));
                        }
                    } else if editor_path == old_normalized {
                        editor.rename_path(new_normalized.clone());
                    }
                }

                self.last_error = None;
            }
            Ok(None) => {}
            Err(error) => self.set_error(format!(
                "Failed to rename {}: {}",
                old_path.display(),
                error
            )),
        }
    }

    fn delete_explorer_item(&mut self) {
        let Some(path) = self.workspace.selected().map(Path::to_path_buf) else {
            return;
        };

        match self.workspace.delete_selected() {
            Ok(Some(_)) => {
                let normalized = normalize_path(&path);
                let mut index = 0usize;

                while index < self.editors.len() {
                    let remove = self.editors[index]
                        .path()
                        .map(normalize_path)
                        .map(|editor_path| {
                            editor_path == normalized || editor_path.starts_with(&normalized)
                        })
                        .unwrap_or(false);

                    if remove {
                        if self.editors[index].is_dirty() {
                            index += 1;
                        } else {
                            self.editors.remove(index);
                            if index < self.current_tab {
                                self.current_tab = self.current_tab.saturating_sub(1);
                            }
                        }
                    } else {
                        index += 1;
                    }
                }

                if self.editors.is_empty() {
                    self.new_tab();
                } else if self.current_tab >= self.editors.len() {
                    self.current_tab = self.editors.len() - 1;
                }

                self.request_editor_focus();
                self.last_error = None;
            }
            Ok(None) => {}
            Err(error) => self.set_error(format!(
                "Failed to delete {}: {}",
                path.display(),
                error
            )),
        }
    }

    fn render_search_bar(&mut self, ctx: &Context) {
        if !self.find_open {
            return;
        }

        egui::TopBottomPanel::top("search_bar")
            .exact_height(if self.replace_open { 68.0 } else { 36.0 })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Find");
                    let response = ui.text_edit_singleline(&mut self.search_query);
                    response.request_focus();
                    if response.lost_focus() && ui.input(|input| input.key_pressed(eframe::egui::Key::Enter)) {
                        self.find_next();
                    }
                    if ui.button("Next").clicked() { self.find_next(); }
                    if ui.button("Prev").clicked() { self.find_previous(); }
                    if self.replace_open && ui.button("Close").clicked() { self.find_open = false; }
                });

                if self.replace_open {
                    ui.horizontal(|ui| {
                        ui.label("Replace");
                        ui.text_edit_singleline(&mut self.replace_query);
                        if ui.button("Replace All").clicked() {
                            self.replace_all();
                        }
                    });
                }
            });
    }

    fn replace_all(&mut self) {
        if self.search_query.is_empty() { return; }
        let query = self.search_query.clone();
        let replacement = self.replace_query.clone();
        {
            if let Some(editor) = self.editors.get_mut(self.current_tab) {
                if editor.text.contains(&query) {
                    editor.text = editor.text.replace(&query, &replacement);
                    editor.set_cursor(editor.text.len());
                    editor.mark_text_changed(0);
                }
            }
        }
        self.request_editor_focus();
    }

    fn render_go_to_line(&mut self, ctx: &Context) {
        if !self.go_to_line_open { return; }

        egui::TopBottomPanel::top("goto_line")
            .exact_height(36.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Go to line");
                    let response = ui.text_edit_singleline(&mut self.go_to_line_input);
                    if response.lost_focus() && ui.input(|input| input.key_pressed(eframe::egui::Key::Enter)) {
                        self.apply_go_to_line();
                    }
                    if ui.button("Go").clicked() { self.apply_go_to_line(); }
                    if ui.button("Close").clicked() { self.go_to_line_open = false; }
                });
            });
    }

    fn apply_go_to_line(&mut self) {
        let Ok(line) = self.go_to_line_input.trim().parse::<usize>() else { return; };
        if let Some(editor) = self.editors.get_mut(self.current_tab) {
            editor.go_to_line(line);
        }
        self.go_to_line_open = false;
        self.request_editor_focus();
    }

    fn render_editor(&mut self, ctx: &Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(BACKGROUND)
                    .inner_margin(0.0),
            )
            .show(ctx, |ui| {
                if let Some(editor) = self.editors.get_mut(self.current_tab) {
                    let editor_id = ui.id().with("code_editor");
                    editor.ui(ui, editor_id, &self.syntax_set, &self.theme_set, &self.settings);
                }
            });
    }

    fn render_status(&self, ctx: &Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(26.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    if let Some(editor) = self.editors.get(self.current_tab) {
                        let stats = editor.stats();

                        ui.label(editor.file_name());
                        ui.label(format!("Lines: {}", stats.lines));
                        ui.label(format!("Chars: {}", stats.chars));

                        if stats.selected > 0 {
                            ui.label(format!("Selected: {}", stats.selected));
                        }

                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(format!(
                                    "Ln {}, Col {}",
                                    stats.cursor_line,
                                    stats.cursor_column,
                                ));
                            },
                        );
                    }
                });
            });

        if let Some(error) = &self.last_error {
            egui::TopBottomPanel::bottom("error_status")
                .exact_height(22.0)
                .show(ctx, |ui| {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                });
        }
    }

    fn render_settings(&mut self, ctx: &Context) {
        if !self.settings_open {
            return;
        }

        let mut open = true;
        let mut reset = false;
        let settings = &mut self.settings;

        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.heading("Editor");
                ui.add_space(6.0);

                egui::Grid::new("settings_editor_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Font size");
                        ui.add(
                            egui::DragValue::new(&mut settings.font_size)
                                .clamp_range(8.0..=32.0)
                                .speed(0.25),
                        );
                        ui.end_row();

                        ui.label("Tab size");
                        ui.add(
                            egui::DragValue::new(&mut settings.tab_size)
                                .clamp_range(1..=16)
                                .speed(1),
                        );
                        ui.end_row();

                        ui.label("Line numbers");
                        ui.checkbox(&mut settings.show_line_numbers, "Show");
                        ui.end_row();

                        ui.label("Current line");
                        ui.checkbox(
                            &mut settings.highlight_current_line,
                            "Highlight",
                        );
                        ui.end_row();

                        ui.label("Left text padding");
                        ui.add(
                            egui::DragValue::new(&mut settings.text_left_padding)
                                .clamp_range(0.0..=32.0)
                                .speed(0.5),
                        );
                        ui.end_row();

                        ui.label("Top text padding");
                        ui.add(
                            egui::DragValue::new(&mut settings.text_top_padding)
                                .clamp_range(0.0..=32.0)
                                .speed(0.5),
                        );
                        ui.end_row();

                        ui.label("Bottom text padding");
                        ui.add(
                            egui::DragValue::new(&mut settings.text_bottom_padding)
                                .clamp_range(0.0..=64.0)
                                .speed(0.5),
                        );
                        ui.end_row();

                        ui.label("Gutter padding");
                        ui.add(
                            egui::DragValue::new(&mut settings.gutter_right_padding)
                                .clamp_range(0.0..=24.0)
                                .speed(0.5),
                        );
                        ui.end_row();
                    });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);

                ui.heading("Appearance");
                ui.add_space(6.0);

                egui::ComboBox::from_id_source("settings_theme")
                    .selected_text(settings.theme.name())
                    .show_ui(ui, |ui| {
                        for theme in EditorTheme::ALL {
                            ui.selectable_value(
                                &mut settings.theme,
                                theme,
                                theme.name(),
                            );
                        }
                    });

                ui.add_space(8.0);

                egui::Grid::new("settings_colors_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Editor background");
                        ui.color_edit_button_srgba(&mut settings.background);
                        ui.end_row();

                        ui.label("Gutter background");
                        ui.color_edit_button_srgba(&mut settings.gutter_background);
                        ui.end_row();

                        ui.label("Current line");
                        ui.color_edit_button_srgba(
                            &mut settings.current_line_background,
                        );
                        ui.end_row();

                        ui.label("Selection");
                        ui.color_edit_button_srgba(
                            &mut settings.selection_background,
                        );
                        ui.end_row();

                        ui.label("Line numbers");
                        ui.color_edit_button_srgba(&mut settings.line_number_color);
                        ui.end_row();

                        ui.label("Cursor");
                        ui.color_edit_button_srgba(&mut settings.cursor_color);
                        ui.end_row();

                        ui.label("Separator");
                        ui.color_edit_button_srgba(&mut settings.separator_color);
                        ui.end_row();

                        ui.label("Active tab");
                        ui.color_edit_button_srgba(&mut settings.tab_active);
                        ui.end_row();

                        ui.label("Tab hover");
                        ui.color_edit_button_srgba(&mut settings.tab_hover);
                        ui.end_row();
                    });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Reset to defaults").clicked() {
                        reset = true;
                    }

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(RichText::new("Changes apply immediately").weak());
                        },
                    );
                });
            });

        if reset {
            self.settings.reset();
            self.request_editor_focus();
        }

        self.settings_open = open;

        if !self.settings_open {
            self.request_editor_focus();
        }
    }

    fn render_close_confirmation(&mut self, ctx: &Context) {
        let Some(uid) = self.close_confirmation else {
            return;
        };

        let Some(index) = self.editors.iter().position(|editor| editor.uid() == uid) else {
            self.close_confirmation = None;
            return;
        };

        let name = self.editors[index].file_name();
        let mut action = None;

        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Save changes to {}?", name));
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        action = Some(0);
                    }
                    if ui.button("Discard").clicked() {
                        action = Some(1);
                    }
                    if ui.button("Cancel").clicked() {
                        action = Some(2);
                    }
                });
            });

        match action {
            Some(0) => {
                self.current_tab = index;
                self.save_current();

                let still_open = self
                    .editors
                    .iter()
                    .position(|editor| editor.uid() == uid);

                if let Some(index) = still_open {
                    if !self.editors[index].is_dirty() {
                        self.remove_tab(index);
                        self.close_confirmation = None;
                    }
                } else {
                    self.close_confirmation = None;
                }
            }
            Some(1) => {
                if let Some(index) = self.editors.iter().position(|editor| editor.uid() == uid) {
                    self.remove_tab(index);
                }
                self.close_confirmation = None;
            }
            Some(2) => {
                self.close_confirmation = None;
                self.request_editor_focus();
            }
            _ => {}
        }
    }

}

impl eframe::App for CodeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if !self.startup_logged {
            println!(
                "[startup] GUI ready in {:.2} ms",
                self.startup_time.elapsed().as_secs_f64() * 1000.0
            );
            self.startup_logged = true;
        }

        self.handle_global_shortcuts(ctx);
        self.render_menu(ctx);
        self.render_tabs(ctx);
        self.render_explorer(ctx);
        self.render_search_bar(ctx);
        self.render_go_to_line(ctx);
        self.render_editor(ctx);
        self.render_status(ctx);
        self.render_settings(ctx);
        self.render_close_confirmation(ctx);
        self.render_explorer_dialog(ctx);
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
}