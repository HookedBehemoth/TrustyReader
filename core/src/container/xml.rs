#[cfg(test)]
extern crate std;

use core::ops::Range;

use embedded_io::Error;

macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(test)]
        std::eprintln!($($arg)*);
    };
}

pub struct XmlParser<R> {
    reader: R,
    remaining: usize,
    buffer: alloc::vec::Vec<u8>,
    pos: usize,
    end: usize,
    at_start: bool,
    self_closing: Option<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum XmlEvent<'a> {
    Declaration {
        attrs: AttributeReader<'a>,
    },
    ProcessingInstruction {
        name: &'a str,
        attrs: AttributeReader<'a>,
    },
    Dtd {
        content: &'a str,
    },
    CDATA {
        data: &'a [u8],
    },
    Comment {
        content: &'a str,
    },
    StartElement {
        name: &'a str,
        attrs: AttributeReader<'a>,
    },
    Text {
        content: &'a str,
    },
    EndElement {
        name: &'a str,
    },
    EndOfFile,
}

#[derive(Debug)]
pub enum XmlError {
    IoError(embedded_io::ErrorKind),
    Utf8Error(core::str::Utf8Error),
    InvalidState,
    Eof,
}

type Result<T> = core::result::Result<T, XmlError>;

impl From<core::str::Utf8Error> for XmlError {
    fn from(err: core::str::Utf8Error) -> Self { XmlError::Utf8Error(err) }
}

impl<R: embedded_io::Read> XmlParser<R> {
    pub fn new(mut reader: R, total: usize, buffer_size: usize) -> Result<XmlParser<R>> {
        let mut buffer = alloc::vec![0; buffer_size];
        let end = reader
            .read(&mut buffer)
            .map_err(|e| XmlError::IoError(e.kind()))?;
        let remaining = total - end;
        Ok(XmlParser {
            reader,
            remaining,
            buffer,
            pos: 0,
            end,
            at_start: true,
            self_closing: None,
        })
    }

    pub fn next_event(&mut self) -> Result<XmlEvent<'_>> {
        // Ensure we have an XML declaration at the start of the document
        // We should probably ensure version 1.0 and UTF-8 encoding.
        if self.at_start {
            self.at_start = false;
            let (start, end) = self.try_find("<?xml", "?>")?;
            let block = core::str::from_utf8(&self.buffer[start..end])?;
            let attrs = AttributeReader::from_block(block);
            self.pos = end + 2;
            return Ok(XmlEvent::Declaration { attrs });
        };

        if self.pos == self.end && self.remaining == 0 {
            trace!("Pos = End");
            return Ok(XmlEvent::EndOfFile);
        }

        if let Some(range) = self.self_closing.take() {
            let block = &self.buffer[range].trim_ascii();
            let name = core::str::from_utf8(block)?
                .split_ascii_whitespace()
                .next()
                .ok_or(XmlError::InvalidState)?;
            return Ok(XmlEvent::EndElement { name });
        }

        let curr_end = match self.try_find_start("<") {
            Ok(pos) => pos,
            Err(XmlError::Eof) => return Ok(XmlEvent::EndOfFile),
            Err(e) => return Err(e),
        };

        let curr = self.buffer()[..curr_end].trim_ascii();
        if !curr.is_empty() {
            let block = self.buffer[self.pos..self.pos + curr_end].trim_ascii();
            let content = core::str::from_utf8(block)?;
            self.pos += curr_end;
            return Ok(XmlEvent::Text { content });
        }

        self.pos += curr_end;
        match self.ensure(3) {
            Ok(()) => {}
            Err(XmlError::Eof) => {
                return Ok(XmlEvent::EndOfFile);
            }
            Err(e) => return Err(e),
        };

        enum BlockType {
            CDATA,
            Comment,
            Dtd,
            PI,
            EndElement,
            StartElement,
        }

        let b = self.buffer();
        let (ty, n_start, n_end) = match (b[1], b[2]) {
            (b'!', b'[') => (BlockType::CDATA, "<![CDATA[", "]]>"),
            (b'!', b'-') => (BlockType::Comment, "<!--", "-->"),
            (b'!', _) => (BlockType::Dtd, "<!", ">"),
            (b'?', _) => (BlockType::PI, "<?", "?>"),
            (b'/', _) => (BlockType::EndElement, "</", ">"),
            (_, _) => (BlockType::StartElement, "<", ">"),
        };

        let (start, end) = self.try_find(n_start, n_end)?;

        let range = if matches!(ty, BlockType::StartElement) && self.buffer()[end - 1] == b'/' {
            let range = self.pos + start..self.pos + end - 1;
            self.self_closing = Some(range.clone());
            range
        } else {
            self.pos + start..self.pos + end
        };

