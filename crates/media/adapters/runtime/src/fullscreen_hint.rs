//! The short operator hint drawn over a clicked full-screen output.

use media_domain::{Alignment, Size, TextStyle, Tint};

use crate::standby::Frame;

pub const RESTORE_CHORD: &str = "Ctrl + Shift + -";
pub const MOVE_CHORD: &str = "Ctrl + Shift + Arrow";

pub fn render(size: Size) -> anyhow::Result<Frame> {
    let mut frame = Frame {
        size,
        pixels: vec![0; size.width as usize * size.height as usize * 4],
    };
    if size.is_empty() {
        return Ok(frame);
    }

    let panel_width = size.width.min(760).saturating_sub(32).max(1);
    let panel_height = size.height.min(118).saturating_sub(16).max(1);
    let left = size.width.saturating_sub(panel_width) / 2;
    let top = size.height.saturating_sub(panel_height + 44);
    fill(
        &mut frame,
        left,
        top,
        panel_width,
        panel_height,
        [10, 13, 18, 224],
    );

    let mut fonts = media_text::Fonts::load()?;
    draw_line(
        &mut frame,
        &mut fonts,
        &format!("Return to a window: {RESTORE_CHORD}"),
        top + 20,
        38,
        Tint::WHITE,
    )?;
    draw_line(
        &mut frame,
        &mut fonts,
        &format!("Move to another display: {MOVE_CHORD}"),
        top + 66,
        28,
        Tint::new(0.62, 0.78, 1.0),
    )?;
    Ok(frame)
}

fn fill(frame: &mut Frame, left: u32, top: u32, width: u32, height: u32, colour: [u8; 4]) {
    for y in top..top.saturating_add(height).min(frame.size.height) {
        for x in left..left.saturating_add(width).min(frame.size.width) {
            let at = ((y * frame.size.width + x) * 4) as usize;
            frame.pixels[at..at + 4].copy_from_slice(&colour);
        }
    }
}

fn draw_line(
    frame: &mut Frame,
    fonts: &mut media_text::Fonts,
    text: &str,
    top: u32,
    height: u32,
    colour: Tint,
) -> anyhow::Result<()> {
    let style = TextStyle {
        family: "sans-serif".to_owned(),
        size: 0.5,
        alignment: Alignment::Center,
        colour,
        ..Default::default()
    };
    let rendered = media_text::render_line(
        fonts,
        text,
        &style,
        (frame.size.width / media_text::SUPERSAMPLE).max(1),
        (height / media_text::SUPERSAMPLE).max(1),
    )?;
    for y in 0..rendered.height.min(frame.size.height.saturating_sub(top)) {
        for x in 0..rendered.width.min(frame.size.width) {
            let source = ((y * rendered.width + x) * 4) as usize;
            let target = (((top + y) * frame.size.width + x) * 4) as usize;
            let source_alpha = u16::from(rendered.pixels[source + 3]);
            let background_alpha = u16::from(frame.pixels[target + 3]);
            let combined_alpha = source_alpha + background_alpha * (255 - source_alpha) / 255;
            for channel in 0..3 {
                let foreground = u16::from(rendered.pixels[source + channel]);
                let background = u16::from(frame.pixels[target + channel]);
                frame.pixels[target + channel] =
                    ((foreground * source_alpha + background * (255 - source_alpha)) / 255) as u8;
            }
            frame.pixels[target + 3] = combined_alpha.min(255) as u8;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hint_keeps_the_program_visible_around_its_panel() {
        let frame = render(Size::new(1280, 720)).expect("hint renders");
        assert_eq!(&frame.pixels[..4], &[0, 0, 0, 0]);
        assert!(frame.pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }
}
