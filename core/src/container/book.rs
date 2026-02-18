use alloc::{
    string::{String, ToString},
    vec::Vec,
};

pub struct Book {
    pub title: String,
    // TODO: should be a function to lazily load chapter
    pub chapters: Vec<Chapter>,
}

pub struct Chapter {
    pub title: Option<String>,
    // TODO: we'd need a custom file format if we want to allow arbitrary seeking
    // Keep it like this for now? We have roughly 200KB free rn and an extra 48kB
    // if we reuse the framebuffer here. 
    pub paragraphs: Vec<Paragraph>,
}

pub struct Paragraph {
    pub text: String,
}

impl Book {
    pub fn from_plaintext(title: String, text: String) -> Self {
        let paragraphs = text
            .split("\n\n")
            .map(|p| Paragraph { text: p.to_string() })
            .collect();
        Book {
            title,
            chapters: alloc::vec![Chapter { title: None, paragraphs }],
        }
    }

    pub fn from_markdown() -> Self {
        unimplemented!()
    }
}
