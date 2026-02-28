use embedded_io::Error;

use crate::Result;
use crate::attributes::AttributeReader;
use crate::events::Event;

use core::ops::Range;

#[cfg(test)]
extern crate std;

macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "log")]
        log::trace!($($arg)*);
        #[cfg(test)]
        std::eprintln!($($arg)*);
    };
}

/// A streaming XML reader.
/// The temporary buffer can be owned or borrowed
pub struct Reader<R, Buffer> {
    reader: R,
    remaining: usize,
    buffer: Buffer,
    pos: usize,
    end: usize,
    self_closing: Option<Range<usize>>,
}

impl<'a, R: embedded_io::Read> Reader<R, &'a mut [u8]> {
    /// Creates a new Reader with a borrowed buffer.
    /// ```
    /// # use embedded_xml as xml;
    /// # fn main() -> Result<(), xml::Error> {
    /// # let xml = "<?xml version=\"1.0\"?>";
    /// # let mut reader = xml.as_bytes();
    /// let mut buffer = [0u8; 256];
    /// let mut parser = xml::Reader::new_borrowed(&mut reader, xml.len(), &mut buffer)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_borrowed(reader: R, total_size: usize, buffer: &'a mut [u8]) -> Result<Self> {
        Self::new_with_read(reader, total_size, buffer)
    }
}

#[cfg(feature = "alloc")]
impl<R: embedded_io::Read> Reader<R, alloc::vec::Vec<u8>> {
    /// Creates a new Reader with an owned buffer of size `buffer_size`.
    /// ```
    /// # use embedded_xml as xml;
    /// # fn main() -> Result<(), xml::Error> {
    /// # let xml = "<?xml version=\"1.0\"?>";
    /// # let mut reader = xml.as_bytes();
    /// let mut parser = xml::Reader::new(&mut reader, xml.len(), 256)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(reader: R, total_size: usize, buffer_size: usize) -> Result<Self> {
        let buffer = alloc::vec![0; buffer_size];
        Self::new_with_read(reader, total_size, buffer)
    }
}

impl<R: embedded_io::Read, Buffer: AsRef<[u8]> + AsMut<[u8]>> Reader<R, Buffer> {
    fn new_with_read(mut reader: R, total_size: usize, mut buffer: Buffer) -> Result<Self> {
        let end = reader
            .read(buffer.as_mut())
            .map_err(|e| crate::Error::IoError(e.kind()))?;
        let remaining = total_size - end;
        Ok(Reader {
            reader,
            remaining,
            buffer,
            pos: 0,
            end,
            self_closing: None,
        })
    }

    /// Advances the reader to the next event and returns it.
    ///
    /// # Examples
    /// ```
    /// # use embedded_xml as xml;
    /// # fn main() -> Result<(), xml::Error> {
    /// # let xml = "<?xml version=\"1.0\"?>";
    /// # let mut reader = xml.as_bytes();
    /// # let mut buffer = [0u8; 256];
    /// # let mut reader = xml::Reader::new_borrowed(&mut reader, xml.len(), &mut buffer)?;
    /// loop {
    ///     match reader.next_event()? {
    ///         xml::Event::ProcessingInstruction { name: "xml", mut attrs } => {
    ///             assert_eq!(attrs.get("version"), Some("1.0"));
    ///         }
    ///         xml::Event::StartElement { name: "item", mut attrs } => {
    ///             for (name, value) in attrs {
    ///                println!("Attribute: {} = {}", name, value);
    ///             }
    ///         }
    ///         xml::Event::EndElement { name } => {
    ///             println!("End element: {}", name);
    ///         }
    ///         xml::Event::EndOfFile => break,
    ///         _ => {}
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn next_event(&mut self) -> Result<Event<'_>> {
        if self.pos == self.end && self.remaining == 0 {
            trace!("Pos = End");
            return Ok(Event::EndOfFile);
        }

        if let Some(range) = self.self_closing.take() {
            let block = &self.buffer.as_ref()[range].trim_ascii();
            let name = core::str::from_utf8(block)?
                .split_ascii_whitespace()
                .next()
                .ok_or(crate::Error::InvalidState)?;
            return Ok(Event::EndElement { name });
        }

        let curr_end = match self.try_find_start("<") {
            Ok(pos) => pos,
            Err(crate::Error::Eof) => return Ok(Event::EndOfFile),
            Err(e) => return Err(e),
        };

        let curr = self.buffer()[..curr_end].trim_ascii();
        if !curr.is_empty() {
            let block = &self.buffer.as_ref()[self.pos..self.pos + curr_end];
            let content = core::str::from_utf8(block)?;
            self.pos += curr_end;
            return Ok(Event::Text { content });
        }

        self.pos += curr_end;
        match self.ensure(3) {
            Ok(()) => {}
            Err(crate::Error::Eof) => {
                return Ok(Event::EndOfFile);
            }
            Err(e) => return Err(e),
        };

