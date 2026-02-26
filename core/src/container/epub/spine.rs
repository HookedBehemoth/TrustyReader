use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use log::trace;

use crate::{
    container::{
        book::{Chapter, Paragraph},
        css,
    },
    layout,
    res::font,
};
use embedded_xml as xml;

pub fn parse<R: embedded_io::Read>(
    title: Option<String>,
    reader: R,
    size: usize,
    extern_stylesheet: Option<&css::Stylesheet>,
) -> super::Result<Chapter> {
    // TODO: Ensure this is XHTML here or while parsing?
    let mut parser = xml::Reader::new(reader, size as _, 8096)?;

    let mut paragraphs = alloc::vec![];
    let mut inline_stylesheet = css::Stylesheet::new();

    loop {
        let event = parser.next_event()?;
        trace!("XML event: {:?}", event);
        match event {
            xml::Event::StartElement { name: "head", .. } => {
                inline_stylesheet = parse_head(&mut parser)?;
            }
            xml::Event::StartElement { name: "body", .. } => {
                paragraphs = parse_body(&mut parser, inline_stylesheet, extern_stylesheet)?;
                break;
            }
            xml::Event::EndOfFile => break,
            _ => {}
        }
    }

    Ok(Chapter { title, paragraphs })
}

fn parse_head<R: embedded_io::Read>(
    reader: &mut xml::OwnedReader<R>,
) -> super::Result<css::Stylesheet> {
    let mut stylesheet = css::Stylesheet::new();

    loop {
        let event = reader.next_event()?;
        trace!("XML event: {:?}", event);
        match event {
            xml::Event::EndElement { name: "head" } => break,
            xml::Event::StartElement { name: "style", attrs } => {
                if attrs.get("type") != Some("text/css") {
                    continue;
                }
                let xml::Event::Text { content } = reader.next_event()? else {
                    continue;
                };
                stylesheet.extend_from_sheet(&content);
            }
            xml::Event::EndOfFile => break,
            _ => {}
        }
    }

    Ok(stylesheet)
}

fn parse_body<R: embedded_io::Read>(
    reader: &mut xml::OwnedReader<R>,
    inline_stylesheet: css::Stylesheet,
    extern_stylesheet: Option<&css::Stylesheet>,
) -> super::Result<Vec<Paragraph>> {
    let mut parser = BodyParser::new();

    fn is_block_element(name: &str) -> bool {
        matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "li")
    }
    fn is_italic(name: &str) -> bool {
        matches!(name, "i" | "em")
    }
    fn is_bold(name: &str) -> bool {
        matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "b")
    }
    fn is_breaking(name: &str) -> bool {
        matches!(name, "br" | "tr")
    }

    loop {
        let event = reader.next_event()?;
        trace!("XML event: {:?}", event);
        match event {
            xml::Event::EndElement { name: "body" } => break,
            xml::Event::StartElement { name, attrs } => {
                if is_block_element(name) {
                    parser.flush_run();
                }

                parser.increase_depth();

                let id = attrs.get("id");
                let class = attrs.get("class");
                let inline_style = attrs
                    .get("style")
                    .map(css::Rule::from_str)
                    .unwrap_or_default();
                let style = inline_style
                    + inline_stylesheet.get(name, id, class)
                    + extern_stylesheet
                        .map(|s| s.get(name, id, class))
                        .unwrap_or_default();

                if is_bold(name) {
                    parser.set_bold(true);
                } else if is_italic(name) {
                    parser.set_italic(true);
                } else if is_breaking(name) {
                    parser.break_line();
                }

                if let Some(italic) = style.italic {
                    parser.set_italic(italic);
                    parser.italic_depth = Some(parser.depth);
                }
                if let Some(bold) = style.bold {
                    parser.set_bold(bold);
                    parser.bold_depth = Some(parser.depth);
                }
                if let Some(alignment) = style.alignment {
                    parser.alignment = Some(alignment);
                }
                if let Some(indent) = style.indent {
                    parser.indent = Some(indent);
                }
            }
            xml::Event::EndElement { name } => {
                if is_block_element(name) {
                    parser.flush_run();
                }

                if parser.bold_depth == None && is_bold(name) {
                    parser.set_bold(false);
                } else if parser.italic_depth == None && is_italic(name) {
                    parser.set_italic(false);
                }

                parser.decrease_depth();
            }
            xml::Event::Text { content } => {
                parser.push_text(content);
            }
            xml::Event::EndOfFile => break,
            _ => {}
        }
    }

    Ok(parser.into_paragraphs())
}

struct BodyParser {
    paragraphs: Vec<Paragraph>,
    runs: Vec<layout::Run>,
    alignment: Option<layout::Alignment>,
    indent: Option<u16>,
    current_run: String,
    bold: bool,
    italic: bool,
    depth: u8,
    has_trailing_space: bool,
    italic_depth: Option<u8>,
    bold_depth: Option<u8>,
}

impl BodyParser {
    fn new() -> Self {
        Self {
            paragraphs: Vec::new(),
            runs: Vec::new(),
            alignment: None,
            indent: None,
            current_run: String::new(),
            bold: false,
            italic: false,
            depth: 0,
            has_trailing_space: false,
            italic_depth: None,
            bold_depth: None,
        }
    }

    fn set_bold(&mut self, bold: bool) {
        if self.bold != bold {
            self.flush_text(false);
            self.bold = bold;
        }
    }

    fn set_italic(&mut self, italic: bool) {
        if self.italic != italic {
            self.flush_text(false);
            self.italic = italic;
        }
    }