        let block = &self.buffer[range].trim_ascii();

        let event = match ty {
            BlockType::CDATA => XmlEvent::CDATA { data: block },
            BlockType::Comment => XmlEvent::Comment {
                content: core::str::from_utf8(block)?,
            },
            BlockType::Dtd => XmlEvent::Dtd {
                content: core::str::from_utf8(block)?,
            },
            BlockType::PI => {
                let (name, attrs) = Self::name_and_attrs(block)?;
                XmlEvent::ProcessingInstruction { name, attrs }
            }
            BlockType::EndElement => XmlEvent::EndElement {
                name: core::str::from_utf8(block)?,
            },
            BlockType::StartElement => {
                let (name, attrs) = Self::name_and_attrs(block)?;
                XmlEvent::StartElement { name, attrs }
            }
        };
        self.pos += end + n_end.len();
        Ok(event)
    }
    pub fn name_and_attrs(block: &[u8]) -> Result<(&str, AttributeReader<'_>)> {
        let block = core::str::from_utf8(block)?;
        let mut split = block.split_ascii_whitespace();
        let name = split.next().unwrap_or("");
        Ok((name, AttributeReader::from_split(split)))
    }

    /// Moves the unparsed characters starting from offset to the beginning
    /// of the buffer, updates positional indices and reads more data.
    fn advance(&mut self, offset: usize) -> Result<()> {
        trace!(
            "Advancing by {offset} bytes (remaining: {})",
            self.remaining
        );
        assert!(offset <= self.end);
        assert!(offset <= self.buffer.len());
        if self.remaining == 0 {
            return Ok(());
        }
        trace!("Copying {} bytes to start of buffer", self.end - offset);
        for i in offset..self.end {
            self.buffer[i - offset] = self.buffer[i];
        }
        self.pos = 0;
        self.end -= offset;
        let data_start = self.buffer.len() - offset;
        let read_bytes = self
            .reader
            .read(&mut self.buffer[data_start..])
            .map_err(|e| XmlError::IoError(e.kind()))?;
        self.end += read_bytes;
        self.remaining -= read_bytes;
        trace!(
            "Read {read_bytes} bytes, new buffer len: {}, remaining: {}",
            self.buffer().len(),
            self.remaining
        );
        Ok(())
    }

    /// Ensure at least `size` bytes are available in the buffer, advancing if necessary.
    fn ensure(&mut self, size: usize) -> Result<()> {
        trace!("Ensuring {size} bytes (remaining: {})", self.remaining);
        let available = self.buffer().len();
        if available >= size {
            return Ok(());
        }
        if available + self.remaining < size {
            return Err(XmlError::Eof);
        }
        self.advance(self.pos)
    }

    /// Tries to find start & end needles in the buffer.
    /// If we find the start needle but not the end, we advance to have the start at 0 and try again - once.
    /// If we find neither, we advance to the end of the buffer and try again - once.
    fn try_find(&mut self, n_start: &str, n_end: &str) -> Result<(usize, usize)> {
        trace!(
            "Trying to find '{n_start}' and '{n_end}' (remaining: {})",
            self.remaining
        );
        let n_start = n_start.as_bytes();
        let n_end = n_end.as_bytes();
        match find_span(self.buffer(), n_start, n_end) {
            Some((start, Some(end))) => Ok((start, end)),
            Some((start, None)) => {
                self.advance(self.pos + start)?;
                let end = memchr::memmem::find(self.buffer(), n_end).ok_or(XmlError::Eof)?;
                Ok((0, end))
            }
            None => {
                self.advance(self.buffer.len())?;
                let Some((start, Some(end))) = find_span(self.buffer(), n_start, n_end) else {
                    return Err(XmlError::Eof);
                };
                Ok((start, end))
            }
        }
    }

    /// Tries to find the start needle in the buffer.
    /// If it is not found, we advance to the end of the buffer and try again - once.
    fn try_find_start(&mut self, n_start: &str) -> Result<usize> {
        trace!(
            "Trying to find start '{n_start}' (pos: {}, remaining: {})",
            self.pos, self.remaining
        );
        let n_start = n_start.as_bytes();
        match memchr::memmem::find(self.buffer(), n_start) {
            Some(pos) => Ok(pos),
            None => {
                self.advance(self.pos)?;
                let Some(pos) = memchr::memmem::find(self.buffer(), n_start) else {
                    trace!("Needle not found!");
                    return Err(XmlError::Eof);
                };
                Ok(pos)
            }
        }
    }

    fn buffer(&self) -> &[u8] { &self.buffer[self.pos..self.end] }
}