        enum BlockType {
            Cdata,
            Comment,
            Dtd,
            PI,
            EndElement,
            StartElement,
        }

        let b = self.buffer();
        let (ty, n_start, n_end) = match (b[1], b[2]) {
            (b'!', b'[') => (BlockType::Cdata, "<![CDATA[", "]]>"),
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

        let block = &self.buffer.as_ref()[range].trim_ascii();

        let event = match ty {
            BlockType::Cdata => Event::CDATA { data: block },
            BlockType::Comment => Event::Comment {
                content: core::str::from_utf8(block)?,
            },
            BlockType::Dtd => Event::Dtd {
                content: core::str::from_utf8(block)?,
            },
            BlockType::PI => {
                let (name, attrs) = Self::name_and_attrs(block)?;
                Event::ProcessingInstruction { name, attrs }
            }
            BlockType::EndElement => Event::EndElement {
                name: core::str::from_utf8(block)?,
            },
            BlockType::StartElement => {
                let (name, attrs) = Self::name_and_attrs(block)?;
                Event::StartElement { name, attrs }
            }
        };
        self.pos += end + n_end.len();
        Ok(event)
    }

    fn name_and_attrs(block: &[u8]) -> Result<(&str, AttributeReader<'_>)> {
        let block = core::str::from_utf8(block)?;

        if let Some((name, rest)) = block.split_once(|c: char| c.is_ascii_whitespace()) {
            Ok((name, AttributeReader::from_block(rest)))
        } else {
            Ok((block, AttributeReader::from_block("")))
        }
    }

    /// Moves the unparsed characters starting from offset to the beginning
    /// of the buffer, updates positional indices and reads more data.
    fn advance(&mut self, offset: usize) -> Result<()> {
        trace!(
            "Advancing by {offset} bytes (remaining: {})",
            self.remaining
        );
        if self.remaining == 0 {
            return Err(crate::Error::Eof);
        }
        assert!(offset <= self.end);
        assert!(offset <= self.buffer.as_ref().len());
        trace!("Copying {} bytes to start of buffer", self.end - offset);
        for i in offset..self.end {
            self.buffer.as_mut()[i - offset] = self.buffer.as_ref()[i];
        }
        self.pos = 0;
        self.end -= offset;
        let data_start = self.buffer.as_ref().len() - offset;
        let read_bytes = self
            .reader
            .read(&mut self.buffer.as_mut()[data_start..])
            .map_err(|e| crate::Error::IoError(e.kind()))?;
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
            return Err(crate::Error::Eof);
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
                let Some(end) = memchr::memmem::find(self.buffer(), n_end) else {
                    return Err(crate::Error::Eof);
                };
                Ok((0, end))
            }
            None => {
                self.advance(self.buffer.as_ref().len())?;
                let Some((start, Some(end))) = find_span(self.buffer(), n_start, n_end) else {
                    return Err(crate::Error::Eof);
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
                    return Err(crate::Error::Eof);
                };
                Ok(pos)
            }
        }
    }

    fn buffer(&self) -> &[u8] {
        &self.buffer.as_ref()[self.pos..self.end]
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

    use crate::*;
    use super::*;

    const LOREM: &str = "\
        Lorem ipsum dolor sit amet, consetetur sadipscing elitr,seddiam \
        nonumy eirmod tempor invidunt ut labore et dolore magna aliquya \
        erat, sed diam voluptua. At vero eos et accusam et justo duo do \
        ores et ea rebum. Stet clita kasd gubergren, no sea takimata sa \
        ctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet,\
        consetetur sadipscing elitr, sed diam nonumy eirmod tempor invid\
        unt ut labore et dolore magna aliquyam erat, sed diam voluptua. \
        At vero eos et accusam et justo duo dolores et ea rebum. Stet cl";

    #[test]
    #[cfg(feature = "alloc")]
    fn test_window() {
        let data = LOREM.as_bytes();
        let mut buffer = data;
        let mut parser = Reader::new(&mut buffer, data.len(), 256).unwrap();
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
    #[cfg(feature = "alloc")]
    fn test_find() {
        fn find_str<'a>(
            parser: &'a mut OwnedReader<&'_ [u8]>,
            n_start: &str,
            n_end: &str,
        ) -> Result<&'a str> {
            let (start, end) = parser.try_find(n_start, n_end)?;
            Ok(core::str::from_utf8(&parser.buffer[start..end])?)
        }

        let data = LOREM.as_bytes();
        let buffer = data;
        let mut parser = Reader::new(buffer, data.len(), 256).unwrap();
        let ipsum = find_str(&mut parser, "Lorem ", " dolor").unwrap();
        assert_eq!(ipsum, "ipsum");
        let aliquyam = find_str(&mut parser, "no sea takimata ", " ctus est").unwrap();
        assert_eq!(aliquyam, "sa");
        assert_eq!(parser.buffer(), &data[253..509]);
    }
}
