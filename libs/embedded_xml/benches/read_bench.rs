use criterion::{black_box, criterion_group, criterion_main, Criterion};

use embedded_io::Read;
use embedded_xml::{Event, Reader};

/// Consume every event from the reader, return the count.
fn drain_events<T: Read>(mut reader: Reader<T, &mut [u8]>) -> usize {
    let mut count = 0usize;
    loop {
        match reader.next_event().unwrap() {
            Event::EndOfFile => break,
            _ => count += 1,
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Benchmark: stream-parse a large repeated XML payload
// ---------------------------------------------------------------------------
fn bench_stream_parse(c: &mut Criterion) {
    let fragment = r#"<root><child attr="val">Text</child><child>More text</child></root>"#;
    let mut data = String::new();
    for _ in 0..2000 {
        data.push_str(fragment);
    }
    let bytes = data.into_bytes();
    let total = bytes.len();
    let mut buf = vec![0u8; 4096];

    c.bench_function("stream_parse_2k_fragments", |b| {
        b.iter(|| {
            let mut slice: &[u8] = black_box(&bytes);
            let reader = Reader::new_borrowed(&mut slice, total, &mut buf).unwrap();
            black_box(drain_events(reader))
        })
    });
}

criterion_group!(
    benches,
    bench_stream_parse,
);
criterion_main!(benches);
