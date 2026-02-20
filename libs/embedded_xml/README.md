# embedded-xml

A no_std XML reader using embedded-io for memory constrainted environment.

## Features
- no_std
- alloc optional
- streaming

## Usage

```rs
// allocates 1024 bytes internally
let mut reader = xml::Reader::new(&mut reader, xml.len(), 1024)?;
loop {
    match reader.next_event()? {
        xml::Event::StartElement { name, attrs } => {
            println!("Start element: {name} with attributes: {attrs:?}");
        }
        xml::Event::EndElement { name } => {
            println!("End element: {name}");
        }
        xml::Event::EndOfFile => break,
        _ => {}
    }
}
```

## Limitations & non-goals
- no rewinding
- no DTD support
- no XPath
- no decoding
- individual "Events" have to fit inside the internal buffer
