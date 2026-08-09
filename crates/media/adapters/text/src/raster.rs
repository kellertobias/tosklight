//! Turning a line of text into pixels.

use media_domain::text_catalog::{Alignment, TextStyle};

use crate::SUPERSAMPLE;
use crate::fonts::{FontError, Fonts};

/// A rasterized line, ready to upload as a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub width: u32,
    pub height: u32,
    /// Straight 8-bit RGBA, premultiplied by nothing: the compositor blends it like any source.
    pub pixels: Vec<u8>,
    /// False when the machine did not have the family and something else was used.
    pub exact_family: bool,
}

/// Draws one line into an output-sized transparent image.
///
/// Output-sized, because a text source is a source: it is the whole layer, and its position and
/// scale are the layer's business, not the renderer's. Anything else would give text a second,
/// invisible transform an operator could not see or undo.
pub fn render_line(
    fonts: &mut Fonts,
    text: &str,
    style: &TextStyle,
    output_width: u32,
    output_height: u32,
) -> Result<Rendered, FontError> {
    let style = style.clamped();
    let (font, exact_family) = fonts.resolve(&style.family)?;

    let width = (output_width * SUPERSAMPLE).max(1);
    let height = (output_height * SUPERSAMPLE).max(1);
    let mut pixels = vec![0u8; width as usize * height as usize * 4];

    // Height is a fraction of the output, so the same look survives a change of resolution.
    let pixel_size = (style.size * height as f32).max(1.0);
    let colour = [
        channel(style.colour.red),
        channel(style.colour.green),
        channel(style.colour.blue),
    ];

    let laid_out = lay_out(&font, text, pixel_size);
    let left = match style.alignment {
        Alignment::Left => 0.0,
        Alignment::Center => (width as f32 - laid_out.width) / 2.0,
        Alignment::Right => width as f32 - laid_out.width,
    };
    // Vertically centred on the line's own extent rather than the font's, so a clock and a word
    // with no descenders sit in the same place.
    let baseline = (height as f32 + laid_out.ascent - laid_out.descent) / 2.0 - laid_out.descent;

    for glyph in laid_out.glyphs {
        let (metrics, coverage) = font.rasterize(glyph.character, pixel_size);
        let origin_x = left + glyph.x + metrics.xmin as f32;
        let origin_y = baseline - metrics.ymin as f32 - metrics.height as f32;

        for row in 0..metrics.height {
            for column in 0..metrics.width {
                let alpha = coverage[row * metrics.width + column];
                if alpha == 0 {
                    continue;
                }
                let x = origin_x as i64 + column as i64;
                let y = origin_y as i64 + row as i64;
                if x < 0 || y < 0 || x >= i64::from(width) || y >= i64::from(height) {
                    continue;
                }
                let at = (y as usize * width as usize + x as usize) * 4;
                // Glyphs may overlap; keeping the strongest coverage avoids a seam where two
                // letters meet.
                if alpha > pixels[at + 3] {
                    pixels[at] = colour[0];
                    pixels[at + 1] = colour[1];
                    pixels[at + 2] = colour[2];
                    pixels[at + 3] = alpha;
                }
            }
        }
    }

    Ok(Rendered {
        width,
        height,
        pixels,
        exact_family,
    })
}

struct Placed {
    character: char,
    x: f32,
}

struct LaidOut {
    glyphs: Vec<Placed>,
    width: f32,
    ascent: f32,
    descent: f32,
}

