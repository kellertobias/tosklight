//! Real multiline glyphs fitted into an atlas. The aspect metadata lets the GPU fit each
//! entry proportionally again as its grid cell changes size, without rerasterizing on beats.
use crate::Fonts;
use media_domain::slice_grid::{
    SLICE_TEXT_HEIGHT, SLICE_TEXT_WIDTH, SliceGridTypography, SliceTextAtlas,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock, Weak},
};

#[derive(Default)]
struct Cache {
    fonts: Option<Fonts>,
    attempted: bool,
    entries: BTreeMap<String, Weak<SliceTextAtlas>>,
}
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}
fn load_fonts(cache: &mut Cache) {
    if !cache.attempted {
        cache.fonts = Fonts::load().ok();
        cache.attempted = true;
    }
}
pub fn slice_font_status(family: &str) -> String {
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    load_fonts(&mut cache);
    match cache
        .fonts
        .as_mut()
        .and_then(|fonts| fonts.resolve(family).ok())
    {
        Some((_, true)) => format!("Using {family}"),
        Some((_, false)) => {
            format!("{family} is unavailable on the Media server; using its system fallback font.")
        }
        None => "Text cannot render: no usable fonts are installed on the Media server.".to_owned(),
    }
}
pub fn slice_text_atlas(typography: &SliceGridTypography) -> Arc<SliceTextAtlas> {
    let key = format!("{}\0{:?}", typography.font, typography.texts);
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(atlas) = cache.entries.get(&key).and_then(Weak::upgrade) {
        return atlas;
    }
    load_fonts(&mut cache);
    let mut atlas = SliceTextAtlas {
        pixels: vec![
            0;
            (SLICE_TEXT_WIDTH * SLICE_TEXT_HEIGHT * 4) as usize * typography.texts.len()
        ],
        aspects: Vec::new(),
    };
    if let Some(fonts) = &mut cache.fonts {
        for (index, text) in typography.texts.iter().enumerate() {
            let row_bytes = (SLICE_TEXT_WIDTH * SLICE_TEXT_HEIGHT * 4) as usize;
            let aspect = raster_entry(
                fonts,
                &typography.font,
                text,
                &mut atlas.pixels[index * row_bytes..(index + 1) * row_bytes],
            );
            atlas.aspects.push(aspect);
        }
    }
    let atlas = Arc::new(atlas);
    if cache.entries.len() >= 32 {
        cache.entries.clear();
    }
    cache.entries.retain(|_, weak| weak.strong_count() > 0);
    cache.entries.insert(key, Arc::downgrade(&atlas));
    atlas
}
fn raster_entry(fonts: &mut Fonts, family: &str, text: &str, target: &mut [u8]) -> f32 {
    let Ok((font, _)) = fonts.resolve(family) else {
        return 1.0;
    };
    let size = 96.0;
    let metrics = font.horizontal_line_metrics(size);
    let ascent = metrics.map_or(size, |m| m.ascent);
    let line_height = metrics.map_or(size * 1.2, |m| m.new_line_size.max(size));
    let lines: Vec<_> = text.split('\n').collect();
    let widths: Vec<f32> = lines
        .iter()
        .map(|line| {
            line.chars()
                .map(|c| font.metrics(c, size).advance_width)
                .sum()
        })
        .collect();
    let width = widths.iter().copied().fold(1.0_f32, f32::max);
    let height = (lines.len() as f32 * line_height).max(1.0);
    let mut glyphs = Vec::new();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0.0_f32, 0.0_f32, width, height);
    for (line_index, line) in lines.iter().enumerate() {
        let mut pen = (width - widths[line_index]) / 2.0;
        let baseline = line_index as f32 * line_height + ascent;
        for character in line.chars() {
            let (m, coverage) = font.rasterize(character, size);
            let left = pen + m.xmin as f32;
            let top = baseline - m.ymin as f32 - m.height as f32;
            min_x = min_x.min(left);
            min_y = min_y.min(top);
            max_x = max_x.max(left + m.width as f32);
            max_y = max_y.max(top + m.height as f32);
            pen += m.advance_width;
            glyphs.push((m, coverage, left, top));
        }
    }
    // Include logical whitespace AND every ink overhang, accent and descender before fitting.
    min_x -= 2.0;
    min_y -= 2.0;
    max_x += 2.0;
    max_y += 2.0;
    let width = (max_x - min_x).max(1.0);
    let height = (max_y - min_y).max(1.0);
    for (m, coverage, left, top) in glyphs {
        let left = left - min_x;
        let top = top - min_y;
        let sx = SLICE_TEXT_WIDTH as f32 / width;
        let sy = SLICE_TEXT_HEIGHT as f32 / height;
        let x0 = (left * sx).floor().max(0.0) as u32;
        let y0 = (top * sy).floor().max(0.0) as u32;
        let x1 = ((left + m.width as f32) * sx)
            .ceil()
            .clamp(0.0, SLICE_TEXT_WIDTH as f32) as u32;
        let y1 = ((top + m.height as f32) * sy)
            .ceil()
            .clamp(0.0, SLICE_TEXT_HEIGHT as f32) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let gx = (((x as f32 + 0.5) / sx - left).max(0.0) as usize)
                    .min(m.width.saturating_sub(1));
                let gy = (((y as f32 + 0.5) / sy - top).max(0.0) as usize)
                    .min(m.height.saturating_sub(1));
                if m.width > 0 && m.height > 0 {
                    let a = coverage[gy * m.width + gx];
                    let at = ((y * SLICE_TEXT_WIDTH + x) * 4) as usize;
                    target[at..at + 3].fill(255);
                    target[at + 3] = target[at + 3].max(a);
                }
            }
        }
    }
    width / height
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accents_bearings_and_descenders_fit_inside_the_atlas() {
        let typography = SliceGridTypography {
            texts: vec!["Áj Ŷf\ngý ſ".into()],
            font: "serif".into(),
            ..Default::default()
        };
        let atlas = slice_text_atlas(&typography);
        let alpha = |x: u32, y: u32| atlas.pixels[((y * SLICE_TEXT_WIDTH + x) * 4 + 3) as usize];
        assert!(
            (0..SLICE_TEXT_WIDTH).all(|x| alpha(x, 0) == 0 && alpha(x, SLICE_TEXT_HEIGHT - 1) == 0)
        );
        assert!(
            (0..SLICE_TEXT_HEIGHT).all(|y| alpha(0, y) == 0 && alpha(SLICE_TEXT_WIDTH - 1, y) == 0)
        );
        assert!((0..SLICE_TEXT_HEIGHT / 2).any(|y| (0..SLICE_TEXT_WIDTH).any(|x| alpha(x, y) > 0)));
        assert!(
            (SLICE_TEXT_HEIGHT / 2..SLICE_TEXT_HEIGHT)
                .any(|y| (0..SLICE_TEXT_WIDTH).any(|x| alpha(x, y) > 0))
        );
    }
    #[test]
    fn multiline_has_separate_lines_aspect_and_cached_font_specific_entries() {
        let single = SliceGridTypography {
            texts: vec!["WIDE WORD".into()],
            ..Default::default()
        };
        let multiline = SliceGridTypography {
            texts: vec!["WIDE\nWORD".into(), "SECOND".into()],
            ..Default::default()
        };
        let a = slice_text_atlas(&single);
        let b = slice_text_atlas(&multiline);
        assert_eq!(b.aspects.len(), 2);
        assert!(a.aspects[0] > b.aspects[0] * 2.0);
        let row = (SLICE_TEXT_WIDTH * SLICE_TEXT_HEIGHT * 4) as usize;
        assert!(b.pixels[..row].chunks_exact(4).any(|p| p[3] > 0));
        assert!(b.pixels[row..].chunks_exact(4).any(|p| p[3] > 0));
        assert!(Arc::ptr_eq(&b, &slice_text_atlas(&multiline)));
        let serif = SliceGridTypography {
            font: "serif".into(),
            ..multiline
        };
        assert!(
            !Arc::ptr_eq(&b, &slice_text_atlas(&serif)),
            "font requests have separate cache entries even if this host resolves them to the same fallback face"
        );
    }
}
