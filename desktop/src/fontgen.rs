use embedded_graphics::prelude::OriginDimensions;
use image::ImageFormat;
use log::{info, trace, warn};
use trusty_core::{framebuffer::{BUFFER_SIZE, DisplayBuffers, HEIGHT, WIDTH}, res::font::{FontDefinition, Glyph, Mode, draw_glyph}};


/// CLI Arguments
#[derive(argh::FromArgs)]
struct Args {
    /// input font file
    #[argh(option, short = 'i')]
    input: Vec<String>,

    /// output font file
    #[argh(positional)]
    output: String,

    /// character file
    #[argh(option)]
    character_file: Option<String>,

    /// font size
    #[argh(option, default = "vec![26.0]", short = 's')]
    font_size: Vec<f32>,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Args = argh::from_env();

    let characters = load_chars(args.character_file.as_deref());

    for input in &args.input {
        generate_font(input, &args.font_size, &characters, &args.output);
    }
}

fn generate_font(font_path: &str, sizes: &[f32], characters: &[char], out_path: &str) {
    let font_file = std::fs::read(font_path)
        .expect("Failed to read input font file");
    let font = fontdue::Font::from_bytes(font_file.as_slice(), fontdue::FontSettings::default())
        .expect("Failed to parse font file");

    for &size in sizes {
        generate_font_size(&font, size, characters, out_path);
        analyze_font_metrics(&font, size);
    }
}

fn generate_font_size(font: &fontdue::Font, font_size: f32, characters: &[char], out_path: &str) {
    let mut glyphs = Vec::new();
    let mut bw_buffer: Vec<u8> = Vec::new();
    let mut msb_buffer: Vec<u8> = Vec::new();
    let mut lsb_buffer: Vec<u8> = Vec::new();
    let mut bitmap_index = 0u16;

    for &ch in characters {
        if !font.has_glyph(ch) {
            warn!("Font does not have glyph for character: '{}'", ch);
            continue;
        }
        let (metrics, bitmap) = font.rasterize(ch, font_size);
        trace!(
            "Character: '{}', Width: {}, Height: {}, Advance Width: {}, xmin: {}, ymin: {}",
            ch, metrics.width, metrics.height, metrics.advance_width, metrics.xmin, metrics.ymin
        );
        let glyph = Glyph::new(
            ch as u16,
            bitmap_index,
            metrics.advance_width.ceil() as u8,
            metrics.width as u8,
            metrics.height as u8,
            metrics.xmin as i8,
            metrics.ymin as i8,
        );
        glyphs.push(glyph);
        let new_size = bitmap_index as usize + bitmap.len().div_ceil(8);
        bw_buffer.resize(new_size, 0u8);
        msb_buffer.resize(new_size, 0u8);
        lsb_buffer.resize(new_size, 0u8);
        for (idx, &byte) in bitmap.iter().enumerate() {
            let byte = 255 - byte;
            let (bw, msb, lsb) = if byte >= 205 {
                (1u8, 0u8, 0u8)
            } else if byte >= 154 {
                (1u8, 0u8, 1u8)
            } else if byte >= 103 {
                (0u8, 1u8, 0u8)
            } else if byte >= 52 {
                (0u8, 1u8, 1u8)
            } else {
                (0u8, 0u8, 0u8)
            };
            let byte_idx = bitmap_index as usize + idx / 8;
            let bit_idx = 7 - (idx % 8);
            bw_buffer
                .get_mut(byte_idx)
                .map(|b| *b |= bw << bit_idx);
            msb_buffer
                .get_mut(byte_idx)
                .map(|b| *b |= msb << bit_idx);
            lsb_buffer
                .get_mut(byte_idx)
                .map(|b| *b |= lsb << bit_idx);
        }
        let bytes = bitmap.len().div_ceil(8);
        bitmap_index += bytes as u16;
    }
    info!("Glyphs: {}", glyphs.len());
    info!("Bitmap size (bytes): {}", bw_buffer.len());
    assert!(bw_buffer.len() == msb_buffer.len() && bw_buffer.len() == lsb_buffer.len());

    let my_font = FontDefinition {
        size: bw_buffer.len() as u32,
        y_advance: font.vertical_line_metrics(font_size).map(|m| m.new_line_size.ceil() as usize).unwrap_or(font_size.ceil() as usize) as u8,
        glyphs: &glyphs,
        bitmap_bw: &bw_buffer,
        bitmap_msb: &msb_buffer,
        bitmap_lsb: &lsb_buffer,
    };

    let name = font.name().expect("Failed to get font name");
    let file_name = format!("{}_{}", name.to_ascii_lowercase().replace(" ", "_"), font_size as u8);
    info!("Generating font: {name} as {file_name} at size {font_size}");
    let base_path = std::path::Path::new(&out_path).join(&file_name);
    std::fs::write(base_path.with_extension("bw"), &bw_buffer).expect("Failed to write BW font file");
    std::fs::write(base_path.with_extension("msb"), &msb_buffer).expect("Failed to write MSB font file");
    std::fs::write(base_path.with_extension("lsb"), &lsb_buffer).expect("Failed to write LSB font file");

    let rust_file = base_path.with_extension("rs");
    let mut rust_code = String::new();
    rust_code.push_str("// Auto-generated font file\n");
    rust_code.push_str(&format!("// Font: {}\n\n", name));
    rust_code.push_str("use crate::res::font::{FontDefinition, Glyph};\n\n");
    rust_code.push_str(&format!("pub static FONT: FontDefinition = FontDefinition {{\n"));
    rust_code.push_str(&format!("    size: {},\n", my_font.size));
    rust_code.push_str(&format!("    y_advance: {},\n", my_font.y_advance));
    rust_code.push_str(&format!("    glyphs: &GLYPHS,\n"));
    rust_code.push_str(&format!("    bitmap_bw: BITMAP_BW,\n"));
    rust_code.push_str(&format!("    bitmap_msb: BITMAP_MSB,\n"));
    rust_code.push_str(&format!("    bitmap_lsb: BITMAP_LSB,\n"));
    rust_code.push_str("};\n\n");
    rust_code.push_str(&format!("static GLYPHS: [Glyph; {}] = [\n", glyphs.len()));
    for glyph in &glyphs {
        rust_code.push_str(&format!(
            "    Glyph::new(0x{:04X}, 0x{:04X}, {}, {}, {}, {}, {}),\n",
            glyph.codepoint, glyph.bitmap_index, glyph.x_advance(), glyph.width(), glyph.height(), glyph.xmin(), glyph.ymin()
        ));
    }
    rust_code.push_str("];\n\n");
    rust_code.push_str(&format!("static BITMAP_BW: &'static [u8; {}] = include_bytes!(\"./{}.bw\");\n", bw_buffer.len(), file_name));
    rust_code.push_str(&format!("static BITMAP_MSB: &'static [u8; {}] = include_bytes!(\"./{}.msb\");\n", msb_buffer.len(), file_name));
    rust_code.push_str(&format!("static BITMAP_LSB: &'static [u8; {}] = include_bytes!(\"./{}.lsb\");\n", lsb_buffer.len(), file_name));
    std::fs::write(&rust_file, rust_code).expect("Failed to write Rust font file");

    test_font_drawing(&my_font);
}

