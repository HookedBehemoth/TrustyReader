use alloc::vec::Vec;

use crate::res::font::FontDefinition;

#[derive(Clone, Copy)]
pub struct Options<'a> {
    pub width: u16,
    pub alignment: Alignment,
    pub justify: bool,
    pub language: hypher::Lang,
    pub font: &'a FontDefinition<'a>,
    space_width: u16,
    dash_width: u16,
}

impl<'a> Options<'a> {
    pub fn new(
        width: u16,
        alignment: Alignment,
        justify: bool,
        language: hypher::Lang,
        font: &'a FontDefinition<'a>,
    ) -> Self {
        let space_width = font.char_width(' ').unwrap() as u16;
        let dash_width = font.char_width('-').unwrap() as u16;
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

pub struct Word<'a> {
    pub text: &'a str,
    pub x: u16,
}

pub struct Line<'a> {
    pub words: Vec<Word<'a>>,
    pub y: u16,
    pub hyphenated: bool,
}

pub fn layout_text<'a>(options: Options<'a>, text: &'a str) -> Vec<Line<'a>> {
    let y_advance = options.font.y_advance as u16;

    let mut x = 0;
    let mut y = y_advance;
    let mut lines: Vec<Line> = Vec::new();
    let mut current_line = Line {
        words: Vec::new(),
        y,
        hyphenated: false,
    };

    let mut words = text.split_whitespace();
    while let Some(mut word) = words.next() {
        let mut word_width = options.font.word_width(word);

        // add space before the word
        if !current_line.words.is_empty() {
            x += options.space_width;
        }

        // advance to the next line
        if x + word_width > options.width {
            if let Some((remaining, remaining_width)) =
                hyphenate(x, word, &mut current_line, options)
            {
                word = remaining;
                word_width = options.font.word_width(word);
                x = options.width - remaining_width;
            }

            if options.justify {
                let space = options.width.saturating_sub(x);
                justify(space, &mut current_line.words);
            }
            lines.push(current_line);
            x = 0;
            y += y_advance;
            current_line = Line {
                words: Vec::new(),
                y,
                hyphenated: false,
            };
        }

        // Add word to current line
        current_line.words.push(Word { text: word, x });
        x += word_width;
    }

    lines
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

    let mut length = 0;
    for part in hypher::hyphenate(word, options.language) {
        let part_width = options.font.word_width(part);
        if part_width > space {
            if length == 0 {
                return None;
            }

            current_line.hyphenated = true;
            current_line.words.push(Word {
                text: &word[0..length],
                x,
            });
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
