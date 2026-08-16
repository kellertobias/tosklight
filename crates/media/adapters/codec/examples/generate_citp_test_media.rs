//! Generates the repository-owned CITP interoperability clip and thumbnail.
//!
//! The pattern is mathematical and contains no third-party artwork. Re-running this example with
//! the fixture directory as its argument deterministically replaces every generated file.

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use media_codec::{ClipHeader, ClipWriter, encode};

const WIDTH: u32 = 128;
const HEIGHT: u32 = 72;
const FRAMES: u32 = 8;
const FRAME_INTERVAL_MICROS: u64 = 125_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: generate_citp_test_media <fixture-directory>")?;
    let library = root.join("library/001");
    fs::create_dir_all(library.join(".thumbs"))?;
    fs::create_dir_all(root.join("source"))?;

    let frames = (0..FRAMES).map(pattern).collect::<Vec<_>>();
    write_png(&root.join("source/001-CITP-Test-Pattern.png"), &frames[0])?;
    write_jpeg(&library.join(".thumbs/001-thumb.jpg"), &frames[0])?;
    write_clip(&library.join("001-CITP Test Pattern.toskclip"), &frames)?;
    fs::write(library.join(".info"), "CITP Test\n")?;
    Ok(())
}

fn pattern(frame: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let checker = ((x / 8) + (y / 8)) % 2 == 0;
            let quadrant = (x >= WIDTH / 2, y >= HEIGHT / 2);
            let mut colour: [u8; 4] = match quadrant {
                (false, false) => [235, 35, 60, 255],
                (true, false) => [25, 180, 95, 255],
                (false, true) => [30, 90, 235, 255],
                (true, true) => [245, 190, 20, 255],
            };
            if checker {
                colour[..3].iter_mut().for_each(|channel| {
                    *channel = (*channel).saturating_add(18);
                });
            }
            let marker_x = 8 + frame * 14;
            if x.abs_diff(marker_x) <= 2 || y == HEIGHT / 2 || x == WIDTH / 2 {
                colour = [255, 255, 255, 255];
            }
            rgba.extend_from_slice(&colour);
        }
    }
    rgba
}

fn write_clip(path: &Path, frames: &[Vec<u8>]) -> Result<(), Box<dyn std::error::Error>> {
    let header = ClipHeader {
        width: WIDTH,
        height: HEIGHT,
        frame_count: frames.len() as u32,
        frame_rate: (8, 1),
        intrinsic_bpm: None,
    };
    let mut writer = ClipWriter::new(File::create(path)?, header)?;
    for (index, frame) in frames.iter().enumerate() {
        writer.write_frame(
            &encode(WIDTH, HEIGHT, frame)?,
            index as u64 * FRAME_INTERVAL_MICROS,
        )?;
    }
    writer.finish()?;
    Ok(())
}

fn write_png(path: &Path, rgba: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

fn write_jpeg(path: &Path, rgba: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let rgb = rgba
        .chunks_exact(4)
        .flat_map(|pixel| pixel[..3].iter().copied())
        .collect::<Vec<_>>();
    let mut encoded = Vec::new();
    jpeg_encoder::Encoder::new(&mut encoded, 90).encode(
        &rgb,
        WIDTH as u16,
        HEIGHT as u16,
        jpeg_encoder::ColorType::Rgb,
    )?;
    fs::write(path, encoded)?;
    Ok(())
}