    fn style(&self) -> font::FontStyle {
        match (self.bold, self.italic) {
            (false, false) => font::FontStyle::Regular,
            (true, false) => font::FontStyle::Bold,
            (false, true) => font::FontStyle::Italic,
            (true, true) => font::FontStyle::BoldItalic,
        }
    }

    fn flush_text(&mut self, breaking: bool) {
        if !self.current_run.is_empty() {
            let text = core::mem::take(&mut self.current_run);
            self.runs.push(layout::Run {
                text,
                style: self.style(),
                breaking,
            });
        }
    }

    fn break_line(&mut self) {
        self.flush_text(true);
    }

    fn flush_run(&mut self) {
        self.flush_text(false);
        if !self.runs.is_empty() {
            let mut runs = core::mem::take(&mut self.runs);
            // trim whitespace off the end of the last run
            runs.last_mut().map(|run| {
                run.text = run.text.trim_ascii_end().to_string();
            });
            self.paragraphs.push(Paragraph {
                runs,
                indent: self.indent,
                alignment: self.alignment,
            });
            self.indent = None;
            self.alignment = None;
        }
    }

    fn into_paragraphs(mut self) -> Vec<Paragraph> {
        self.flush_run();
        self.paragraphs
    }

    fn push_text(&mut self, text: &str) {
        let text = if self.runs.is_empty() && self.current_run.is_empty() {
            text.trim_ascii_start()
        } else {
            text
        };
        if !self.has_trailing_space && text.starts_with(char::is_whitespace) {
            self.current_run.push(' ');
        }
        self.has_trailing_space = text.ends_with(char::is_whitespace);
        let text = text
            .split_whitespace()
            .fold(String::new(), |mut acc, word| {
                if !acc.is_empty() {
                    acc.push(' ');
                }
                acc.push_str(html_escape::decode_html_entities(word).as_ref());
                acc
            });
        self.current_run.push_str(&text);
        if self.has_trailing_space {
            self.current_run.push(' ');
        }
    }

    fn increase_depth(&mut self) {
        self.depth += 1;
    }

    fn decrease_depth(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
        }
        if let Some(italic_depth) = self.italic_depth {
            if self.depth < italic_depth {
                self.set_italic(false);
                self.italic_depth = None;
            }
        }
        if let Some(bold_depth) = self.bold_depth {
            if self.depth < bold_depth {
                self.set_bold(false);
                self.bold_depth = None;
            }
        }
    }
}

#[rustfmt::skip]
#[cfg(test)]
mod test {
    use alloc::string::ToString;

    use crate::{layout::Run, res::font::FontStyle};

    #[test]
    fn inline_styles() {
        let body = r#"
        <?xml version="1.0" encoding="utf-8"?>
        <html xmlns="http://www.w3.org/1999/xhtml">
            <body>
                <p>Text with <i>Inline</i> styles <b>bold</b>, <em>emphasized</em> or <i>italic</i></p>
            </body>
        </html>"#;
        let chapter = super::parse(None, body.as_bytes(), body.len(), None).unwrap();
        assert_eq!(chapter.paragraphs.len(), 1);
        let mut runs = chapter.paragraphs[0].runs.iter();
        assert_eq!(runs.next().unwrap(), &Run { text: "Text with ".to_string(), style: FontStyle::Regular, breaking: false });
        assert_eq!(runs.next().unwrap(), &Run { text: "Inline".to_string(), style: FontStyle::Italic, breaking: false });
        assert_eq!(runs.next().unwrap(), &Run { text: " styles ".to_string(), style: FontStyle::Regular, breaking: false });
        assert_eq!(runs.next().unwrap(), &Run { text: "bold".to_string(), style: FontStyle::Bold, breaking: false });
        assert_eq!(runs.next().unwrap(), &Run { text: ", ".to_string(), style: FontStyle::Regular, breaking: false });
        assert_eq!(runs.next().unwrap(), &Run { text: "emphasized".to_string(), style: FontStyle::Italic, breaking: false });
        assert_eq!(runs.next().unwrap(), &Run { text: " or ".to_string(), style: FontStyle::Regular, breaking: false });
        assert_eq!(runs.next().unwrap(), &Run { text: "italic".to_string(), style: FontStyle::Italic, breaking: false });
        assert!(runs.next().is_none());
    }

    // https://patrickbrosset.medium.com/when-does-white-space-matter-in-html-b90e8a7cdd33
    #[test]
    fn test_whitespace() {
        let body = r#"
        <?xml version="1.0" encoding="utf-8"?>
        <html xmlns="http://www.w3.org/1999/xhtml">
            <body>
                <p> Text
                    
                 with <span> White </span> space<span> before</span> and <span>after</span>Spans
                 
                </p>
            </body>
        </html>"#;
        let chapter = super::parse(None, body.as_bytes(), body.len(), None).unwrap();
        assert_eq!(chapter.paragraphs.len(), 1);
        let paragraph = &chapter.paragraphs[0];
        assert_eq!(paragraph.runs.len(), 1);
        let run = &paragraph.runs[0];
        assert_eq!(run.text, "Text with White space before and afterSpans");
    }

    #[test]
    fn test_amp_escape() {
        let body = r#"
        <?xml version="1.0" encoding="utf-8"?>
        <html xmlns="http://www.w3.org/1999/xhtml">
            <body>
                <p>We support &quot;&amp;amp;&quot; escaping now!!!</p>
            </body>
        </html>"#;
        let chapter = super::parse(None, body.as_bytes(), body.len(), None).unwrap();
        assert_eq!(chapter.paragraphs.len(), 1);
        let paragraph = &chapter.paragraphs[0];
        assert_eq!(paragraph.runs.len(), 1);
        let run = &paragraph.runs[0];
        assert_eq!(run.text, "We support \"&amp;\" escaping now!!!");
    }
}
