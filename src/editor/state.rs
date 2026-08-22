#[derive(Clone, Copy)]
pub struct BracketPair {
    pub open: char,
    pub close: char,
}

pub const BRACKET_PAIRS: [BracketPair; 3] = [
    BracketPair { open: '(', close: ')' },
    BracketPair { open: '[', close: ']' },
    BracketPair { open: '{', close: '}' },
];

#[derive(Clone, Copy, Default)]
pub struct EditorStats {
    pub lines: usize,
    pub chars: usize,
    pub selected: usize,
    pub cursor_line: usize,
    pub cursor_column: usize,
}

#[derive(Clone)]
pub(super) struct EditorSnapshot {
    pub text: String,
    pub cursor: usize,
    pub selection: Option<std::ops::Range<usize>>,
    pub selection_anchor: usize,
}
