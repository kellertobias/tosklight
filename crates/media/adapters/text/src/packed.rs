//! Cached real font glyphs, packed as a compact monochrome mask. Cell typography deliberately
//! stretches the line to its panel, matching the condensed/expanded poster treatment.
use crate::{Fonts, render_line};
use media_domain::text_catalog::TextStyle;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

pub type TextMask = [[u32; 4]; 128];
#[derive(Default)]
struct Cache {
    fonts: Option<Fonts>,
    entries: BTreeMap<String, TextMask>,
}

pub fn packed_line_mask(text: &str) -> TextMask {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(Cache::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(mask) = cache.entries.get(text) {
        return *mask;
    }
    let mut result = [[0; 4]; 128];
    if text.is_empty() {
        return result;
    }
    if cache.fonts.is_none() {
        cache.fonts = Fonts::load().ok();
    }
    let Some(fonts) = &mut cache.fonts else {
        return result;
    };
    // Render a long enough line before fitting, so 64 characters never clip.
    let style = TextStyle {
        family: "Arial Black".to_owned(),
        size: 0.75,
        bold: true,
        ..TextStyle::default()
    };
    let Ok(rendered) = render_line(fonts, text, &style, 8192, 96) else {
        return result;
    };
    pack(
        &rendered.pixels,
        rendered.width,
        rendered.height,
        &mut result,
    );
    if cache.entries.len() >= 32 {
        cache.entries.clear();
    }
    cache.entries.insert(text.to_owned(), result);
    result
}

fn pack(pixels: &[u8], width: u32, height: u32, result: &mut TextMask) {
    let mut bounds = (width, height, 0, 0);
    for y in 0..height {
        for x in 0..width {
            if pixels[((y * width + x) * 4 + 3) as usize] >= 96 {
                bounds.0 = bounds.0.min(x);
                bounds.1 = bounds.1.min(y);
                bounds.2 = bounds.2.max(x + 1);
                bounds.3 = bounds.3.max(y + 1);
            }
        }
    }
    if bounds.0 >= bounds.2 || bounds.1 >= bounds.3 {
        return;
    }
    for y in 0..64 {
        for x in 0..256 {
            let source_x = bounds.0 + x * (bounds.2 - bounds.0) / 256;
            let source_y = bounds.1 + y * (bounds.3 - bounds.1) / 64;
            if pixels[((source_y * width + source_x) * 4 + 3) as usize] >= 96 {
                let bit = (y * 256 + x) as usize;
                result[bit / 128][(bit / 32) % 4] |= 1 << (bit % 32);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mask_packing_preserves_glyph_edges_and_blank_gaps() {
        let mut result = [[0; 4]; 128];
        pack(
            &[255, 255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255],
            3,
            1,
            &mut result,
        );
        assert_eq!(result[0][0], u32::MAX);
        assert_eq!(result[0][3], 0);
        assert_eq!(result[1][3], u32::MAX);
    }
}
