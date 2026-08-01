//! Operator overlay: panels, labels, and meters composited over the rendered image.
//!
//! The overlay is built as plain data by the application and drawn by the render core, so the
//! connection surface and Quick Settings can be unit tested without a GPU.

use crate::font::{ADVANCE, GLYPH_HEIGHT, GLYPH_WIDTH, glyph, text_width};
use bytemuck::{Pod, Zeroable};

/// Atlas layout: 95 printable glyphs, then the extra glyphs the named views need, then one solid
/// cell used for panels and rules.
pub const ATLAS_GLYPHS: usize = 95;
/// Code points beyond ASCII that the operator surface actually uses.
const EXTRA_GLYPHS: [char; 3] = ['\u{2192}', '\u{2022}', '\u{00b0}'];
const ATLAS_CELLS: usize = ATLAS_GLYPHS + EXTRA_GLYPHS.len() + 1;
/// Square edge, in atlas pixels, reserved for the application icon below the glyph strip. The
/// icon is real artwork rather than a drawn mark, so the atlas carries colour and the glyph cells
/// are white with a coverage alpha.
pub const ICON_SIZE: usize = 128;
pub const ATLAS_WIDTH: usize = if ATLAS_CELLS * GLYPH_WIDTH > ICON_SIZE {
    ATLAS_CELLS * GLYPH_WIDTH
} else {
    ICON_SIZE
};
pub const ATLAS_HEIGHT: usize = GLYPH_HEIGHT + ICON_SIZE;
/// Row where the icon region starts.
pub const ICON_ORIGIN_Y: usize = GLYPH_HEIGHT;
const SOLID_CELL: usize = ATLAS_CELLS - 1;
/// Bottom of the glyph strip in texture coordinates. The atlas is taller than the strip because
/// the icon sits below it, so a glyph must not sample all the way down.
const GLYPH_V1: f32 = GLYPH_HEIGHT as f32 / ATLAS_HEIGHT as f32;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct OverlayQuad {
    /// `x`, `y`, `width`, `height` in physical pixels.
    pub rect: [f32; 4],
    /// `u0`, `v0`, `u1`, `v1`.
    pub uv_rect: [f32; 4],
    pub colour: [f32; 4],
}

impl OverlayQuad {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
    };
}

/// Collected overlay geometry for one frame.
#[derive(Clone, Debug, Default)]
pub struct Overlay {
    pub quads: Vec<OverlayQuad>,
}

impl Overlay {
    pub fn clear(&mut self) {
        self.quads.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.quads.is_empty()
    }

    /// Solid rectangle in physical pixels.
    pub fn rect(&mut self, x: f32, y: f32, width: f32, height: f32, colour: [f32; 4]) {
        let u0 = (SOLID_CELL * GLYPH_WIDTH) as f32 / ATLAS_WIDTH as f32;
        let u1 = ((SOLID_CELL + 1) * GLYPH_WIDTH) as f32 / ATLAS_WIDTH as f32;
        self.quads.push(OverlayQuad {
            rect: [x, y, width, height],
            uv_rect: [u0, 0.0, u1, GLYPH_V1],
            colour,
        });
    }

    /// One line of text with its top-left corner at `x`, `y`. Returns the advance width.
    pub fn text(&mut self, x: f32, y: f32, scale: f32, colour: [f32; 4], text: &str) -> f32 {
        let mut cursor = x;
        for character in text.chars() {
            if character != ' ' {
                let index = glyph_index(character);
                let u0 = (index * GLYPH_WIDTH) as f32 / ATLAS_WIDTH as f32;
                let u1 = ((index + 1) * GLYPH_WIDTH) as f32 / ATLAS_WIDTH as f32;
                self.quads.push(OverlayQuad {
                    rect: [
                        cursor,
                        y,
                        GLYPH_WIDTH as f32 * scale,
                        GLYPH_HEIGHT as f32 * scale,
                    ],
                    uv_rect: [u0, 0.0, u1, GLYPH_V1],
                    colour,
                });
            }
            cursor += ADVANCE as f32 * scale;
        }
        cursor - x
    }

