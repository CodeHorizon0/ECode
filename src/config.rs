use egui::Color32;

pub const FONT_SIZE: f32 = 14.0;
pub const TEXT_LEFT_PADDING: f32 = 7.0;
pub const TEXT_TOP_PADDING: f32 = 4.0;
pub const TEXT_BOTTOM_PADDING: f32 = 12.0;
pub const GUTTER_RIGHT_PADDING: f32 = 7.0;

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
    "HTML",
    "CSS",
    "JSON",
    "TOML",
    "YAML",
    "XML",
    "Markdown",
    "SQL",
    "C",
    "C++",
    "C#",
    "Java",
    "Go",
    "Kotlin",
    "Swift",
    "Dart",
    "PHP",
    "Ruby",
    "Lua",
    "Shell Script",
    "PowerShell",
    "Dockerfile",
    "Makefile",
    "Plain Text",
];

pub fn language_for_path(path: &std::path::Path) -> &'static str {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    match file_name.to_ascii_lowercase().as_str() {
        "dockerfile" => "Dockerfile",
        "makefile" | "gnumakefile" => "Makefile",
        _ => path
            .extension()
            .and_then(|value| value.to_str())
            .map(language_for_extension)
            .unwrap_or("Plain Text"),
    }
}

pub fn language_for_extension(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "rs" => "Rust",
        "py" | "pyw" => "Python",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "ts" | "tsx" | "mts" | "cts" => "TypeScript",
        "html" | "htm" | "xhtml" => "HTML",
        "css" => "CSS",
        "scss" | "sass" => "CSS",
        "json" | "json5" | "jsonc" => "JSON",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "xml" | "xsd" | "xsl" | "xslt" => "XML",
        "md" | "markdown" | "mdown" | "mkdn" => "Markdown",
        "sql" => "SQL",
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "C++",
        "cs" => "C#",
        "java" => "Java",
        "go" => "Go",
        "kt" | "kts" => "Kotlin",
        "swift" => "Swift",
        "dart" => "Dart",
        "php" => "PHP",
        "rb" | "ruby" => "Ruby",
        "lua" => "Lua",
        "sh" | "bash" | "zsh" | "fish" => "Shell Script",
        "ps1" | "psm1" | "psd1" => "PowerShell",
        _ => "Plain Text",
    }
}

