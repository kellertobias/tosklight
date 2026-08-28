//! The projector-alignment and connection surface shown until this process sees valid DMX.

use media_domain::{Alignment, Size, TextStyle, Tint};

const BACKGROUND: [u8; 4] = [0, 0, 0, 255];
const GUIDE: [u8; 4] = [255, 255, 255, 255];
const GUIDE_LENGTH: u32 = 20;
/// The halo's colour, spread as a fraction of the logo edge, and its peak opacity in percent.
const GLOW: [u8; 3] = [255, 255, 255];
const GLOW_SPREAD: u32 = 12;
const GLOW_STRENGTH: u32 = 22;
const LOGO_PNG: &[u8] = include_bytes!("../../../../../assets/branding/ToskLight Pixel.png");

pub struct Frame {
    pub size: Size,
    pub pixels: Vec<u8>,
}

pub fn visible(status_overlay: bool, received_dmx: bool, web_takeover: bool) -> bool {
    status_overlay && !received_dmx && !web_takeover
}

/// One line of the standby block.
///
/// `height` is the height of the glyphs themselves, not of the band they are drawn into, so the
/// spacing constants below describe the gaps a reader actually sees.
struct Line<'a> {
    text: &'a str,
    height: u32,
    colour: Tint,
}

/// The gap between the logo and the name.
const LOGO_GAP: u32 = 20;
/// The gap under each line but the last.
const LINE_GAPS: [u32; 3] = [36, 52, 24];
/// Glyphs are centred in a band twice their height, which is room enough that no line clips its
/// own ascenders or descenders.
const BAND_RATIO: u32 = 2;
const TEXT_SIZE: f32 = 0.5;

pub fn render(size: Size, endpoint: &str) -> anyhow::Result<Frame> {
    let mut frame = Frame {
        size,
        pixels: vec![0; size.width as usize * size.height as usize * 4],
    };
    for pixel in frame.pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&BACKGROUND);
    }
    draw_guides(&mut frame);

    let connect = format!("Connect your browser to {endpoint}");
    let lines = [
        Line {
            text: "ToskLight Pixel",
            height: 80,
            colour: Tint::WHITE,
        },
        Line {
            text: "Media Server",
            height: 50,
            colour: Tint::new(0.62, 0.68, 0.78),
        },
        Line {
            text: &connect,
            height: 40,
            colour: Tint::new(0.40, 0.72, 1.0),
        },
        Line {
            text: "This message disappears once ToskLight Pixel receives DMX.",
            height: 26,
            colour: Tint::new(0.62, 0.68, 0.78),
        },
    ];

    // Logo and text are one block, and the block is what sits in the middle of the output. Laying
    // each part out from its own fraction of the height instead would move them apart on a tall
    // monitor and overlap them on a short one.
    let logo_edge = (size.height * 21 / 100).min(size.width * 21 / 100).max(1);
    let block = logo_edge
        + LOGO_GAP
        + lines.iter().map(|line| line.height).sum::<u32>()
        + LINE_GAPS.iter().sum::<u32>();
    let mut top = size.height.saturating_sub(block) / 2;

    draw_logo(&mut frame, top, logo_edge)?;
    top += logo_edge + LOGO_GAP;

    let mut fonts = media_text::Fonts::load()?;
    for (index, line) in lines.iter().enumerate() {
        draw_text(&mut frame, &mut fonts, line, top + line.height / 2)?;
        top += line.height + LINE_GAPS.get(index).copied().unwrap_or(0);
    }
    Ok(frame)
}

fn draw_guides(frame: &mut Frame) {
    let width = frame.size.width;
    let height = frame.size.height;
    if width == 0 || height == 0 {
        return;
    }
    let horizontal = GUIDE_LENGTH.min(width);
    let vertical = GUIDE_LENGTH.min(height);
    for x in 0..horizontal {
        set(frame, x, 0, GUIDE);
        set(frame, x, height - 1, GUIDE);
        set(frame, width - 1 - x, 0, GUIDE);
        set(frame, width - 1 - x, height - 1, GUIDE);
    }
    for y in 0..vertical {
        set(frame, 0, y, GUIDE);
        set(frame, width - 1, y, GUIDE);
        set(frame, 0, height - 1 - y, GUIDE);
        set(frame, width - 1, height - 1 - y, GUIDE);
    }
}

fn draw_logo(frame: &mut Frame, top: u32, edge: u32) -> anyhow::Result<()> {
    let decoder = png::Decoder::new(std::io::Cursor::new(LOGO_PNG));
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .expect("the embedded PNG has a size")
    ];
    let info = reader.next_frame(&mut buffer)?;
    let left = frame.size.width.saturating_sub(edge) / 2;

    // A halo in the shape of the logo's own silhouette, so the mark lifts off a black field
    // instead of ending on a hard edge. Built from the alpha channel, blurred, and laid down
    // before the logo covers the middle of it.
    let pad = (edge / GLOW_SPREAD).max(1);
    let span = (edge + pad * 2) as usize;
    let mut halo = vec![0u8; span * span];
    for y in 0..edge {
        for x in 0..edge {
            let source_x = x * info.width / edge;
            let source_y = y * info.height / edge;
            let at = ((source_y * info.width + source_x) * 4) as usize;
            halo[(y + pad) as usize * span + (x + pad) as usize] = buffer[at + 3];
        }
    }
    blur(&mut halo, span, (pad / 2).max(1) as usize);
    for y in 0..span {
        for x in 0..span {
            let alpha = u32::from(halo[y * span + x]) * GLOW_STRENGTH / 100;
            if alpha == 0 {
                continue;
            }
            let at_x = left as i64 + x as i64 - i64::from(pad);
            let at_y = top as i64 + y as i64 - i64::from(pad);
            if at_x < 0 || at_y < 0 {
                continue;
            }
            blend(
                frame,
                at_x as u32,
                at_y as u32,
                &[GLOW[0], GLOW[1], GLOW[2], alpha as u8],
            );
        }
    }

    for y in 0..edge {
        for x in 0..edge {
            let source_x = x * info.width / edge;
            let source_y = y * info.height / edge;
            let at = ((source_y * info.width + source_x) * 4) as usize;
            blend(frame, left + x, top + y, &buffer[at..at + 4]);
        }
    }
    Ok(())
}