#[derive(Clone)]
pub struct AttributeReader<'a> {
    split: core::str::SplitAsciiWhitespace<'a>,
}

impl core::fmt::Debug for AttributeReader<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut builder = f.debug_map();
        for (n, v) in self.clone() {
            builder.entry(&n, &v);
        }
        builder.finish()
    }
}

impl Default for AttributeReader<'_> {
    fn default() -> Self {
        AttributeReader {
            split: "".split_ascii_whitespace(),
        }
    }
}

impl PartialEq for AttributeReader<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.clone()
            .zip(other.clone())
            .all(|((n1, v1), (n2, v2))| n1.eq_ignore_ascii_case(n2) && v1 == v2)
    }
    fn ne(&self, other: &Self) -> bool { !self.eq(other) }
}

impl<'a> AttributeReader<'a> {
    pub fn from_block(buffer: &str) -> AttributeReader<'_> {
        AttributeReader {
            split: buffer.trim_ascii().split_ascii_whitespace(),
        }
    }

    pub fn from_split(split: core::str::SplitAsciiWhitespace<'_>) -> AttributeReader<'_> {
        AttributeReader { split }
    }

    /// Case-insensitive
    /// Careful: mutates internal iterator
    pub fn get(&mut self, name: &str) -> Option<&str> {
        for (n, v) in self {
            if n.eq_ignore_ascii_case(name) {
                return Some(v);
            }
        }
        None
    }
}

impl<'a> Iterator for AttributeReader<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let part = self.split.next()?;
        let mut iter = part.splitn(2, '=');
        let name = iter.next()?;
        let value = iter.next()?.trim_matches('"').trim_matches('\'');
        Some((name, value))
    }
}

