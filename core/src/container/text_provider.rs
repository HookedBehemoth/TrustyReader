use core::str::Split;

use alloc::string::String;

trait TextProvider {
    // type Progress: Progress;
    fn next_paragraph(&mut self) -> Option<Paragraph<'_>>;
    // fn progress(&self) -> Self::Progress;
}

// should this be a trait that returns runs
struct Paragraph<'a> {
    text: &'a str,
}

// struct Run<'a> {
//     text: &'a str,
//     bold: bool,
//     italic: bool,
// }

// trait Progress {
//     fn percentage(&self) -> u8;
//     fn serialize(&self) -> String;
// }

struct PlainTextProvider<'a> {
    text: Split<'a, char>,
}

impl<'a> PlainTextProvider<'a> {
    fn new(text: &'a str) -> Self {
        Self { text: text.split('\n') }
    }
}

// struct PlainTextProgress {
//     percentage: u8,
// }

impl TextProvider for PlainTextProvider<'_> {
    // type Progress = PlainTextProgress;

    fn next_paragraph(&mut self) -> Option<Paragraph<'_>> {
        self.text.next().map(|text| Paragraph { text })
    }
}

struct MarkdownTextProvider<'a> {
    text: Split<'a, char>,
}

impl<'a> MarkdownTextProvider<'a> {
    fn new(text: &'a str) -> Self {
        Self { text: text.split('\n') }
    }
}

impl TextProvider for MarkdownTextProvider<'_> {
    // type Progress = PlainTextProgress;

    fn next_paragraph(&mut self) -> Option<Paragraph<'_>> {
        let text = self.text.next()?;
        // TODO
        Some(Paragraph { text })
    }
}
