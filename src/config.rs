use egui::Color32;

pub const FONT_SIZE: f32 = 14.0;
pub const TEXT_LEFT_PADDING: f32 = 7.0;
pub const TEXT_TOP_PADDING: f32 = 4.0;
pub const GUTTER_RIGHT_PADDING: f32 = 7.0;
pub const INDENT: &str = "    ";

pub const BACKGROUND: Color32 = Color32::from_rgb(30, 30, 30);
pub const GUTTER_BACKGROUND: Color32 = Color32::from_rgb(27, 27, 27);
pub const CURRENT_LINE_BACKGROUND: Color32 = Color32::from_rgb(43, 45, 52);
pub const SELECTION_BACKGROUND: Color32 = Color32::from_rgb(63, 92, 140);
pub const LINE_NUMBER_COLOR: Color32 = Color32::from_rgb(105, 105, 105);
pub const CURSOR_COLOR: Color32 = Color32::WHITE;
pub const SEPARATOR_COLOR: Color32 = Color32::from_rgb(40, 40, 40);
pub const TAB_ACTIVE: Color32 = Color32::from_rgb(36, 38, 44);
pub const TAB_HOVER: Color32 = Color32::from_rgb(45, 47, 54);

pub const LANGUAGES: &[&str] = &[
    "Rust",
    "Python",
    "JavaScript",
    "TypeScript",
    "C",
    "C++",
    "Plain Text",
];

pub fn language_for_extension(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "rs" => "Rust",
        "py" => "Python",
        "js" | "jsx" => "JavaScript",
        "ts" | "tsx" => "TypeScript",
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" => "C++",
        _ => "Plain Text",
    }
}
