//! Finding a font on this machine.
//!
//! Nothing is bundled. An operator names a family and the machine's own fonts are searched, which
//! is why the same show can look right on a Mac and a Linux rack: the family is the intent, and
//! the fallback is what makes an absent one survivable rather than fatal.

use std::collections::HashMap;
use std::sync::Arc;

/// Why text cannot be drawn.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FontError {
    #[error("this machine has no usable fonts installed")]
    NoFonts,
    #[error("the font data for {family} could not be read")]
    Unreadable { family: String },
}

/// The machine's fonts, loaded once.
///
/// Scanning a system's font directories is slow — hundreds of files — so it happens at startup and
/// the result is kept. A family that is asked for repeatedly is parsed once and cached, because a
/// clock re-renders every second and reparsing a font each time would be absurd.
pub struct Fonts {
    database: fontdb::Database,
    parsed: HashMap<String, Arc<fontdue::Font>>,
    /// What an unknown family falls back to. Chosen once, so a show does not change appearance
    /// halfway through because a different fallback happened to match.
    fallback: Option<fontdb::ID>,
}

impl std::fmt::Debug for Fonts {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Fonts")
            .field("faces", &self.database.len())
            .finish()
    }
}

impl Fonts {
    /// Loads what this machine has.
    pub fn load() -> Result<Self, FontError> {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        if database.is_empty() {
            return Err(FontError::NoFonts);
        }
        Ok(Self::from_database(database))
    }

    fn from_database(database: fontdb::Database) -> Self {
        let fallback = pick(&database, &["sans-serif"])
            .or_else(|| database.faces().next().map(|face| face.id));
        Self {
            database,
            parsed: HashMap::new(),
            fallback,
        }
    }

    /// Builds from explicit font data. Used by tests, which must not depend on what a build
    /// machine happens to have installed.
    pub fn from_data(data: Vec<u8>) -> Result<Self, FontError> {
        let mut database = fontdb::Database::new();
        database.load_font_data(data);
        if database.is_empty() {
            return Err(FontError::NoFonts);
        }
        Ok(Self::from_database(database))
    }

    pub fn is_empty(&self) -> bool {
        self.database.is_empty()
    }

    /// The font to draw a family with, falling back when the machine does not have it.
    ///
    /// Returns the font and whether it is the one asked for, so a caller can tell an operator
    /// their font was substituted instead of leaving them wondering why it looks wrong.
    pub fn resolve(&mut self, family: &str) -> Result<(Arc<fontdue::Font>, bool), FontError> {
        let requested = pick(&self.database, &[family]);
        let generic = matches!(family, "sans-serif" | "serif" | "monospace");
        if let Some(id) = requested
            && let Some(font) = self.parse(id)
            && (!generic || supports_text(&font))
        {
            return Ok((font, true));
        }

        if let Some(id) = self.fallback
            && let Some(font) = self.parse(id)
            && supports_text(&font)
        {
            return Ok((font, false));
        }

        // Generic aliases can name an uninstalled family. The first installed face may
        // instead be emoji/symbol-only. Prefer actual text coverage for default fallback;
        // explicitly selected symbol or non-Latin families remain valid above.
        let candidates = self
            .fallback
            .into_iter()
            .chain(self.database.faces().map(|face| face.id))
            .collect::<Vec<_>>();
        let mut readable = None;
        for id in candidates {
            if let Some(font) = self.parse(id) {
                if supports_text(&font) {
                    self.fallback = Some(id);
                    return Ok((font, false));
                }
                readable.get_or_insert(font);
            }
        }
        readable.map(|font| (font, false)).ok_or(FontError::NoFonts)
    }

    fn parse(&mut self, id: fontdb::ID) -> Option<Arc<fontdue::Font>> {
        let key = format!("{id:?}");
        if let Some(font) = self.parsed.get(&key) {
            return Some(Arc::clone(font));
        }
        let font = self
            .database
            .with_face_data(id, |data, index| {
                fontdue::Font::from_bytes(
                    data,
                    fontdue::FontSettings {
                        collection_index: index,
                        ..Default::default()
                    },
                )
                .ok()
            })
            .flatten()?;
        let font = Arc::new(font);
        self.parsed.insert(key, Arc::clone(&font));
        Some(font)
    }
}

fn supports_text(font: &fontdue::Font) -> bool {
    ['A', 'W', '0']
        .into_iter()
        .all(|glyph| font.lookup_glyph_index(glyph) != 0)
}

/// The first face matching any of these families.
fn pick(database: &fontdb::Database, families: &[&str]) -> Option<fontdb::ID> {
    let query = fontdb::Query {
        families: &families
            .iter()
            .map(|family| match *family {
                "sans-serif" => fontdb::Family::SansSerif,
                "serif" => fontdb::Family::Serif,
                "monospace" => fontdb::Family::Monospace,
                other => fontdb::Family::Name(other),
            })
            .collect::<Vec<_>>(),
        ..fontdb::Query::default()
    };
    database.query(&query)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal_font() -> Vec<u8> {
        std::fs::read("/System/Library/Fonts/Helvetica.ttc")
            .or_else(|_| std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"))
            .or_else(|_| std::fs::read("C:/Windows/Fonts/arial.ttf"))
            .expect("the text test fixture requires Helvetica, DejaVu Sans or Arial")
    }

    #[test]
    fn missing_generic_skips_symbol_only_fallback_but_explicit_symbols_still_work() {
        let mut database = fontdb::Database::new();
        database.load_font_data(include_bytes!("../tests/fixtures/symbol-only.ttf").to_vec());
        database.load_font_data(normal_font());
        database.set_sans_serif_family("Missing generic family");
        let mut fonts = Fonts::from_database(database);
        let first = fonts.resolve("Fallback Symbol Fixture").unwrap().0;
        assert_eq!(
            first.lookup_glyph_index('W'),
            0,
            "symbol face really is first"
        );
        for family in ["sans-serif", "Absent Cue Font"] {
            let (font, exact) = fonts.resolve(family).unwrap();
            assert!(!exact);
            assert!(font.lookup_glyph_index('W') != 0);
            assert!(
                font.rasterize('W', 48.0)
                    .1
                    .iter()
                    .any(|coverage| *coverage > 0)
            );
        }
        let (symbols, exact) = fonts.resolve("Fallback Symbol Fixture").unwrap();
        assert!(exact);
        assert_eq!(symbols.lookup_glyph_index('W'), 0);
        assert_ne!(symbols.lookup_glyph_index('★'), 0);
    }
}
