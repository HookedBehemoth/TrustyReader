use alloc::vec::Vec;
use log::trace;

use crate::res::font;

#[derive(Clone, Copy)]
pub struct Options {
    pub width: u16,
    pub alignment: Alignment,
    pub justify: bool,
    pub language: hypher::Lang,
    pub font: font::Font,
    // split by type?
    space_width: u16,
    dash_width: u16,
}

impl Options {
    pub fn new(
        width: u16,
        alignment: Alignment,
        justify: bool,
        language: hypher::Lang,
        font: font::Font,
    ) -> Self {
        let font_def = font.definition(font::FontStyle::Regular);
        let space_width = font_def.char_width(' ').unwrap() as u16;
        let dash_width = font_def.char_width('-').unwrap() as u16;
        Self {
            width,
            alignment,
            justify,
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
}
impl Alignment {
    pub fn repr(self) -> &'static str {
        match self {
            Alignment::Start => "Start",
            Alignment::Center => "Center",
            Alignment::End => "End",
        }
    }
}

pub struct Word<'a> {
    pub text: &'a str,
    pub x: u16,
}

pub struct Line<'a> {
    pub words: Vec<Word<'a>>,
    pub hyphenated: bool,
}

pub fn layout_text<'a>(options: Options, text: &'a str) -> Vec<Line<'a>> {
    let mut x = 0;
    let mut lines: Vec<Line> = Vec::new();
    let mut current_line = Line {
        words: Vec::new(),
        hyphenated: false,
    };
    let font = options.font.definition(font::FontStyle::Regular);
    trace!("Width: {}", options.width);

    let words = text.split_whitespace();
    for mut word in words {
        let mut word_width = font.word_width(word);

        // add space before the word
        if !current_line.words.is_empty() {
            x += options.space_width;
        }

        // advance to the next line
        if x + word_width >= options.width {
            if let Some((remaining, remaining_width)) =
                hyphenate(x, word, &mut current_line, options)
            {
                word = remaining;
                word_width = font.word_width(word);
                x = options.width - remaining_width;
            }

            let space = options.width.saturating_sub(x);
            if options.justify {
                justify(space, &mut current_line.words);
            } else {
                nudge(options.alignment, space, &mut current_line.words);
            }
            lines.push(current_line);
            x = 0;
            current_line = Line {
                words: Vec::new(),
                hyphenated: false,
            };
        }

        trace!("Word: '{}', width: {}, x: {}", word, word_width, x);

        // Add word to current line
        current_line.words.push(Word { text: word, x });
        x += word_width;
    }

    if !current_line.words.is_empty() {
        let space = options.width.saturating_sub(x);
        nudge(options.alignment, space, &mut current_line.words);
        
        lines.push(current_line);
    }

    lines
}

fn nudge(alignment: Alignment, space: u16, words: &mut [Word]) {
    let offset = match alignment {
        Alignment::Start => 0,
        Alignment::Center => space / 2,
        Alignment::End => space,
    };
    for word in words.iter_mut() {
        word.x += offset;
    }
}

/// Greedily hyphenate the given word to fit in the remaining space.
fn hyphenate<'a>(
    x: u16,
    word: &'a str,
    current_line: &mut Line<'a>,
    options: Options,
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
    
    let font = options.font.definition(font::FontStyle::Regular);
    let mut length = 0;
    for part in hypher::hyphenate(word, options.language) {
        let part_width = font.word_width(part);
        if part_width > space {
            if length == 0 {
                return None;
            }

            let text = &word[0..length];
            let text = if text.chars().last() != Some('-') {
                text
            } else {
                &text[0..text.len() - 1]
            };
            current_line.hyphenated = true;
            current_line.words.push(Word { text, x });
            return Some((&word[length..], space));
        }

        length += part.len();
        space -= part_width;
    }

    None
}

/// Justify the words in the current line by splitting
/// the remaining space evenly between the words.
fn justify(room: u16, words: &mut [Word]) {
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