    /// One line of text that stops at `limit`, so a long line can never run under whatever the
    /// status bar draws to its right. Returns the advance width actually used.
    pub fn clipped_text(
        &mut self,
        x: f32,
        y: f32,
        scale: f32,
        colour: [f32; 4],
        text: &str,
        limit: f32,
    ) -> f32 {
        if Overlay::measure(text, scale) + x <= limit {
            return self.text(x, y, scale, colour, text);
        }
        let advance = ADVANCE as f32 * scale;
        let room = (((limit - x) / advance).floor() as isize - 1).max(0) as usize;
        let mut clipped: String = text.chars().take(room).collect();
        if room > 0 && clipped.chars().count() < text.chars().count() {
            clipped.push('\u{2026}');
        }
        self.text(x, y, scale, colour, &clipped)
    }

    /// One line of text with more weight, for a label that has to win against the drawing under
    /// it. The bitmap font has one weight, so the run is drawn twice a pixel apart.
    pub fn bold_text(&mut self, x: f32, y: f32, scale: f32, colour: [f32; 4], text: &str) -> f32 {
        self.text(x, y, scale, colour, text);
        self.text(x + scale.max(1.0), y, scale, colour, text)
    }

    /// A rectangle with rounded corners, for a badge.
    pub fn rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        colour: [f32; 4],
    ) {
        let radius = radius.min(width * 0.5).min(height * 0.5).max(0.0);
        if radius <= 0.5 {
            self.rect(x, y, width, height, colour);
            return;
        }
        self.rect(x + radius, y, width - radius * 2.0, height, colour);
        self.rect(x, y + radius, radius, height - radius * 2.0, colour);
        self.rect(
            x + width - radius,
            y + radius,
            radius,
            height - radius * 2.0,
            colour,
        );
        for (corner_x, corner_y) in [
            (x + radius, y + radius),
            (x + width - radius, y + radius),
            (x + radius, y + height - radius),
            (x + width - radius, y + height - radius),
        ] {
            self.disc(corner_x, corner_y, radius, colour);
        }
    }

    /// A rounded badge: an outline in one colour with a fill in another.
    ///
    /// Positional geometry the whole way, like every other primitive here: a rectangle, a corner
    /// radius and two colours. Naming the halves of that in a struct would read worse at the call
    /// sites, which are drawing shapes rather than describing them.
    #[allow(clippy::too_many_arguments)]
    pub fn badge(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        outline: [f32; 4],
        fill: [f32; 4],
    ) {
        let stroke = (height * 0.09).clamp(1.0, 2.0);
        self.rounded_rect(x, y, width, height, radius, outline);
        self.rounded_rect(
            x + stroke,
            y + stroke,
            width - stroke * 2.0,
            height - stroke * 2.0,
            (radius - stroke).max(0.0),
            fill,
        );
    }

    /// A small filled disc, for the colour a lamp is currently emitting.
    pub fn disc(&mut self, centre_x: f32, centre_y: f32, radius: f32, colour: [f32; 4]) {
        // Drawn as horizontal spans of the solid cell: a handful of rectangles reads as a dot at
        // the sizes the plan uses, and needs no second pipeline.
        let steps = 7;
        for step in 0..steps {
            let t = (step as f32 + 0.5) / steps as f32 * 2.0 - 1.0;
            let half_width = radius * (1.0 - t * t).max(0.0).sqrt();
            let height = radius * 2.0 / steps as f32;
            self.rect(
                centre_x - half_width,
                centre_y + t * radius - height * 0.5,
                half_width * 2.0,
                height,
                colour,
            );
        }
    }

    /// The application icon, drawn at its own colours. `tint` is normally opaque white.
    pub fn icon(&mut self, x: f32, y: f32, size: f32, tint: [f32; 4]) {
        self.quads.push(OverlayQuad {
            rect: [x, y, size, size],
            uv_rect: [
                0.0,
                ICON_ORIGIN_Y as f32 / ATLAS_HEIGHT as f32,
                ICON_SIZE as f32 / ATLAS_WIDTH as f32,
                (ICON_ORIGIN_Y + ICON_SIZE) as f32 / ATLAS_HEIGHT as f32,
            ],
            colour: tint,
        });
    }

    /// Horizontal bar meter, used for fog amount and level readouts.
    ///
    /// Positional geometry, for the same reason [`Self::badge`] is.
    #[allow(clippy::too_many_arguments)]
    pub fn meter(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fraction: f32,
        track: [f32; 4],
        fill: [f32; 4],
    ) {
        self.rect(x, y, width, height, track);
        let filled = width * fraction.clamp(0.0, 1.0);
        if filled > 0.5 {
            self.rect(x, y, filled, height, fill);
        }
    }

    /// Width in pixels of `text` at `scale`.
    pub fn measure(text: &str, scale: f32) -> f32 {
        text_width(text) as f32 * scale
    }

    /// Line height in pixels at `scale`, including leading.
    pub fn line_height(scale: f32) -> f32 {
        (GLYPH_HEIGHT as f32 + 4.0) * scale
    }
}