fn test_font_drawing(font: &FontDefinition) {
    info!("testing font draw");
    let mut fb_bw = Box::new(DisplayBuffers::default());
    let mut fb_msb = Box::new(DisplayBuffers::default());
    let mut fb_lsb = Box::new(DisplayBuffers::default());
    fb_bw.clear_screen(0xFF);
    fb_msb.clear_screen(0x00);
    fb_lsb.clear_screen(0x00);
    // fb_bw.set_rotation(trusty_core::framebuffer::Rotation::Rotate270);
    // fb_msb.set_rotation(trusty_core::framebuffer::Rotation::Rotate270);
    // fb_lsb.set_rotation(trusty_core::framebuffer::Rotation::Rotate270);

    let x_start = 10usize;
    let x_end = fb_bw.size().width as usize - 10usize;
    let mut x_advance = x_start;
    let mut y_advance = 0usize;
    y_advance += font.y_advance as usize;
    for glyph in font.glyphs {
        if (x_advance + glyph.x_advance() as usize) >= x_end {
            x_advance = x_start;
            y_advance += font.y_advance as usize;
        }
        draw_glyph(&font, glyph.codepoint, &mut fb_bw, x_advance as isize, y_advance as isize, Mode::Bw).expect("Glyph not found");
        draw_glyph(&font, glyph.codepoint, &mut fb_msb, x_advance as isize, y_advance as isize, Mode::Msb).expect("Glyph not found");
        draw_glyph(&font, glyph.codepoint, &mut fb_lsb, x_advance as isize, y_advance as isize, Mode::Lsb).expect("Glyph not found");
        x_advance += glyph.x_advance() as usize;
    }

    let fb_bw = fb_bw.get_active_buffer();
    let fb_msb = fb_msb.get_active_buffer();
    let fb_lsb = fb_lsb.get_active_buffer();

    let mut blowup_bw = vec![0u8; WIDTH * HEIGHT];
    let mut blowup_msb = vec![0u8; WIDTH * HEIGHT];
    let mut blowup_lsb = vec![0u8; WIDTH * HEIGHT];
    let mut merged = vec![0u8; WIDTH * HEIGHT];
    for i in 0..fb_bw.len() {
        let bw = fb_bw[i];
        let msb = fb_msb[i];
        let lsb = fb_lsb[i];
        for bit in 0..8 {
            let idx = i * 8 + bit;
            let pixel_bw = (bw >> (7 - bit)) & 0x01;
            blowup_bw[idx] = if pixel_bw == 0 { 0u8 } else { 255u8 };
            let pixel_msb = (msb >> (7 - bit)) & 0x01;
            blowup_msb[idx] = if pixel_msb == 0 { 0u8 } else { 255u8 };
            let pixel_lsb = (lsb >> (7 - bit)) & 0x01;
            blowup_lsb[idx] = if pixel_lsb == 0 { 0u8 } else { 255u8 };
            merged[idx] = match (pixel_bw, pixel_msb, pixel_lsb) {
                (_, 1, 1) => 0x33u8,
                (_, 0, 1) => 0x55u8,
                (_, 1, 0) => 0xaau8,
                (1, _, _) => 0xffu8,
                (0, _, _) => 0x00u8,
                _ => unreachable!(),
            };
        }
    }
    image::save_buffer(
        &std::path::Path::new("font_bw.png"),
        &blowup_bw,
        WIDTH as u32,
        HEIGHT as u32,
        image::ColorType::L8,
    ).expect("Failed to save image");
    image::save_buffer(
        &std::path::Path::new("font_msb.png"),
        &blowup_msb,
        WIDTH as u32,
        HEIGHT as u32,
        image::ColorType::L8,
    ).expect("Failed to save image");
    image::save_buffer(
        &std::path::Path::new("font_lsb.png"),
        &blowup_lsb,
        WIDTH as u32,
        HEIGHT as u32,
        image::ColorType::L8,
    ).expect("Failed to save image");
    image::save_buffer(
        &std::path::Path::new("font_merged.png"),
        &merged,
        WIDTH as u32,
        HEIGHT as u32,
        image::ColorType::L8,
    ).expect("Failed to save image");
}