/// Places each glyph along the line and measures the whole of it.
fn lay_out(font: &fontdue::Font, text: &str, pixel_size: f32) -> LaidOut {
    let metrics = font.horizontal_line_metrics(pixel_size);
    let mut glyphs = Vec::new();
    let mut pen = 0.0;
    for character in text.chars() {
        let advance = font.metrics(character, pixel_size).advance_width;
        glyphs.push(Placed { character, x: pen });
        pen += advance;
    }
    LaidOut {
        glyphs,
        width: pen,
        ascent: metrics.map_or(pixel_size * 0.8, |line| line.ascent),
        descent: metrics.map_or(-pixel_size * 0.2, |line| line.descent),
    }
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::Tint;

    /// A font built here rather than taken from the machine, so a build server with an unusual
    /// font set cannot change what these tests measure.
    fn fonts() -> Fonts {
        let data = std::fs::read("/System/Library/Fonts/Helvetica.ttc")
            .or_else(|_| std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"))
            .or_else(|_| std::fs::read("C:/Windows/Fonts/arial.ttf"));
        match data {
            Ok(data) => Fonts::from_data(data).expect("a real font parses"),
            // A machine with none of those still has *something*; the tests below only need a
            // font, not a particular one.
            Err(_) => Fonts::load().expect("this machine has fonts"),
        }
    }

    fn ink(rendered: &Rendered) -> usize {
        rendered
            .pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .count()
    }

    /// The horizontal centre of gravity of the ink, `0.0..1.0` across the image.
    fn centre_of_ink(rendered: &Rendered) -> f32 {
        let mut weighted = 0.0;
        let mut total = 0.0;
        for (index, pixel) in rendered.pixels.chunks_exact(4).enumerate() {
            let alpha = f32::from(pixel[3]);
            if alpha == 0.0 {
                continue;
            }
            let x = (index as u32 % rendered.width) as f32;
            weighted += x * alpha;
            total += alpha;
        }
        if total == 0.0 {
            0.5
        } else {
            weighted / total / rendered.width as f32
        }
    }

    #[test]
    fn text_is_drawn_and_empty_text_is_not() {
        let mut fonts = fonts();
        let style = TextStyle::default();

        let drawn = render_line(&mut fonts, "12:00:00", &style, 320, 180).expect("it renders");
        assert!(ink(&drawn) > 0, "a clock must put ink on the layer");

        let empty = render_line(&mut fonts, "", &style, 320, 180).expect("it renders");
        assert_eq!(ink(&empty), 0, "an empty entry draws nothing, not a box");
    }

    #[test]
    fn a_text_source_is_output_sized_so_the_layer_owns_its_placement() {
        let mut fonts = fonts();
        let rendered =
            render_line(&mut fonts, "Cue", &TextStyle::default(), 320, 180).expect("renders");
        assert_eq!(rendered.width, 320 * SUPERSAMPLE);
        assert_eq!(rendered.height, 180 * SUPERSAMPLE);
    }

    #[test]
    fn alignment_moves_the_line_rather_than_the_glyphs() {
        let mut fonts = fonts();
        let mut at = |alignment| {
            let style = TextStyle {
                alignment,
                ..Default::default()
            };
            centre_of_ink(&render_line(&mut fonts, "Stand by", &style, 320, 180).expect("renders"))
        };

        let left = at(Alignment::Left);
        let centre = at(Alignment::Center);
        let right = at(Alignment::Right);
        assert!(left < centre, "{left} < {centre}");
        assert!(centre < right, "{centre} < {right}");
        assert!(
            (centre - 0.5).abs() < 0.1,
            "centred text sits centred: {centre}"
        );
    }

    #[test]
    fn size_is_a_fraction_of_the_output_so_a_look_survives_a_resolution_change() {
        let mut fonts = fonts();
        let small = render_line(
            &mut fonts,
            "88",
            &TextStyle {
                size: 0.1,
                ..Default::default()
            },
            320,
            180,
        )
        .expect("renders");
        let large = render_line(
            &mut fonts,
            "88",
            &TextStyle {
                size: 0.4,
                ..Default::default()
            },
            320,
            180,
        )
        .expect("renders");

        assert!(
            ink(&large) > ink(&small) * 2,
            "four times the height is far more ink: {} against {}",
            ink(&large),
            ink(&small)
        );
    }

    #[test]
    fn the_style_s_colour_is_what_is_drawn() {
        let mut fonts = fonts();
        let style = TextStyle {
            colour: Tint::new(1.0, 0.0, 0.0),
            ..Default::default()
        };
        let rendered = render_line(&mut fonts, "8", &style, 64, 64).expect("renders");

        let inked = rendered
            .pixels
            .chunks_exact(4)
            .find(|pixel| pixel[3] > 0)
            .expect("something was drawn");
        assert_eq!([inked[0], inked[1], inked[2]], [255, 0, 0]);
    }

    #[test]
    fn a_family_this_machine_does_not_have_still_draws_and_says_it_substituted() {
        let mut fonts = fonts();
        let style = TextStyle {
            family: "No Such Family At All".into(),
            ..Default::default()
        };
        let rendered = render_line(&mut fonts, "Cue", &style, 320, 180).expect("it falls back");

        assert!(ink(&rendered) > 0, "an operator still sees their words");
        assert!(
            !rendered.exact_family,
            "and can be told the font was substituted"
        );
    }

    #[test]
    fn text_wider_than_the_output_is_clipped_rather_than_written_out_of_bounds() {
        let mut fonts = fonts();
        let style = TextStyle {
            size: 0.9,
            alignment: Alignment::Left,
            ..Default::default()
        };
        let rendered = render_line(
            &mut fonts,
            "a line far too long to fit across this output at all",
            &style,
            64,
            64,
        )
        .expect("renders");

        assert_eq!(
            rendered.pixels.len(),
            (64 * SUPERSAMPLE) as usize * (64 * SUPERSAMPLE) as usize * 4,
            "the buffer is exactly the image, however much text was asked for"
        );
        assert!(ink(&rendered) > 0);
    }
}