fn find_span(buffer: &[u8], start: &[u8], end: &[u8]) -> Option<(usize, Option<usize>)> {
    let start = memchr::memmem::find(buffer, start)? + start.len();
    let end = memchr::memmem::find(&buffer[start..], end).map(|pos| pos + start);
    Some((start, end))
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    extern crate std;

    use super::*;
    use alloc::string::String;

    fn walk(xml: &str) {
        let mut bytes = xml.as_bytes();
        let mut parser = XmlParser::new(&mut bytes, xml.len(), 2048).unwrap();
        let mut element_stack = heapless::Vec::<String, 10>::new();
        loop {
            match parser.next_event().unwrap() {
                XmlEvent::Declaration { mut attrs } => {
                    trace!("--Declaration");
                    assert_eq!(attrs.get("version"), Some("1.0"));
                    assert!(
                        attrs
                            .get("encoding")
                            .map(|v| v.to_ascii_lowercase() == "utf-8")
                            == Some(true)
                    );
                }
                XmlEvent::EndOfFile { .. } => {
                    trace!("--End of file");
                    break;
                }
                XmlEvent::StartElement { name, .. } => {
                    trace!("--Start element: {}", name);
                    element_stack.push(String::from(name)).unwrap();
                }
                XmlEvent::Text { content } => {
                    trace!("--Text: {}", content);
                }
                XmlEvent::EndElement { name } => {
                    trace!("--End element: {}", name);
                    let expected = element_stack.pop().unwrap();
                    assert_eq!(name, expected);
                }
                _ => {}
            }
        }
        for rem in &element_stack {
            trace!("Unclosed element: {}", rem);
        }
        assert!(
            element_stack.is_empty(),
            "Element stack should be empty at end of document"
        );
    }

    #[test]
    fn test_walk_toc() {
        let xml = include_str!("test_data/toc.ncx");
        walk(xml);
        let xml = include_str!("test_data/ellc12_toc.ncx");
        walk(xml);
        let xml = include_str!("test_data/ellc13_toc.ncx");
        walk(xml);
    }

    #[test]
    fn test_walk_opf() {
        let xml = include_str!("test_data/content.opf");
        walk(xml);
        let xml = include_str!("test_data/ellc12_content.opf");
        walk(xml);
    }

    #[test]
    fn test_walk_container() {
        let xml = include_str!("test_data/container.xml");
        walk(xml);
    }

    #[test]
    fn test_walk_content() {
        let xml = include_str!("test_data/content-2.xhtml");
        walk(xml);
        let xml = include_str!("test_data/content-7.xhtml");
        walk(xml);
        let xml = include_str!("test_data/titlepage.xhtml");
        walk(xml);
    }

    const LOREM: &'static str = "\
        Lorem ipsum dolor sit amet, consetetur sadipscing elitr,seddiam \
        nonumy eirmod tempor invidunt ut labore et dolore magna aliquya \
        erat, sed diam voluptua. At vero eos et accusam et justo duo do \
        ores et ea rebum. Stet clita kasd gubergren, no sea takimata sa \
        ctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet,\
        consetetur sadipscing elitr, sed diam nonumy eirmod tempor invid\
        unt ut labore et dolore magna aliquyam erat, sed diam voluptua. \
        At vero eos et accusam et justo duo dolores et ea rebum. Stet cl";

    #[test]
    fn test_window() {
        let data = LOREM.as_bytes();
        let mut buffer = data;
        let mut parser = XmlParser::new(&mut buffer, data.len(), 256).unwrap();
        assert_eq!(parser.buffer(), &data[..256]);
        parser.advance(256).unwrap();
        assert_eq!(parser.buffer(), &data[256..]);
    }

    #[test]
    fn test_needle_range() {
        let xml = "\
            <root>\
                <child>Text</child>\
                <child>More text</child>\
            </root>";
        let data = xml.as_bytes();

        let Some((start, Some(end))) = find_span(data, b"<", b">") else {
            panic!("Failed to find span");
        };
        assert_eq!(&xml[start..end], "root");

        let Some((start, Some(end))) = find_span(data, b"<child>", b"</child>") else {
            panic!("Failed to find span");
        };
        assert_eq!(&xml[start..end], "Text");
    }

    #[test]
    fn test_find() {
        fn find_str<'a>(
            parser: &'a mut XmlParser<&'_ [u8]>,
            n_start: &str,
            n_end: &str,
        ) -> Result<&'a str> {
            let (start, end) = parser.try_find(n_start, n_end)?;
            Ok(core::str::from_utf8(&parser.buffer[start..end])?)
        }

        let data = LOREM.as_bytes();
        let buffer = data;
        let mut parser = XmlParser::new(buffer, data.len(), 256).unwrap();
        let ipsum = find_str(&mut parser, "Lorem ", " dolor").unwrap();
        assert_eq!(ipsum, "ipsum");
        let aliquyam = find_str(&mut parser, "no sea takimata ", " ctus est").unwrap();
        assert_eq!(aliquyam, "sa");
        assert_eq!(parser.buffer(), &data[253..509]);
    }

    #[test]
    fn test_full() {
        use XmlEvent::*;
        use core::assert_matches;

        let xml = "\
            <?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
            <root attr1=\"value1\" attr2=\"value2\">\
                <child>Text</child>\
                <child>More text</child>\
                <child>  \tText with whitespace \r\n  </child>\
                <self-closing />\
                <self-closing/>\
            </root>";
        let mut data = xml.as_bytes();
        let mut parser = XmlParser::new(&mut data, xml.len(), 512).unwrap();
        let Declaration { mut attrs } = parser.next_event().unwrap() else {
            panic!("Expected declaration");
        };
        assert_eq!(attrs.get("version"), Some("1.0"));
        assert_eq!(attrs.get("encoding"), Some("UTF-8"));
        assert_eq!(attrs.get("standalone"), Some("yes"));
        let StartElement { name: "root", mut attrs } = parser.next_event().unwrap() else {
            panic!("Expected start element");
        };
        assert_eq!(attrs.get("attr1"), Some("value1"));
        assert_eq!(attrs.get("attr2"), Some("value2"));
        assert_matches!(parser.next_event(), Ok(StartElement { name: "child", .. }) );
        assert_matches!(parser.next_event(), Ok(Text { content: "Text" }));
        assert_matches!(parser.next_event(), Ok(EndElement { name: "child" }));
        assert_matches!(parser.next_event(), Ok(StartElement { name: "child", .. }) );
        assert_matches!(parser.next_event(), Ok(Text { content: "More text" }));
        assert_matches!(parser.next_event(), Ok(EndElement { name: "child" }));
        assert_matches!(parser.next_event(), Ok(StartElement { name: "child", .. }) );
        assert_matches!(parser.next_event(), Ok(Text { content: "Text with whitespace" }));
        assert_matches!(parser.next_event(), Ok(EndElement { name: "child" }));
        assert_matches!(parser.next_event(), Ok(StartElement { name: "self-closing", .. }) );
        assert_matches!(parser.next_event(), Ok(EndElement { name: "self-closing" }) );
        assert_matches!(parser.next_event(), Ok(StartElement { name: "self-closing", .. }) );
        assert_matches!(parser.next_event(), Ok(EndElement { name: "self-closing" }) );
        assert_matches!(parser.next_event(), Ok(EndElement { name: "root" }));
        assert_matches!(parser.next_event(), Ok(EndOfFile));
    }
}
