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
        // A generic family resolves through names this machine may not carry, so a rack with
        // fonts installed can still answer "no sans-serif". Any real face beats drawing nothing.
        let fallback = pick(&database, &["sans-serif"])
            .or_else(|| database.faces().next().map(|face| face.id));
        Ok(Self {
            database,
            parsed: HashMap::new(),
            fallback,
        })
    }

    /// Builds from explicit font data. Used by tests, which must not depend on what a build
    /// machine happens to have installed.
    pub fn from_data(data: Vec<u8>) -> Result<Self, FontError> {
        let mut database = fontdb::Database::new();
        database.load_font_data(data);
        if database.is_empty() {
            return Err(FontError::NoFonts);
        }
        let fallback = database.faces().next().map(|face| face.id);
        Ok(Self {
            database,
            parsed: HashMap::new(),
            fallback,
        })
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
        let exact = requested.is_some();
        let id = requested.or(self.fallback).ok_or(FontError::NoFonts)?;

        let key = format!("{id:?}");
        if let Some(font) = self.parsed.get(&key) {
            return Ok((Arc::clone(font), exact));
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
            .flatten()
            .ok_or_else(|| FontError::Unreadable {
                family: family.to_owned(),
            })?;

        let font = Arc::new(font);
        self.parsed.insert(key, Arc::clone(&font));
        Ok((font, exact))
    }
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
