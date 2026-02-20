use crate::attributes::AttributeReader;

#[derive(Debug, Clone, PartialEq)]
pub enum Event<'a> {
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