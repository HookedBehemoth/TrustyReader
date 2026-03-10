use std::path::PathBuf;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use trusty_core::container::epub;
use trusty_core::container::image::Format;
use trusty_core::fs::Filesystem;
use trusty_desktop::std_fs::StdFilesystem;

/// Resolve the `sd/books` directory relative to the workspace root.
fn books_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().join("sd").join("books")
}

fn fs() -> StdFilesystem {
    StdFilesystem::new_with_base_path(books_dir())
}

/// List all `.epub` files in the books directory.
///
/// Uses `std::fs` directly because `StdFilesystem::open_directory("/")`
/// doesn't work on the host (Rust's `PathBuf::join("/")` replaces the
/// base path on Unix).
fn epub_files() -> Vec<String> {
    let dir = books_dir();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("could not read sd/books – is the directory present?")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "epub")
                && e.file_type().is_ok_and(|ft| ft.is_file())
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Benchmark parsing the EPUB container (ZIP + OPF + TOC).
fn bench_epub_parse(c: &mut Criterion) {
    let filesystem = fs();
    let files = epub_files();

    let mut group = c.benchmark_group("epub_parse");
    for name in &files {
        group.bench_with_input(BenchmarkId::from_parameter(name), name, |b, name| {
            b.iter(|| {
                let mut file = filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
                let book = epub::parse(&mut file).unwrap();
                black_box(&book);
            });
        });
    }
    group.finish();
}

/// Benchmark parsing every chapter of each EPUB.
fn bench_epub_parse_chapters(c: &mut Criterion) {
    let filesystem = fs();
    let files = epub_files();

    let mut group = c.benchmark_group("epub_parse_chapters");
    // Chapters can be many – allow more time per measurement.
    group.sample_size(10);

    for name in &files {
        let mut file = filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
        let book = epub::parse(&mut file).unwrap();
        let n_chapters = book.spine.len();

        group.bench_with_input(
            BenchmarkId::new(name, format!("{n_chapters} chapters")),
            &book,
            |b, book| {
                b.iter(|| {
                    let mut file =
                        filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
                    for i in 0..book.spine.len() {
                        let _ = black_box(epub::parse_chapter(book, i, &mut file));
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark parsing a single chapter (first chapter of each book).
fn bench_epub_parse_single_chapter(c: &mut Criterion) {
    let filesystem = fs();
    let files = epub_files();

    let mut group = c.benchmark_group("epub_parse_single_chapter");

    for name in &files {
        let mut file = filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
        let book = epub::parse(&mut file).unwrap();
        if book.spine.is_empty() {
            continue;
        }

        group.bench_with_input(BenchmarkId::from_parameter(name), &book, |b, book| {
            b.iter(|| {
                let mut file =
                    filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
                let ch = epub::parse_chapter(book, 0, &mut file);
                black_box(ch)
            });
        });
    }
    group.finish();
}

/// Benchmark decoding all images in each EPUB at 800×480 (landscape).
fn bench_epub_parse_images_landscape(c: &mut Criterion) {
    let filesystem = fs();
    let files = epub_files();
    let max = (800u16, 480u16);

    let mut group = c.benchmark_group("epub_parse_images_landscape");
    group.sample_size(10);

    for name in &files {
        let mut file = filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
        let book = epub::parse(&mut file).unwrap();

        let image_indices: Vec<u16> = book
            .file_resolver
            .entries()
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                Format::guess_from_filename(&entry.name).map(|_| idx as u16)
            })
            .collect();

        if image_indices.is_empty() {
            continue;
        }

        group.bench_with_input(
            BenchmarkId::new(name, format!("{} images", image_indices.len())),
            &(book, image_indices),
            |b, (book, indices)| {
                b.iter(|| {
                    let mut file =
                        filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
                    for &idx in indices {
                        let img = epub::parse_image(book, idx, max, &mut file);
                        black_box(&img);
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark decoding all images in each EPUB at 480×800 (portrait).
fn bench_epub_parse_images_portrait(c: &mut Criterion) {
    let filesystem = fs();
    let files = epub_files();
    let max = (480u16, 800u16);

    let mut group = c.benchmark_group("epub_parse_images_portrait");
    group.sample_size(10);

    for name in &files {
        let mut file = filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
        let book = epub::parse(&mut file).unwrap();

        let image_indices: Vec<u16> = book
            .file_resolver
            .entries()
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                Format::guess_from_filename(&entry.name).map(|_| idx as u16)
            })
            .collect();

        if image_indices.is_empty() {
            continue;
        }

        group.bench_with_input(
            BenchmarkId::new(name, format!("{} images", image_indices.len())),
            &(book, image_indices),
            |b, (book, indices)| {
                b.iter(|| {
                    let mut file =
                        filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
                    for &idx in indices {
                        let img = epub::parse_image(book, idx, max, &mut file);
                        black_box(&img);
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark decoding a single image from an EPUB.
fn bench_epub_parse_single_image(c: &mut Criterion) {
    let filesystem = fs();
    let files = epub_files();
    let max = (800u16, 480u16);

    let mut group = c.benchmark_group("epub_parse_single_image");

    for name in &files {
        let mut file = filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
        let book = epub::parse(&mut file).unwrap();

        // Find the first image entry.
        let first_image = book
            .file_resolver
            .entries()
            .iter()
            .enumerate()
            .find_map(|(idx, entry)| {
                Format::guess_from_filename(&entry.name).map(|fmt| (idx as u16, fmt, entry.name.clone()))
            });

        let Some((idx, _fmt, img_name)) = first_image else {
            continue;
        };

        group.bench_with_input(
            BenchmarkId::new(name, &*img_name),
            &(book, idx),
            |b, (book, idx)| {
                b.iter(|| {
                    let mut file =
                        filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
                    let img = epub::parse_image(book, *idx, max, &mut file);
                    black_box(img)
                });
            },
        );
    }
    group.finish();
}

/// Full end-to-end benchmark: parse + all chapters + all images (mirrors bench.rs).
fn bench_epub_full(c: &mut Criterion) {
    let filesystem = fs();
    let files = epub_files();
    let max = (800u16, 480u16);

    let mut group = c.benchmark_group("epub_full");
    group.sample_size(10);

    for name in &files {
        group.bench_with_input(BenchmarkId::from_parameter(name), name, |b, name| {
            b.iter(|| {
                let mut file =
                    filesystem.open_file(name, trusty_core::fs::Mode::Read).unwrap();
                let book = epub::parse(&mut file).unwrap();

                // Parse all chapters
                for i in 0..book.spine.len() {
                    let _ = black_box(epub::parse_chapter(&book, i, &mut file));
                }

                // Decode all images
                let entries = book.file_resolver.entries();
                for idx in 0..entries.len() {
                    if Format::guess_from_filename(&entries[idx].name).is_some() {
                        let _ = black_box(epub::parse_image(
                            &book,
                            idx as u16,
                            max,
                            &mut file,
                        ));
                    }
                }
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_epub_parse,
    bench_epub_parse_single_chapter,
    bench_epub_parse_chapters,
    bench_epub_parse_single_image,
    bench_epub_parse_images_landscape,
    bench_epub_parse_images_portrait,
    bench_epub_full,
);
criterion_main!(benches);
