use alloc::string::ToString;

use crate::{container::book, layout, res::font};

pub fn from_str(text: &str) -> book::Chapter {
    let paragraphs = text
        .split("\n\n")
        .map(|p| book::Paragraph {
            runs: p
                .split("\n")
                .map(|line| layout::Run {
                    text: line.to_string(),
                    style: font::FontStyle::Regular,
                    breaking: true,
                })
                .collect(),
            alignment: None,
            indent: None,
        })
        .collect();
    book::Chapter { title: None, paragraphs }
}
