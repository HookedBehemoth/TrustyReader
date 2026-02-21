#![cfg_attr(rustfmt, rustfmt_skip)]
extern crate std;

use core::assert_matches;

macro_rules! trace {
    ($($arg:tt)*) => {
        std::eprintln!($($arg)*);
    };
}

use super::*;
use std::string::String;
use std::vec::Vec;

fn walk(xml: &str) {
    let mut bytes = xml.as_bytes();
    let mut buffer = [0u8; 2048];
    let mut parser = Reader::new_borrowed(&mut bytes, xml.len(), &mut buffer).unwrap();
    let mut element_stack = Vec::<String>::new();
    loop {
        match parser.next_event().unwrap() {
            Event::Declaration { attrs } => {
                trace!("--Declaration");
                assert_eq!(attrs.get("version"), Some("1.0"));
                assert!(
                    attrs
                        .get("encoding")
                        .map(|v: &str| v.eq_ignore_ascii_case("utf-8"))
                        == Some(true)
                );
            }
            Event::EndOfFile { .. } => {
                trace!("--End of file");
                break;
            }
            Event::StartElement { name, .. } => {
                trace!("--Start element: {}", name);
                element_stack.push(String::from(name));
            }
            Event::Text { content } => {
                trace!("--Text: {}", content);
            }
            Event::EndElement { name } => {
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
}

#[test]
fn test_walk_opf() {
    let xml = include_str!("test_data/content.opf");
    walk(xml);
}

#[test]
fn test_walk_container() {
    let xml = include_str!("test_data/container.xml");
    walk(xml);
}

#[test]
fn test_walk_content() {
    let xml = include_str!("test_data/pg-header.xhtml");
    walk(xml);
    let xml = include_str!("test_data/toc.xhtml");
    walk(xml);
    let xml = include_str!("test_data/pg-footer.xhtml");
    walk(xml);
}

#[test]
fn full_tree() {
    use Event::*;

    let xml = b"\
        <?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
        <root attr1=\"value1\" attr2=\"value2\">\
            <child>Text</child>\
            <?example processing=\"instructions\"?>\
            <child>More text</child>\
            <![CDATA[doesn't need to be \xff utf-8 encoded]]>
            <!-- a comment -->\
            <self-closing />\
            <self-closing/>\
        </root>";
    let mut data = &xml[..];
    let mut buffer = [0u8; 512];
    let mut parser = Reader::new_borrowed(&mut data, xml.len(), &mut buffer).unwrap();
    let Declaration { attrs } = parser.next_event().unwrap() else {
        panic!("Expected declaration");
    };
    assert_eq!(attrs.get("version"), Some("1.0"));
    assert_eq!(attrs.get("encoding"), Some("UTF-8"));
    assert_eq!(attrs.get("standalone"), Some("yes"));
    let StartElement { name: "root", attrs } = parser.next_event().unwrap() else {
        panic!("Expected start element");
    };
    assert_eq!(attrs.get("attr1"), Some("value1"));
    assert_eq!(attrs.get("attr2"), Some("value2"));
    assert_matches!(parser.next_event(), Ok(StartElement { name: "child", .. }) );
    assert_matches!(parser.next_event(), Ok(Text { content: "Text" }));
    assert_matches!(parser.next_event(), Ok(EndElement { name: "child" }));
    let ProcessingInstruction { name: "example", mut attrs } = parser.next_event().unwrap() else {
        panic!("Expected processing instruction");
    };
    assert_matches!(attrs.next(), Some(("processing", "instructions")));
    assert_matches!(attrs.next(), None);
    assert_matches!(parser.next_event(), Ok(StartElement { name: "child", .. }) );
    assert_matches!(parser.next_event(), Ok(Text { content: "More text" }));
    assert_matches!(parser.next_event(), Ok(EndElement { name: "child" }));
    assert_matches!(parser.next_event(), Ok(CDATA { data: b"doesn't need to be \xff utf-8 encoded" }));
    assert_matches!(parser.next_event(), Ok(Comment { content: "a comment" }));
    assert_matches!(parser.next_event(), Ok(StartElement { name: "self-closing", .. }) );
    assert_matches!(parser.next_event(), Ok(EndElement { name: "self-closing" }) );
    assert_matches!(parser.next_event(), Ok(StartElement { name: "self-closing", .. }) );
    assert_matches!(parser.next_event(), Ok(EndElement { name: "self-closing" }) );
    assert_matches!(parser.next_event(), Ok(EndElement { name: "root" }));
    assert_matches!(parser.next_event(), Ok(EndOfFile));
}

#[test]
fn invalid_doc() {
    let xml = "This isn't actually an XML document";
    let mut bytes = xml.as_bytes();
    let mut buffer = [0u8; 512];
    let mut parser = Reader::new_borrowed(&mut bytes, xml.len(), &mut buffer).unwrap();
    assert_matches!(parser.next_event(), Err(crate::Error::Eof));

    // we don't care about unclosed elements.
    let xml = "<?xml?><unclosed><child>Text</child>";
    let mut bytes = xml.as_bytes();
    let mut buffer = [0u8; 512];
    let mut parser = Reader::new_borrowed(&mut bytes, xml.len(), &mut buffer).unwrap();
    assert_matches!(parser.next_event(), Ok(Event::Declaration { .. }));
    assert_matches!(parser.next_event(), Ok(Event::StartElement { name: "unclosed", .. }));
    assert_matches!(parser.next_event(), Ok(Event::StartElement { name: "child", .. }));
    assert_matches!(parser.next_event(), Ok(Event::Text { content: "Text" }));
    assert_matches!(parser.next_event(), Ok(Event::EndElement { name: "child" }));
    assert_matches!(parser.next_event(), Ok(Event::EndOfFile));
}

#[test]
fn non_owning() {
    let xml = "<?xml?><root>Text</root>";
    let mut bytes = xml.as_bytes();
    let mut buffer = [0u8; 5];
    let mut parser = Reader::new_borrowed(&mut bytes, xml.len(), &mut buffer).unwrap();
    assert_matches!(parser.next_event(), Ok(Event::Declaration { .. }));
    assert_matches!(parser.next_event(), Ok(Event::StartElement { name: "root", .. }));
    assert_matches!(parser.next_event(), Ok(Event::Text { content: "Text" }));
    assert_matches!(parser.next_event(), Ok(Event::EndElement { name: "root" }));
    assert_matches!(parser.next_event(), Ok(Event::EndOfFile));
}
