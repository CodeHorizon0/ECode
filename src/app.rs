use std::time::Instant;

use eframe::egui::{self, Context};
use eframe::CreationContext;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::config::{BACKGROUND, LANGUAGES};
use crate::editor::CodeEditor;

pub struct CodeApp {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    editors: Vec<CodeEditor>,
    current_tab: usize,
    show_panel: bool,
    tab_names: Vec<String>,
    new_tab_language: String,
    startup_time: Instant,
    startup_logged: bool,
}

impl CodeApp {
    pub fn new(
        _cc: &CreationContext<'_>,
        startup_time: Instant,
    ) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
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
            tab_names: vec!["Tab 1".to_string()],
            new_tab_language: "Rust".to_string(),
            startup_time,
            startup_logged: false,
        }
    }

    fn new_tab(&mut self) {
        let editor = CodeEditor::new(
            &self.syntax_set,
            &self.theme_set,
            &self.new_tab_language,
            "// new file",
        );

        self.editors.push(editor);
        self.tab_names
            .push(format!("Tab {}", self.editors.len()));
        self.current_tab = self.editors.len() - 1;
    }

    fn close_current_tab(&mut self) {
        if self.editors.len() <= 1 {
            return;
        }

        self.editors.remove(self.current_tab);
        self.tab_names.remove(self.current_tab);

        if self.current_tab >= self.editors.len() {
            self.current_tab = self.editors.len() - 1;
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
            println!(
                "[startup] GUI ready in {:.2} ms",
                self.startup_time.elapsed().as_secs_f64() * 1000.0
            );
            self.startup_logged = true;
        }

        self.render_menu(ctx);
        self.render_sidebar(ctx);
        self.render_editor(ctx);
        self.render_status(ctx);
    }
}

impl CodeApp {
    fn render_menu(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("menu")
            .exact_height(38.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    if ui.button("New Tab").clicked() {
                        self.new_tab();
                    }

                    if ui
                        .add_enabled(
                            self.editors.len() > 1,
                            egui::Button::new("Close Tab"),
                        )
                        .clicked()
                    {
                        self.close_current_tab();
                    }

                    ui.separator();
                    ui.label("Language:");

                    egui::ComboBox::from_id_source("lang_combo")
                        .width(120.0)
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

                    if ui.button("Toggle Panel").clicked() {
                        self.show_panel = !self.show_panel;
                    }
                });
            });
    }

    fn render_sidebar(&mut self, ctx: &Context) {
        if !self.show_panel {
            return;
        }

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

                for (index, name) in self.tab_names.iter().enumerate() {
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
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("No editor");
                    });
                }
            });
    }

    fn render_status(&self, ctx: &Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(26.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    ui.label(format!("Tab {}", self.current_tab + 1));

                    if let Some(editor) = self.editors.get(self.current_tab) {
                        let stats = editor.stats();

                        ui.label(format!("Lines: {}", stats.lines));
                        ui.label(format!("Chars: {}", stats.chars));

                        if stats.selected > 0 {
                            ui.label(format!("Selected: {}", stats.selected));
                        }

                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(format!(
                                    "Cursor: {}:{}",
                                    stats.cursor_line,
                                    stats.cursor_column,
                                ));
                            },
                        );
                    }
                });
            });
    }
}