/// Blurs a square single-channel image in place, three box passes standing in for a Gaussian.
fn blur(image: &mut [u8], span: usize, radius: usize) {
    let mut scratch = vec![0u8; image.len()];
    for _ in 0..3 {
        blur_pass(image, &mut scratch, span, radius, true);
        blur_pass(&scratch, image, span, radius, false);
    }
}

/// One box pass along rows or columns, carrying a running sum rather than re-adding the window.
fn blur_pass(source: &[u8], target: &mut [u8], span: usize, radius: usize, rows: bool) {
    let index = |line: usize, at: usize| {
        if rows {
            line * span + at
        } else {
            at * span + line
        }
    };
    for line in 0..span {
        let mut sum = 0u32;
        let mut high = 0usize;
        while high < span && high <= radius {
            sum += u32::from(source[index(line, high)]);
            high += 1;
        }
        for at in 0..span {
            let low = at.saturating_sub(radius);
            target[index(line, at)] = (sum / ((high - low) as u32).max(1)) as u8;
            if at + 1 + radius < span {
                sum += u32::from(source[index(line, at + 1 + radius)]);
                high += 1;
            }
            if at >= radius {
                sum -= u32::from(source[index(line, at - radius)]);
            }
        }
    }
}

fn draw_text(
    frame: &mut Frame,
    fonts: &mut media_text::Fonts,
    line: &Line,
    centre: u32,
) -> anyhow::Result<()> {
    let band = line.height * BAND_RATIO;
    let top = centre.saturating_sub(band / 2);
    let band = band.min(frame.size.height.saturating_sub(top));
    if band == 0 {
        return Ok(());
    }
    let style = TextStyle {
        family: "sans-serif".to_owned(),
        size: TEXT_SIZE,
        alignment: Alignment::Center,
        colour: line.colour,
        ..Default::default()
    };
    // render_line supersamples 2x; asking for half dimensions gives a final-size band.
    let rendered = media_text::render_line(
        fonts,
        line.text,
        &style,
        (frame.size.width / media_text::SUPERSAMPLE).max(1),
        (band / media_text::SUPERSAMPLE).max(1),
    )?;
    for y in 0..rendered.height.min(band) {
        for x in 0..rendered.width.min(frame.size.width) {
            let at = ((y * rendered.width + x) * 4) as usize;
            blend(frame, x, top + y, &rendered.pixels[at..at + 4]);
        }
    }
    Ok(())
}

fn set(frame: &mut Frame, x: u32, y: u32, colour: [u8; 4]) {
    let at = ((y * frame.size.width + x) * 4) as usize;
    frame.pixels[at..at + 4].copy_from_slice(&colour);
}

fn blend(frame: &mut Frame, x: u32, y: u32, source: &[u8]) {
    if x >= frame.size.width || y >= frame.size.height {
        return;
    }
    let at = ((y * frame.size.width + x) * 4) as usize;
    let alpha = u16::from(source[3]);
    for (channel, foreground) in source.iter().copied().take(3).enumerate() {
        let foreground = u16::from(foreground);
        let background = u16::from(frame.pixels[at + channel]);
        frame.pixels[at + channel] =
            ((foreground * alpha + background * (255 - alpha)) / 255) as u8;
    }
    frame.pixels[at + 3] = 255;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standby_ends_permanently_after_the_first_valid_dmx() {
        assert!(visible(true, false, false));
        assert!(!visible(true, true, false));
        assert!(!visible(false, false, false));
    }

    #[test]
    fn browser_takeover_exposes_program_output_without_waiting_for_dmx() {
        assert!(!visible(true, false, true));
    }

    #[test]
    fn every_corner_has_exactly_two_one_pixel_twenty_pixel_guides() {
        let mut frame = Frame {
            size: Size::new(100, 80),
            pixels: vec![0; 100 * 80 * 4],
        };
        draw_guides(&mut frame);
        let pixel = |x: u32, y: u32| {
            let at = ((y * 100 + x) * 4) as usize;
            &frame.pixels[at..at + 4]
        };
        assert_eq!(pixel(19, 0), GUIDE);
        assert_eq!(pixel(20, 0), [0, 0, 0, 0]);
        assert_eq!(pixel(0, 19), GUIDE);
        assert_eq!(pixel(0, 20), [0, 0, 0, 0]);
        assert_eq!(pixel(80, 79), GUIDE);
        assert_eq!(pixel(79, 79), [0, 0, 0, 0]);
        assert_eq!(pixel(99, 60), GUIDE);
        assert_eq!(pixel(99, 59), [0, 0, 0, 0]);
    }
}
