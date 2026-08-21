use std::path::{Path, PathBuf};
use std::time::Instant;

use eframe::egui::{self, Context, RichText};
use eframe::CreationContext;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::config::{BACKGROUND, LANGUAGES, TAB_ACTIVE, TAB_HOVER};
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
    close_confirmation: Option<usize>,
    explorer_dialog: Option<ExplorerDialogKind>,
    explorer_dialog_name: String,
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
        );

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
            explorer_dialog: None,
            explorer_dialog_name: String::new(),
        }
    }

    fn new_tab(&mut self) {
        let editor = CodeEditor::new_with_name(
            &self.syntax_set,
            &self.theme_set,
            &self.new_tab_language,
            "",
            format!("Untitled {}", self.untitled_index()),
        );

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

        let editor = CodeEditor::from_file(
            &self.syntax_set,
            &self.theme_set,
            file.path,
            &file.language,
            file.text,
        );

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
            self.close_confirmation = Some(self.current_tab);
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
            self.close_confirmation = Some(index);
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
        let mut save = false;
        let mut save_as = false;
        let mut open = false;
        let mut new_tab = false;
        let mut close = false;
        let mut project = false;

        ctx.input_mut(|input| {
            if input.modifiers.ctrl
                && input.modifiers.shift
                && input.consume_key(input.modifiers, egui::Key::S)
            {
                save_as = true;
            } else if input.modifiers.ctrl
                && !input.modifiers.shift
                && input.consume_key(input.modifiers, egui::Key::S)
            {
                save = true;
            }

            if input.modifiers.ctrl
                && !input.modifiers.shift
                && input.consume_key(input.modifiers, egui::Key::O)
            {
                open = true;
            }

            if input.modifiers.ctrl
                && !input.modifiers.shift
                && input.consume_key(input.modifiers, egui::Key::N)
            {
                new_tab = true;
            }

            if input.modifiers.ctrl
                && !input.modifiers.shift
                && input.consume_key(input.modifiers, egui::Key::W)
            {
                close = true;
            }

            if input.modifiers.ctrl
                && input.modifiers.shift
                && input.consume_key(input.modifiers, egui::Key::O)
            {
                project = true;
            }
        });

        if save_as {
            self.save_current_as();
        } else if save {
            self.save_current();
        } else if project {
            self.open_project();
        } else if open {
            self.open_file_dialog();
        } else if new_tab {
            self.new_tab();
        } else if close {
            self.close_current_tab();
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
                                    .fill(if selected { TAB_ACTIVE } else { egui::Color32::TRANSPARENT })
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
                                                TAB_HOVER,
                                            );
                                        }

                                        if response.clicked() {
                                            self.current_tab = index;
                                            self.request_editor_focus();
                                        }

                                        if ui.small_button("×").clicked() {
                                            self.close_tab(index);
                                        }
                                    });
                                });
                            }
                        });
                    });
            });
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
                ui.add_space(6.0);

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

                ui.add_space(4.0);

                let root = self.workspace.root().map(Path::to_path_buf);

                if let Some(root) = root {
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
                            for node in self.workspace.visible_nodes() {
                                let selected = self
                                    .workspace
                                    .selected()
                                    .map(|path| path == node.path.as_path())
                                    .unwrap_or(false);

                                let indent = 8.0 + node.depth as f32 * 14.0;
                                let icon = match node.kind {
                                    NodeKind::Directory => {
                                        if node.expanded { "▾" } else { "▸" }
                                    }
                                    NodeKind::File => "•",
                                };

                                let label = format!("{} {}", icon, node.name);

                                let mut response = None;

                                ui.horizontal(|ui| {
                                    ui.add_space(indent);
                                    response = Some(ui.add_sized(
                                        [ui.available_width(), 24.0],
                                        egui::SelectableLabel::new(selected, label),
                                    ));
                                });

                                if response.map(|value| value.clicked()).unwrap_or(false) {
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
                    editor.ui(ui, editor_id, &self.syntax_set);
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

    fn render_close_confirmation(&mut self, ctx: &Context) {
        let Some(index) = self.close_confirmation else {
            return;
        };

        if index >= self.editors.len() {
            self.close_confirmation = None;
            return;
        }

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

                if !self.editors[index].is_dirty() {
                    self.remove_tab(index);
                    self.close_confirmation = None;
                }
            }
            Some(1) => {
                self.remove_tab(index);
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
        self.render_editor(ctx);
        self.render_status(ctx);
        self.render_close_confirmation(ctx);
        self.render_explorer_dialog(ctx);
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
}
