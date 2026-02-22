use alloc::string::ToString;

use crate::{container::book, layout, res::font};

pub fn from_str(text: &str) -> book::Chapter {
    let mut paragraphs = alloc::vec![];

    for text in text.split("\n\n") {
        let mut runs = alloc::vec![];

        for mut line in text.lines().map(str::trim) {
            let style = if line.starts_with('#') {
                line = line.trim_start_matches('#').trim();
                font::FontStyle::Bold
            } else {
                font::FontStyle::Regular
            };
            runs.push(layout::Run {
                text: line.to_string(),
                style,
                breaking: true,
            })
        }

        paragraphs.push(book::Paragraph {
            runs,
            alignment: Some(layout::Alignment::Start),
            indent: Some(0),
        });
    }

    book::Chapter { title: None, paragraphs }
}
