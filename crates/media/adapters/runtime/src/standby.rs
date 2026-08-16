//! The projector-alignment and connection surface shown until this process sees valid DMX.

use media_domain::{Alignment, Size, TextStyle, Tint};

const BACKGROUND: [u8; 4] = [1, 5, 15, 255];
const GUIDE: [u8; 4] = [255, 255, 255, 255];
const GUIDE_LENGTH: u32 = 20;
const LOGO_PNG: &[u8] = include_bytes!("../../../../../assets/branding/tosklight-media-icon.png");

pub struct Frame {
    pub size: Size,
    pub pixels: Vec<u8>,
}

pub fn visible(status_overlay: bool, received_dmx: bool, web_takeover: bool) -> bool {
    status_overlay && !received_dmx && !web_takeover
}

pub fn render(size: Size, endpoint: &str) -> anyhow::Result<Frame> {
    let mut frame = Frame {
        size,
        pixels: vec![0; size.width as usize * size.height as usize * 4],
    };
    for pixel in frame.pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&BACKGROUND);
    }
    draw_guides(&mut frame);
    draw_logo(&mut frame)?;

    let mut fonts = media_text::Fonts::load()?;
    let title_y = size.height.saturating_mul(57) / 100;
    draw_text(
        &mut frame,
        &mut fonts,
        "ToskLight Media",
        title_y,
        96,
        0.42,
        Tint::WHITE,
    )?;
    draw_text(
        &mut frame,
        &mut fonts,
        endpoint,
        title_y + 105,
        70,
        0.40,
        Tint::new(0.40, 0.72, 1.0),
    )?;
    draw_text(
        &mut frame,
        &mut fonts,
        &format!("Connect your browser to {endpoint}"),
        title_y + 190,
        54,
        0.34,
        Tint::WHITE,
    )?;
    draw_text(
        &mut frame,
        &mut fonts,
        "This message disappears once ToskLight Media receives DMX.",
        title_y + 255,
        42,
        0.32,
        Tint::new(0.62, 0.68, 0.78),
    )?;
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

fn draw_logo(frame: &mut Frame) -> anyhow::Result<()> {
    let decoder = png::Decoder::new(std::io::Cursor::new(LOGO_PNG));
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .expect("the embedded PNG has a size")
    ];
    let info = reader.next_frame(&mut buffer)?;
    let edge = (frame.size.height * 32 / 100)
        .min(frame.size.width * 32 / 100)
        .max(1);
    let left = frame.size.width.saturating_sub(edge) / 2;
    let top = frame.size.height.saturating_mul(18) / 100;
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

fn draw_text(
    frame: &mut Frame,
    fonts: &mut media_text::Fonts,
    text: &str,
    top: u32,
    height: u32,
    size: f32,
    colour: Tint,
) -> anyhow::Result<()> {
    let height = height.min(frame.size.height.saturating_sub(top));
    if height == 0 {
        return Ok(());
    }
    let style = TextStyle {
        family: "sans-serif".to_owned(),
        size,
        alignment: Alignment::Center,
        colour,
        ..Default::default()
    };
    // render_line supersamples 2x; asking for half dimensions gives a final-size band.
    let rendered = media_text::render_line(
        fonts,
        text,
        &style,
        (frame.size.width / media_text::SUPERSAMPLE).max(1),
        (height / media_text::SUPERSAMPLE).max(1),
    )?;
    for y in 0..rendered.height.min(height) {
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
