use super::CodeEditor;

pub fn rebuild_line_index(editor: &mut CodeEditor) {
    editor.line_starts.clear();
    editor.line_char_counts.clear();
    editor.line_starts.push(0);

    let mut line_start = 0usize;

    for (index, byte) in editor.text.bytes().enumerate() {
        if byte == b'\n' {
            let count = editor.text[line_start..index].chars().count();
            editor.line_char_counts.push(count as u32);
            editor.line_starts.push(index + 1);
            line_start = index + 1;
        }
    }

    editor.line_char_counts
        .push(editor.text[line_start..].chars().count() as u32);
    editor.max_line_chars = editor
        .line_char_counts
        .iter()
        .copied()
        .max()
        .unwrap_or(1)
        .max(1) as usize;
    editor.char_count = editor.text.chars().count();
}

pub fn line_from_index(editor: &CodeEditor, index: usize) -> usize {
    let index = index.min(editor.text.len());

    match editor.line_starts.binary_search(&index) {
        Ok(value) => value,
        Err(value) => value.saturating_sub(1),
    }
}

pub fn line_end(editor: &CodeEditor, line: usize) -> usize {
    if line + 1 < editor.line_starts.len() {
        editor.line_starts[line + 1].saturating_sub(1)
    } else {
        editor.text.len()
    }
}

pub fn byte_index_from_column(
    editor: &CodeEditor,
    line_start: usize,
    line_end: usize,
    column: usize,
) -> usize {
    if column == 0 {
        return line_start;
    }

    let mut current = 0usize;

    for (offset, character) in editor.text[line_start..line_end].char_indices() {
        if current == column {
            return line_start + offset;
        }

        current += 1;

        if current == column {
            return line_start + offset + character.len_utf8();
        }
    }

    line_end
}

pub fn char_to_byte(text: &str, column: usize) -> usize {
    if column == 0 {
        return 0;
    }

    let mut current = 0usize;

    for (index, character) in text.char_indices() {
        if current == column {
            return index;
        }

        current += 1;

        if current == column {
            return index + character.len_utf8();
        }
    }

    text.len()
}