fn analyze_font_metrics(font: &fontdue::Font, font_size: f32) {
    let mut max_advance = 0u8;
    let mut max_width = 0u32;
    let mut max_height = 0u32;
    let mut min_xmin = 0i32;
    let mut max_xmin = 0i32;
    let mut min_ymin = 0i32;
    let mut max_ymin = 0i32;

    for (&ch, &idx) in font.chars() {
        let metrics = font.metrics_indexed(idx.into(), font_size);
        if metrics.advance_width > 63.0 {
            warn!(
                "Large advance for '{ch}' width: Index: {}, Advance Width: {}",
                idx,
                metrics.advance_width
            );
            continue;
        }

        max_advance = max_advance.max(metrics.advance_width.ceil() as u8);
        max_width = max_width.max(metrics.width as u32);
        max_height = max_height.max(metrics.height as u32);
        min_xmin = min_xmin.min(metrics.xmin);
        max_xmin = max_xmin.max(metrics.xmin);
        min_ymin = min_ymin.min(metrics.ymin);
        max_ymin = max_ymin.max(metrics.ymin);
    }
    info!("Max Advance: {}", max_advance);
    info!("Max Width: {}", max_width);
    info!("Max Height: {}", max_height);
    info!("Xmin Range: {} to {}", min_xmin, max_xmin);
    info!("Ymin Range: {} to {}", min_ymin, max_ymin);
}

static DEFAULT_CHARACTERS: &str = r##" !"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\]^_`abcdefghijklmnopqrstuvwxyz{|}~¡¢£¤¥§©ª«®°±²³µ¶·¹º»¼½¾¿ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÞßàáâãäåæçèéêëìíîïðñòóôõö÷øùúûüþÿĄąĆćČčĎďĘęĚěĹĺĽľŁłŃńŇňŒœŘřŚśŠšŤťŮůŰűŸŹźŻżŽžπẞ–—’†‡•…‹›⁰⁴⁵⁶⁷⁸⁹₀₁₂₃₄₅₆₇₈₉₩₪€₴₹₽™⅓⅔⅛⅜⅝⅞←↑→↓↔↕⇐⇑⇒⇓⇔∂∆∏∑√∞∫≠≤≥─│┌┐└┘├┤┬┴┼═║╔╗╚╝╠╣╦╩╬‘’‚‛“”„‟"##;

fn load_chars(path: Option<&str>) -> Vec<char> {
    let mut characters = if let Some(character_file) = path {
        std::fs::read_to_string(character_file).expect("Failed to read character file")
            .chars().collect::<Vec<char>>()
    } else {
        DEFAULT_CHARACTERS.chars().collect::<Vec<char>>()
    };
    characters.sort_unstable();
    characters.dedup();
    characters
}
