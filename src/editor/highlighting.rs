use eframe::egui::{text::LayoutJob, Color32, FontId};
use syntect::easy::HighlightLines;
use syntect::highlighting::Theme;
use syntect::parsing::{SyntaxReference, SyntaxSet};

#[derive(Clone, Copy)]
struct StyleRange {
    start: usize,
    end: usize,
    color: Color32,
}

#[derive(Default)]
struct CachedLine {
    ranges: Vec<StyleRange>,
}

pub struct Highlighter {
    syntax: SyntaxReference,
    theme: Theme,
    lines: Vec<Option<CachedLine>>,
    revisions: Vec<u64>,
    revision: u64,
}

impl Highlighter {
    pub fn new(
        syntax_set: &SyntaxSet,
        theme_set: &syntect::highlighting::ThemeSet,
        language: &str,
    ) -> Self {
        let syntax = syntax_set
            .find_syntax_by_name(language)
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
        let theme = theme_set.themes["base16-ocean.dark"].clone();

        Self {
            syntax: syntax.clone(),
            theme,
            lines: Vec::new(),
            revisions: Vec::new(),
            revision: 0,
        }
    }

    pub fn sync_line_count(&mut self, count: usize) {
        self.lines.resize_with(count, || None);
        self.revisions.resize(count, 0);
    }

    pub fn invalidate_from(&mut self, line: usize, count: usize) {
        self.revision = self.revision.wrapping_add(1);
        self.sync_line_count(count);

        for index in line..self.lines.len() {
            self.lines[index] = None;
            self.revisions[index] = 0;
        }
    }

    pub fn ensure_range(
        &mut self,
        text: &str,
        line_starts: &[usize],
        start_line: usize,
        end_line: usize,
        syntax_set: &SyntaxSet,
    ) {
        if line_starts.is_empty() || start_line > end_line {
            return;
        }

        let last_line = end_line.min(line_starts.len() - 1);
        let first_missing = (start_line..=last_line).find(|line| {
            self.lines[*line].is_none() || self.revisions[*line] != self.revision
        });

        let Some(first_missing) = first_missing else {
            return;
        };

        let syntax = self.syntax.clone();
        let theme = self.theme.clone();
        let mut highlighter = HighlightLines::new(&syntax, &theme);
        let mut line = 0usize;

        while line <= last_line {
            let line_start = line_starts[line];
            let line_end = if line + 1 < line_starts.len() {
                line_starts[line + 1]
            } else {
                text.len()
            };
            let line_text = &text[line_start..line_end];

            let should_cache = line >= first_missing
                && (self.lines[line].is_none() || self.revisions[line] != self.revision);

            if should_cache {
                let mut cached = CachedLine::default();

                match highlighter.highlight_line(line_text, syntax_set) {
                    Ok(ranges) => {
                        let mut offset = 0usize;

                        for (style, chunk) in ranges {
                            if chunk.is_empty() {
                                continue;
                            }

                            let start = offset;
                            offset += chunk.len();

                            cached.ranges.push(StyleRange {
                                start,
                                end: offset,
                                color: Color32::from_rgb(
                                    style.foreground.r,
                                    style.foreground.g,
                                    style.foreground.b,
                                ),
                            });
                        }
                    }
                    Err(_) => {
                        let visible_end = display_end(line_text);
                        if visible_end > 0 {
                            cached.ranges.push(StyleRange {
                                start: 0,
                                end: visible_end,
                                color: Color32::WHITE,
                            });
                        }
                    }
                }

                self.lines[line] = Some(cached);
                self.revisions[line] = self.revision;
            }

            line += 1;
        }
    }

    pub fn line_job(
        &self,
        text: &str,
        line: usize,
        line_starts: &[usize],
        font_id: &FontId,
    ) -> LayoutJob {
        let mut job = LayoutJob::default();

        if line >= line_starts.len() {
            return job;
        }

        let start = line_starts[line];
        let end = if line + 1 < line_starts.len() {
            line_starts[line + 1]
        } else {
            text.len()
        };

        let line_text = &text[start..end];
        let visible_end = display_end(line_text);
        let visible_text = &line_text[..visible_end];

        let default_format = egui::TextFormat {
            font_id: font_id.clone(),
            color: Color32::WHITE,
            background: Color32::TRANSPARENT,
            ..Default::default()
        };

        let Some(Some(cached)) = self.lines.get(line) else {
            job.append(visible_text, 0.0, default_format);
            return job;
        };

        let mut cursor = 0usize;

        for range in &cached.ranges {
            let range_start = range.start.min(visible_end);
            let range_end = range.end.min(visible_end);

            if range_start > cursor {
                job.append(
                    &visible_text[cursor..range_start],
                    0.0,
                    default_format.clone(),
                );
            }

            if range_end > range_start {
                job.append(
                    &visible_text[range_start..range_end],
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: range.color,
                        background: Color32::TRANSPARENT,
                        ..Default::default()
                    },
                );
                cursor = range_end;
            }
        }

        if cursor < visible_end {
            job.append(
                &visible_text[cursor..visible_end],
                0.0,
                default_format,
            );
        }

        job
    }
}

fn display_end(line: &str) -> usize {
    line.strip_suffix('\n')
        .map(|value| value.len())
        .unwrap_or(line.len())
}
