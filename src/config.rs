use egui::Color32;

pub const FONT_SIZE: f32 = 14.0;
pub const TEXT_LEFT_PADDING: f32 = 7.0;
pub const TEXT_TOP_PADDING: f32 = 4.0;
pub const TEXT_BOTTOM_PADDING: f32 = 12.0;
pub const GUTTER_RIGHT_PADDING: f32 = 7.0;
pub const INDENT: &str = "    ";

pub const BACKGROUND: Color32 = Color32::from_rgb(15, 17, 23);
pub const GUTTER_BACKGROUND: Color32 = Color32::from_rgb(13, 15, 20);
pub const CURRENT_LINE_BACKGROUND: Color32 = Color32::from_rgb(23, 27, 36);
pub const SELECTION_BACKGROUND: Color32 = Color32::from_rgb(38, 79, 120);
pub const LINE_NUMBER_COLOR: Color32 = Color32::from_rgb(92, 99, 112);
pub const CURSOR_COLOR: Color32 = Color32::from_rgb(196, 213, 255);
pub const SEPARATOR_COLOR: Color32 = Color32::from_rgb(35, 40, 51);
pub const TAB_ACTIVE: Color32 = Color32::from_rgb(23, 27, 36);
pub const TAB_HOVER: Color32 = Color32::from_rgb(29, 34, 44);

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
