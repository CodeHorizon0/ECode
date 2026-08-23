use eframe::egui::Color32;

use crate::config::{
    BACKGROUND,
    CURSOR_COLOR,
    CURRENT_LINE_BACKGROUND,
    FONT_SIZE,
    GUTTER_RIGHT_PADDING,
    LINE_NUMBER_COLOR,
    SELECTION_BACKGROUND,
    TEXT_BOTTOM_PADDING,
    TEXT_LEFT_PADDING,
    TEXT_TOP_PADDING,
    SEPARATOR_COLOR,
    TAB_ACTIVE,
    TAB_HOVER,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditorTheme {
    ECodeDark,
    Base16OceanDark,
}

impl EditorTheme {
    pub const ALL: [Self; 2] = [
        Self::ECodeDark,
        Self::Base16OceanDark,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::ECodeDark => "ECode Dark",
            Self::Base16OceanDark => "Base16 Ocean Dark",
        }
    }
}

#[derive(Clone)]
pub struct EditorSettings {
    pub font_size: f32,
    pub tab_size: usize,
    pub show_line_numbers: bool,
    pub highlight_current_line: bool,
    pub text_left_padding: f32,
    pub text_top_padding: f32,
    pub text_bottom_padding: f32,
    pub gutter_right_padding: f32,
    pub background: Color32,
    pub gutter_background: Color32,
    pub current_line_background: Color32,
    pub selection_background: Color32,
    pub line_number_color: Color32,
    pub cursor_color: Color32,
    pub separator_color: Color32,
    pub tab_active: Color32,
    pub tab_hover: Color32,
    pub theme: EditorTheme,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_size: FONT_SIZE,
            tab_size: 4,
            show_line_numbers: true,
            highlight_current_line: true,
            text_left_padding: TEXT_LEFT_PADDING,
            text_top_padding: TEXT_TOP_PADDING,
            text_bottom_padding: TEXT_BOTTOM_PADDING,
            gutter_right_padding: GUTTER_RIGHT_PADDING,
            background: BACKGROUND,
            gutter_background: crate::config::GUTTER_BACKGROUND,
            current_line_background: CURRENT_LINE_BACKGROUND,
            selection_background: SELECTION_BACKGROUND,
            line_number_color: LINE_NUMBER_COLOR,
            cursor_color: CURSOR_COLOR,
            separator_color: SEPARATOR_COLOR,
            tab_active: TAB_ACTIVE,
            tab_hover: TAB_HOVER,
            theme: EditorTheme::ECodeDark,
        }
    }
}

impl EditorSettings {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
