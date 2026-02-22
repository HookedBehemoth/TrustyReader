use alloc::{string::String, vec::Vec};

use crate::res::font;

#[derive(Clone, Copy)]
pub struct Options {
    pub width: u16,
    pub language: hypher::Lang,
    pub font: font::Font,
    // split by type?
    space_width: u16,
    dash_width: u16,
}

impl Options {
    pub fn new(
        width: u16,
        language: hypher::Lang,
        font: font::Font,
    ) -> Self {
        let font_def = font.definition(font::FontStyle::Regular);
        let space_width = font_def.char_width(' ').unwrap() as u16;
        let dash_width = font_def.char_width('-').unwrap() as u16;
        Self {
            width,
            language,
            font,
            space_width,
            dash_width,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Start,
    Center,
    End,
    Justify,
}
impl Alignment {
    pub fn repr(self) -> &'static str {
        match self {
            Alignment::Start => "Start",
            Alignment::Center => "Center",
            Alignment::End => "End",
            Alignment::Justify => "Justify",
        }
    }
}

pub struct Text<'a> {
    pub text: &'a str,
    pub x: u16,
    pub style: font::FontStyle,
}

pub struct Line<'a> {
    pub words: Vec<Text<'a>>,
    pub hyphenated: bool,
}

pub struct Image<'a> {
    pub handle: &'a str,
    pub width: u16,
    pub height: u16,
}

pub enum Block<'a> {
    Text(Vec<Line<'a>>),
    Image(Image<'a>),
}

/// Input for layouting.
pub struct Run {
    pub text: String,
    pub style: font::FontStyle,
    pub breaking: bool,
}

pub fn layout_text<'a>(
    options: Options,
    alignment: Alignment,
    indent: u16,
    runs: &'a [Run]
) -> Vec<Line<'a>> {
    let mut x = indent;
    let mut lines: Vec<Line> = Vec::new();
    let mut current_line = Line {
        words: Vec::new(),
        hyphenated: false,
    };

    for run in runs {
        let font = options.font.definition(run.style);

        for mut word in run.text.split_whitespace() {
            let mut word_width = font.word_width(word);

            // advance to the next line
            if x + options.space_width + word_width >= options.width {
                if let Some((remaining, remaining_width)) =
                    hyphenate(x, word, &mut current_line, options, run.style)
                {
                    word = remaining;
                    word_width = font.word_width(word);
                    x = options.width - remaining_width;
                }

                let space = options.width.saturating_sub(x);
                align(alignment, space, &mut current_line.words);
                lines.push(current_line);
                x = 0;
                current_line = Line {
                    words: Vec::new(),
                    hyphenated: false,
                };
            }

            // add space before the word
            if !current_line.words.is_empty() {
                x += options.space_width;
            }

            // Add word to current line
            current_line.words.push(Text { text: word, x, style: run.style });
            x += word_width;
        }

        if run.breaking {
            let space = options.width.saturating_sub(x);
            align(alignment, space, &mut current_line.words);
            lines.push(current_line);
            x = 0;
            current_line = Line {
                words: Vec::new(),
                hyphenated: false,
            };
        }
    }

    if !current_line.words.is_empty() {
        let space = options.width.saturating_sub(x);
        nudge(alignment, space, &mut current_line.words);

        lines.push(current_line);
    }

    lines
}

/// Greedily hyphenate the given word to fit in the remaining space.
fn hyphenate<'a>(
    mut x: u16,
    word: &'a str,
    current_line: &mut Line<'a>,
    options: Options,
    style: font::FontStyle,
) -> Option<(&'a str, u16)> {
    if word.len() < 5 {
        return None;
    }

    let space_width = options.space_width;
    let dash_width = options.dash_width;
    let mut space = options.width.saturating_sub(x + space_width + dash_width);
    if space == 0 {
        return None;
    }

    let font = options.font.definition(style);
    let mut length = 0;
    for part in hypher::hyphenate(word, options.language) {
        let part_width = font.word_width(part);
        if part_width > space {
            if length == 0 {
                return None;
            }

            // add space before the word
            if !current_line.words.is_empty() {
                x += options.space_width;
            }

            let text = &word[0..length];
            let text = if text.chars().last() != Some('-') {
                text
            } else {
                x += options.dash_width;
                &text[0..text.len() - 1]
            };
            current_line.hyphenated = true;
            current_line.words.push(Text { text, x, style });
            return Some((&word[length..], space));
        }

        length += part.len();
        space -= part_width;
    }

    None
}

/// Justify the words in the current line by splitting
/// the remaining space evenly between the words.
fn justify(room: u16, words: &mut [Text]) {
    let whitespaces = words.len().saturating_sub(1);
    if whitespaces == 0 {
        return;
    }
    let space = room / whitespaces as u16;
    let mut rem = room % whitespaces as u16;
    let mut offset = 0;
    for word in words.iter_mut().skip(1) {
        if rem > 0 {
            offset += 1;
            rem -= 1;
        }
        word.x += offset + space;
        offset += space;
    }
}

fn nudge_by(offset: u16, words: &mut [Text]) {
    for word in words.iter_mut() {
        word.x += offset;
    }
}

fn nudge(alignment: Alignment, space: u16, words: &mut [Text]) {
    match alignment {
        Alignment::Start => { }
        Alignment::Center => nudge_by(space / 2, words),
        Alignment::End => nudge_by(space, words),
        Alignment::Justify => { }
    }
}

fn align(alignment: Alignment, space: u16, words: &mut [Text]) {
    match alignment {
        // we lay out like that anyway
        Alignment::Start => { }
        Alignment::Justify => justify(space, words),
        _ => nudge(alignment, space, words),
    }
}