fn glyph_index(character: char) -> usize {
    let code = character as u32;
    if (0x20..=0x7e).contains(&code) {
        return (code - 0x20) as usize;
    }
    if let Some(extra) = EXTRA_GLYPHS
        .iter()
        .position(|candidate| *candidate == character)
    {
        return ATLAS_GLYPHS + extra;
    }
    // Anything else reuses the substitute glyph the font provides.
    (b'?' - 0x20) as usize
}

/// Rasterise the font atlas: one `RGBA8` row-major image, glyphs and the solid cell in the top
/// strip and the application icon below them. Glyph texels are white so a tint colours them;
/// the icon carries its own colours and is drawn with a white tint.
pub fn build_atlas(icon: Option<&[u8]>) -> Vec<u8> {
    let mut pixels = vec![0_u8; ATLAS_WIDTH * ATLAS_HEIGHT * 4];
    let mut set = |x: usize, y: usize, texel: [u8; 4]| {
        let offset = (y * ATLAS_WIDTH + x) * 4;
        pixels[offset..offset + 4].copy_from_slice(&texel);
    };
    for index in 0..ATLAS_GLYPHS + EXTRA_GLYPHS.len() {
        let character = if index < ATLAS_GLYPHS {
            char::from_u32(0x20 + index as u32).unwrap_or(' ')
        } else {
            EXTRA_GLYPHS[index - ATLAS_GLYPHS]
        };
        let columns = glyph(character);
        for (column_index, column) in columns.iter().enumerate() {
            for row in 0..GLYPH_HEIGHT {
                if column & (1 << row) != 0 {
                    set(index * GLYPH_WIDTH + column_index, row, [0xff; 4]);
                }
            }
        }
    }
    for row in 0..GLYPH_HEIGHT {
        for column in 0..GLYPH_WIDTH {
            set(SOLID_CELL * GLYPH_WIDTH + column, row, [0xff; 4]);
        }
    }
    if let Some(icon) = icon {
        // Anything but a complete square of `RGBA8` is a packaging mistake, and a missing icon is
        // better than a torn one, so a wrong length simply leaves the region empty.
        if icon.len() == ICON_SIZE * ICON_SIZE * 4 {
            for row in 0..ICON_SIZE {
                for column in 0..ICON_SIZE {
                    let offset = (row * ICON_SIZE + column) * 4;
                    let texel = [
                        icon[offset],
                        icon[offset + 1],
                        icon[offset + 2],
                        icon[offset + 3],
                    ];
                    set(column, ICON_ORIGIN_Y + row, texel);
                }
            }
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_atlas_has_a_fully_solid_cell_for_panels() {
        let atlas = build_atlas(None);
        assert_eq!(atlas.len(), ATLAS_WIDTH * ATLAS_HEIGHT * 4);
        for row in 0..GLYPH_HEIGHT {
            for column in 0..GLYPH_WIDTH {
                let offset = (row * ATLAS_WIDTH + SOLID_CELL * GLYPH_WIDTH + column) * 4;
                assert_eq!(&atlas[offset..offset + 4], &[0xff; 4]);
            }
        }
    }

    #[test]
    fn text_emits_one_quad_per_visible_character() {
        let mut overlay = Overlay::default();
        overlay.text(0.0, 0.0, 2.0, [1.0; 4], "AB C");
        assert_eq!(overlay.quads.len(), 3);
    }

    #[test]
    fn a_meter_draws_a_track_and_a_proportional_fill() {
        let mut overlay = Overlay::default();
        overlay.meter(0.0, 0.0, 100.0, 6.0, 0.5, [0.1; 4], [1.0; 4]);
        assert_eq!(overlay.quads.len(), 2);
        assert_eq!(overlay.quads[1].rect[2], 50.0);
        overlay.clear();
        overlay.meter(0.0, 0.0, 100.0, 6.0, 0.0, [0.1; 4], [1.0; 4]);
        assert_eq!(
            overlay.quads.len(),
            1,
            "an empty meter draws only its track"
        );
    }
}
