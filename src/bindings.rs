use eframe::egui::{Key, KeyboardShortcut, Modifiers};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Save,
    SaveAs,
    SaveAll,
    OpenFile,
    OpenFolder,
    NewTab,
    CloseTab,
    CloseAllTabs,
    NextTab,
    PreviousTab,
    Undo,
    Redo,
    Find,
    FindNext,
    FindPrevious,
    Replace,
    GoToLine,
    SelectAll,
    SelectWord,
    SelectLine,
    DeleteLine,
    MoveLineUp,
    MoveLineDown,
    ToggleLineComment,
    RenameSelected,
}

#[derive(Clone, Copy)]
pub struct Binding {
    pub action: Action,
    pub shortcut: KeyboardShortcut,
}

impl Binding {
    pub const fn new(
        action: Action,
        modifiers: Modifiers,
        key: Key,
    ) -> Self {
        Self {
            action,
            shortcut: KeyboardShortcut {
                modifiers,
                logical_key: key,
            },
        }
    }
}

const CTRL: Modifiers = Modifiers {
    ctrl: true,
    ..Modifiers::NONE
};

const CTRL_SHIFT: Modifiers = Modifiers {
    ctrl: true,
    shift: true,
    ..Modifiers::NONE
};

const CTRL_ALT: Modifiers = Modifiers {
    ctrl: true,
    alt: true,
    ..Modifiers::NONE
};

const ALT: Modifiers = Modifiers {
    alt: true,
    ..Modifiers::NONE
};

const SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::NONE
};

pub fn default_bindings() -> Vec<Binding> {
    vec![
        Binding::new(Action::Save, CTRL, Key::S),
        Binding::new(Action::SaveAs, CTRL_SHIFT, Key::S),
        Binding::new(Action::SaveAll, CTRL_ALT, Key::S),
        Binding::new(Action::OpenFile, CTRL, Key::O),
        Binding::new(Action::OpenFile, CTRL, Key::P),
        Binding::new(Action::OpenFolder, CTRL_SHIFT, Key::O),
        Binding::new(Action::NewTab, CTRL, Key::N),
        Binding::new(Action::NewTab, CTRL, Key::T),
        Binding::new(Action::CloseTab, CTRL, Key::W),
        Binding::new(Action::CloseAllTabs, CTRL_SHIFT, Key::W),
        Binding::new(Action::NextTab, CTRL, Key::Tab),
        Binding::new(Action::PreviousTab, CTRL_SHIFT, Key::Tab),
        Binding::new(Action::NextTab, CTRL, Key::PageDown),
        Binding::new(Action::PreviousTab, CTRL, Key::PageUp),
        Binding::new(Action::Undo, CTRL, Key::Z),
        Binding::new(Action::Redo, CTRL, Key::Y),
        Binding::new(Action::Redo, CTRL_SHIFT, Key::Z),
        Binding::new(Action::Find, CTRL, Key::F),
        Binding::new(Action::FindNext, Modifiers::NONE, Key::F3),
        Binding::new(Action::FindPrevious, SHIFT, Key::F3),
        Binding::new(Action::Replace, CTRL, Key::H),
        Binding::new(Action::GoToLine, CTRL, Key::G),
        Binding::new(Action::SelectAll, CTRL, Key::A),
        Binding::new(Action::SelectWord, CTRL, Key::D),
        Binding::new(Action::SelectLine, CTRL, Key::L),
        Binding::new(Action::DeleteLine, CTRL_SHIFT, Key::K),
        Binding::new(Action::MoveLineUp, ALT, Key::ArrowUp),
        Binding::new(Action::MoveLineDown, ALT, Key::ArrowDown),
        Binding::new(Action::ToggleLineComment, CTRL, Key::Slash),
        Binding::new(Action::RenameSelected, Modifiers::NONE, Key::F2),
    ]
}
