//! Migrating the legacy application's text sources.
//!
//! An operator's text is `media/.text-sources.json` in the C++ installation: a map from a DMX slot
//! to a source with rich-text spans, a countdown length, a clock format, and inline `[clock]` and
//! `[countdown]` tokens the legacy renderer substituted. None of that survives unchanged, and
//! cutover must not quietly lose it.
//!
//! So this module maps what maps and *says* what it simplified. Every departure from the stored
//! document is reported as a [`Note`] rather than logged and forgotten, because an operator has to
//! be able to look at their show and know which parts of it were changed for them.
//!
//! Three differences matter.
//!
//! **Addressing.** The legacy server put every text source in folder `200` and read the file byte
//! as `slot - 200`, so slot `201` was `(200, 1)` and slot `200` was `(200, 0)`. File zero is a
//! blank sentinel in every bank of the current contract, so slot `200` has no address it can keep.
//! Its content is preserved at the first free file in the bank and the move is reported: a desk
//! addressing it has to be repatched, and that is better than losing what it said.
//!
//! **Rich text.** A source could carry several spans with their own sizes and weights. A text slot
//! now has one style, so the spans are joined — respecting their line breaks — and the first span's
//! weight and slant become the slot's.
//!
//! **Tokens.** `[countdown]` and `[clock]` inside static text were substituted as the layer drew.
//! Text now *is* a clock or a countdown rather than containing one. A source whose text is only a
//! token becomes that kind exactly; one that wraps a token in other words becomes that kind and the
//! surrounding words are reported as dropped — a live countdown is what the operator wanted, and
//! literal brackets on a screen during a show is a visible failure.

use std::time::Duration;

use media_domain::MediaAddress;
use media_domain::color::Tint;
use media_domain::text::{TextEntry, TextKind};
use media_domain::text_catalog::{
    Alignment, CATALOG_VERSION, DEFAULT_BANK, TextCatalog, TextSlot, TextStyle,
};
use serde::Deserialize;

/// The height the legacy application drew text at, in a 1080-line output.
///
/// Sizes were pixels there and are a fraction of the output's height here, so a look survives a
/// change of resolution. Dividing by the height the legacy server always used is what makes a
/// migrated slot the same size on the screen it was designed on.
const LEGACY_OUTPUT_HEIGHT: f32 = 1080.0;

/// One thing the migration had to change, in words an operator can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The legacy slot this is about.
    pub slot: u16,
    pub detail: String,
}

impl Note {
    fn new(slot: u16, detail: impl Into<String>) -> Self {
        Self {
            slot,
            detail: detail.into(),
        }
    }
}

/// The file the legacy application kept its text sources in, inside the library root.
pub const LEGACY_TEXT_DOCUMENT: &str = ".text-sources.json";

/// What a migration produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Migrated {
    pub catalog: TextCatalog,
    /// Every simplification, in slot order. Empty means nothing was lost.
    pub notes: Vec<Note>,
}

/// One span of a legacy rich-text source.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySpan {
    #[serde(default)]
    text: String,
    /// A span carried its own size. The slot has one now, so the first span's is the one kept.
    #[serde(default)]
    font_size: Option<f32>,
    #[serde(default)]
    font_weight: String,
    #[serde(default)]
    font_style: String,
    #[serde(default)]
    new_line: bool,
}

/// One legacy text source. Unknown fields are ignored: this reads an older document, and refusing
/// it over a field this build has never heard of would strand an installation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySource {
    #[serde(default = "yes")]
    enabled: bool,
    /// `static`, `countdown`, or `clock`. Deprecated in the legacy application itself, where the
    /// content's tokens decided what a source did.
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    is_rich_text: bool,
    #[serde(default)]
    spans: Vec<LegacySpan>,
    #[serde(default)]
    countdown_duration: f64,
    /// `on_visible` or `target_datetime`.
    #[serde(default)]
    countdown_mode: String,
    /// A time of day, `HH:MM:SS`. Empty when the source counts a length instead.
    #[serde(default)]
    target_datetime: String,
    #[serde(default = "default_family")]
    font_family: String,
    #[serde(default = "default_size")]
    font_size: f32,
    #[serde(default = "default_alignment")]
    alignment: String,
}

const fn yes() -> bool {
    true
}

fn default_family() -> String {
    "sans-serif".to_owned()
}

const fn default_size() -> f32 {
    72.0
}

fn default_alignment() -> String {
    "center".to_owned()
}

/// Why a legacy text document could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyTextError {
    #[error("the legacy text document is not readable JSON: {detail}")]
    Malformed { detail: String },
}

/// Reads a legacy `.text-sources.json` into a text catalog.
///
/// `now_unix_millis` anchors a countdown that targets a time of day: the legacy document stores
/// only `HH:MM:SS`, so the date has to come from the machine reading it, and the next occurrence of
/// that time is what an operator means by it.
pub fn migrate(serialized: &str, now_unix_millis: i64) -> Result<Migrated, LegacyTextError> {
    let document: std::collections::BTreeMap<String, serde_json::Value> =
        serde_json::from_str(serialized).map_err(|error| LegacyTextError::Malformed {
            detail: error.to_string(),
        })?;

    let mut catalog = TextCatalog {
        version: CATALOG_VERSION,
        slots: Vec::new(),
    };
    let mut notes = Vec::new();
    let mut unaddressable = Vec::new();

    for (key, value) in document {
        let Ok(slot) = key.parse::<u16>() else {
            continue;
        };
        let source: LegacySource = match serde_json::from_value(value) {
            Ok(source) => source,
            Err(error) => {
                notes.push(Note::new(
                    slot,
                    format!("this source could not be read and was left out: {error}"),
                ));
                continue;
            }
        };

        let (entry, mut source_notes) = entry_of(slot, &source, now_unix_millis);
        let style = style_of(&source);
        notes.append(&mut source_notes);

        let file = slot.saturating_sub(u16::from(DEFAULT_BANK));
        let name = name_of(slot, &entry);
        let slotted = TextSlot {
            address: MediaAddress::new(DEFAULT_BANK, 0),
            name,
            entry,
            style,
        };
        // Slot 200 lands on file zero, which is blank in every bank now. It is placed after
        // everything that has a real address, so it takes a file nothing else wanted.
        match u8::try_from(file) {
            Ok(0) => unaddressable.push((slot, slotted)),
            Ok(file) => {
                let mut placed = slotted;
                placed.address = MediaAddress::new(DEFAULT_BANK, file);
                if catalog.assign(placed).is_err() {
                    notes.push(Note::new(
                        slot,
                        "another source already answers at that address",
                    ));
                }
            }
            Err(_) => notes.push(Note::new(
                slot,
                "this slot is outside the range a text bank can address and was left out",
            )),
        }
    }

    for (slot, mut moved) in unaddressable {
        match first_free_file(&catalog) {
            Some(file) => {
                moved.address = MediaAddress::new(DEFAULT_BANK, file);
                notes.push(Note::new(
                    slot,
                    format!(
                        "moved to {DEFAULT_BANK}/{file:03}, because file 0 is a blank sentinel in \
                         every bank now; a desk addressing it has to be repatched"
                    ),
                ));
                let _ = catalog.assign(moved);
            }
            None => notes.push(Note::new(
                slot,
                "the text bank is full, so this source could not be kept",
            )),
        }
    }

    notes.sort_by_key(|note| note.slot);
    Ok(Migrated { catalog, notes })
}

/// What the source becomes, and what that cost.
fn entry_of(slot: u16, source: &LegacySource, now_unix_millis: i64) -> (TextEntry, Vec<Note>) {
    let mut notes = Vec::new();
    let text = flatten(source, &mut notes, slot);

    let (kind, token) = match token_in(&text) {
        Some(Token::Clock) => (TextKind::Clock, Some("[clock]")),
        Some(Token::Countdown) => (countdown_of(source, now_unix_millis), Some("[countdown]")),
        None => match source.r#type.as_str() {
            "clock" => (TextKind::Clock, None),
            "countdown" => (countdown_of(source, now_unix_millis), None),
            _ => (TextKind::Static { text: text.clone() }, None),
        },
    };

    if let Some(token) = token {
        let stripped = strip_token(&text);
        if !stripped.trim().is_empty() {
            notes.push(Note::new(
                slot,
                format!(
                    "the words around {token} were dropped: text is a countdown or a clock now \
                     rather than containing one, and “{}” cannot be drawn beside it",
                    stripped.trim()
                ),
            ));
        }
    }

    let mut entry = TextEntry::new(kind);
    entry.enabled = source.enabled;
    (entry, notes)
}

enum Token {
    Clock,
    Countdown,
}

/// The first substitution token in a legacy text, if it has one.
fn token_in(text: &str) -> Option<Token> {
    let clock = text.find("[clock");
    let countdown = text.find("[countdown");
    match (clock, countdown) {
        (Some(at_clock), Some(at_countdown)) if at_clock < at_countdown => Some(Token::Clock),
        (_, Some(_)) => Some(Token::Countdown),
        (Some(_), None) => Some(Token::Clock),
        (None, None) => None,
    }
}

/// The text with every token removed, which is what would have been lost.
fn strip_token(text: &str) -> String {
    let mut left = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        left.push_str(&rest[..start]);
        match rest[start..].find(']') {
            Some(end) => rest = &rest[start + end + 1..],
            None => {
                left.push_str(&rest[start..]);
                return left;
            }
        }
    }
    left.push_str(rest);
    left
}

fn countdown_of(source: &LegacySource, now_unix_millis: i64) -> TextKind {
    let target = source.target_datetime.trim();
    if !target.is_empty()
        && source.countdown_mode != "on_visible"
        && let Some(target_unix_millis) = next_occurrence(target, now_unix_millis)
    {
        return TextKind::CountdownToTarget { target_unix_millis };
    }
    TextKind::CountdownFromDuration {
        duration: Duration::from_secs_f64(source.countdown_duration.max(0.0)),
    }
}

/// The next time today or tomorrow that the clock reads `HH:MM:SS`.
///
/// The legacy document stores a time of day with no date, so a date has to be chosen. The next
/// occurrence is what an operator means by "count down to 20:30".
fn next_occurrence(time_of_day: &str, now_unix_millis: i64) -> Option<i64> {
    let mut parts = time_of_day.split(':');
    let hours: i64 = parts.next()?.trim().parse().ok()?;
    let minutes: i64 = parts.next()?.trim().parse().ok()?;
    let seconds: i64 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
    if !(0..24).contains(&hours) || !(0..60).contains(&minutes) || !(0..60).contains(&seconds) {
        return None;
    }

    const DAY: i64 = 24 * 60 * 60 * 1000;
    let wanted = ((hours * 60 + minutes) * 60 + seconds) * 1000;
    // Milliseconds since midnight UTC. The legacy document carries no zone either, so the two
    // agree on treating the stored time as the one the machine reads.
    let midnight = now_unix_millis.div_euclid(DAY) * DAY;
    let today = midnight + wanted;
    Some(if today > now_unix_millis {
        today
    } else {
        today + DAY
    })
}

/// One string from however many spans the source had.
fn flatten(source: &LegacySource, notes: &mut Vec<Note>, slot: u16) -> String {
    if !source.is_rich_text || source.spans.is_empty() {
        return source.text.clone();
    }

    let mut text = String::new();
    for (index, span) in source.spans.iter().enumerate() {
        if span.new_line && index > 0 {
            text.push('\n');
        }
        text.push_str(&span.text);
    }
    if source.spans.len() > 1 {
        notes.push(Note::new(
            slot,
            format!(
                "{} spans were joined into one line of text: a text source has one style now, so \
                 their separate sizes and weights are gone",
                source.spans.len()
            ),
        ));
    }
    text
}

fn style_of(source: &LegacySource) -> TextStyle {
    // A rich-text source's weight and slant come from its first span, which is what its heading
    // was; the plain field is what a non-rich source had.
    let first = source.spans.first();
    TextStyle {
        family: if source.font_family.trim().is_empty() {
            default_family()
        } else {
            source.font_family.trim().to_owned()
        },
        size: size_of(source),
        bold: first.is_some_and(|span| span.font_weight == "bold"),
        italic: first.is_some_and(|span| span.font_style == "italic"),
        alignment: match source.alignment.as_str() {
            "left" => Alignment::Left,
            "right" => Alignment::Right,
            _ => Alignment::Center,
        },
        // The legacy renderer drew white text and stored no colour of its own.
        colour: Tint::WHITE,
    }
    .clamped()
}

fn size_of(source: &LegacySource) -> f32 {
    // A rich-text source's first span is its heading, and that is the size an operator remembers
    // it by; a plain source only ever had the one field.
    let pixels = source
        .spans
        .first()
        .filter(|_| source.is_rich_text)
        .and_then(|span| span.font_size)
        .unwrap_or(source.font_size);
    if pixels.is_finite() && pixels > 0.0 {
        pixels / LEGACY_OUTPUT_HEIGHT
    } else {
        default_size() / LEGACY_OUTPUT_HEIGHT
    }
}

/// A name an operator can find the slot by in a list.
fn name_of(slot: u16, entry: &TextEntry) -> String {
    let described = match &entry.kind {
        TextKind::Static { text } => text.lines().next().unwrap_or("").trim().to_owned(),
        TextKind::Clock => "Clock".to_owned(),
        TextKind::CountdownFromDuration { .. } | TextKind::CountdownToTarget { .. } => {
            "Countdown".to_owned()
        }
    };
    if described.is_empty() {
        // A slot with nothing in it still needs to be findable, and its old number is what the
        // operator knows it as.
        return format!("Slot {slot}");
    }
    described
}

/// The first file in the bank nothing answers at. File zero is never offered.
fn first_free_file(catalog: &TextCatalog) -> Option<u8> {
    (1..=254).find(|file| {
        catalog
            .resolve(MediaAddress::new(DEFAULT_BANK, *file))
            .is_none()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real installation's document, reduced to what each case needs.
    fn document(body: &str) -> Migrated {
        migrate(body, 1_767_225_600_000).expect("a legacy document")
    }

    #[test]
    fn a_slot_keeps_the_address_a_desk_already_uses() {
        let migrated = document(
            r#"{"203":{"type":"static","text":"[countdown]","countdownDuration":60,
                "countdownMode":"on_visible","enabled":true,"fontFamily":"sans-serif",
                "fontSize":72,"alignment":"center","isRichText":false,"spans":[]}}"#,
        );

        let slot = migrated
            .catalog
            .resolve(MediaAddress::new(200, 3))
            .expect("slot 203 is 200/003");
        assert_eq!(
            slot.entry.kind,
            TextKind::CountdownFromDuration {
                duration: Duration::from_secs(60)
            },
            "a source whose whole text was the token is that kind exactly"
        );
        assert!(
            migrated.notes.is_empty(),
            "nothing was lost: {:?}",
            migrated.notes
        );
    }

    #[test]
    fn the_slot_that_cannot_keep_its_address_is_moved_and_says_so() {
        let migrated = document(
            r#"{"200":{"type":"static","text":"Hello","enabled":true,"isRichText":false,
                "spans":[],"fontFamily":"Arial","fontSize":72,"alignment":"center"},
               "201":{"type":"static","text":"Second","enabled":true,"isRichText":false,
                "spans":[],"fontFamily":"Arial","fontSize":72,"alignment":"center"}}"#,
        );

        assert_eq!(
            migrated
                .catalog
                .resolve(MediaAddress::new(200, 1))
                .map(|slot| slot.name.as_str()),
            Some("Second"),
            "the slot with a real address keeps it"
        );
        let moved = migrated
            .catalog
            .slots
            .iter()
            .find(|slot| slot.name == "Hello")
            .expect("kept, somewhere");
        assert_ne!(moved.address.file, 0, "file zero is blank in every bank");
        assert_ne!(moved.address.file, 1, "and it did not displace slot 201");

        let note = migrated
            .notes
            .iter()
            .find(|note| note.slot == 200)
            .expect("the move is reported");
        assert!(note.detail.contains("repatched"), "{}", note.detail);
    }

    #[test]
    fn rich_text_spans_are_joined_and_the_loss_is_reported() {
        let migrated = document(
            r#"{"201":{"type":"static","text":"unused","enabled":true,"isRichText":true,
                "fontFamily":"Arial","fontSize":72,"alignment":"center","spans":[
                  {"text":"Hello","fontSize":100,"fontWeight":"bold","fontStyle":"normal","newLine":true},
                  {"text":"World","fontSize":50,"fontWeight":"normal","fontStyle":"normal","newLine":true}]}}"#,
        );

        let slot = migrated
            .catalog
            .resolve(MediaAddress::new(200, 1))
            .expect("migrated");
        assert_eq!(
            slot.entry.kind,
            TextKind::Static {
                text: "Hello\nWorld".to_owned()
            },
            "the line break a span asked for is kept"
        );
        assert!(
            slot.style.bold,
            "the first span's weight becomes the slot's"
        );
        assert!(
            migrated
                .notes
                .iter()
                .any(|note| note.detail.contains("spans were joined")),
            "{:?}",
            migrated.notes
        );
    }

    #[test]
    fn a_token_wrapped_in_words_becomes_the_kind_and_reports_the_words() {
        let migrated = document(
            r#"{"201":{"type":"static","text":"Count: [countdown]","countdownDuration":60,
                "countdownMode":"on_visible","enabled":true,"isRichText":false,"spans":[],
                "fontFamily":"Arial","fontSize":72,"alignment":"center"}}"#,
        );

        let slot = migrated
            .catalog
            .resolve(MediaAddress::new(200, 1))
            .expect("migrated");
        assert_eq!(
            slot.entry.kind,
            TextKind::CountdownFromDuration {
                duration: Duration::from_secs(60)
            }
        );
        let note = migrated.notes.first().expect("reported");
        assert!(note.detail.contains("Count:"), "{}", note.detail);
    }

    #[test]
    fn a_countdown_to_a_time_of_day_lands_on_the_next_one() {
        // 2026-01-01T00:00:00Z, so 20:30 is later the same day.
        let migrated = document(
            r#"{"201":{"type":"countdown","text":"","countdownDuration":300,
                "countdownMode":"target_datetime","targetDatetime":"20:30:00","enabled":true,
                "isRichText":false,"spans":[],"fontFamily":"Arial","fontSize":72,
                "alignment":"center"}}"#,
        );

        let slot = migrated
            .catalog
            .resolve(MediaAddress::new(200, 1))
            .expect("migrated");
        assert_eq!(
            slot.entry.kind,
            TextKind::CountdownToTarget {
                target_unix_millis: 1_767_225_600_000 + ((20 * 60 + 30) * 60) * 1000
            }
        );
    }

    #[test]
    fn a_time_of_day_already_past_counts_to_tomorrow() {
        // 1767225600000 is midnight; 2026-01-01T02:00 is in the past by 03:00.
        let three_am = 1_767_225_600_000 + 3 * 60 * 60 * 1000;
        let migrated = migrate(
            r#"{"201":{"type":"countdown","countdownMode":"target_datetime",
                "targetDatetime":"02:00:00","countdownDuration":300,"enabled":true,
                "isRichText":false,"spans":[],"fontFamily":"Arial","fontSize":72,
                "alignment":"center","text":""}}"#,
            three_am,
        )
        .expect("migrated");

        let TextKind::CountdownToTarget { target_unix_millis } =
            migrated.catalog.slots[0].entry.kind
        else {
            panic!("a target countdown");
        };
        assert!(
            target_unix_millis > three_am,
            "a countdown to a moment that has passed counts to the next one"
        );
    }

    #[test]
    fn a_size_in_pixels_becomes_a_fraction_of_the_output() {
        let migrated = document(
            r#"{"201":{"type":"static","text":"Words","enabled":true,"isRichText":false,
                "spans":[],"fontFamily":"Arial","fontSize":108,"alignment":"right"}}"#,
        );

        let slot = &migrated.catalog.slots[0];
        assert!(
            (slot.style.size - 0.1).abs() < 1e-6,
            "108 of 1080 lines is a tenth of the height: {}",
            slot.style.size
        );
        assert_eq!(slot.style.alignment, Alignment::Right);
        assert_eq!(slot.style.family, "Arial");
    }

    #[test]
    fn a_parked_source_stays_parked() {
        let migrated = document(
            r#"{"201":{"type":"static","text":"Interval","enabled":false,"isRichText":false,
                "spans":[],"fontFamily":"Arial","fontSize":72,"alignment":"center"}}"#,
        );
        assert!(!migrated.catalog.slots[0].entry.enabled);
    }

    #[test]
    fn a_slot_with_nothing_in_it_is_still_findable_by_its_old_number() {
        let migrated = document(
            r#"{"204":{"type":"static","text":"","enabled":true,"isRichText":true,
                "spans":[{"text":"","fontSize":72,"fontWeight":"normal","fontStyle":"normal",
                "newLine":false}],"fontFamily":"sans-serif","fontSize":72,"alignment":"center"}}"#,
        );
        assert_eq!(migrated.catalog.slots[0].name, "Slot 204");
    }

    #[test]
    fn a_slot_outside_the_bank_is_left_out_and_reported() {
        let migrated = document(
            r#"{"999":{"type":"clock","text":"","enabled":true,"isRichText":false,"spans":[],
                "fontFamily":"Arial","fontSize":72,"alignment":"center"}}"#,
        );
        assert!(migrated.catalog.slots.is_empty());
        assert_eq!(migrated.notes.len(), 1);
        assert!(migrated.notes[0].detail.contains("outside the range"));
    }

    #[test]
    fn a_document_that_is_not_json_is_refused_rather_than_ignored() {
        assert!(matches!(
            migrate("{ not json", 0),
            Err(LegacyTextError::Malformed { .. })
        ));
    }

    #[test]
    fn everything_migrated_is_addressable_and_none_of_it_is_blank() {
        let migrated = document(include_str!("legacy_text_fixture.json"));

        assert_eq!(migrated.catalog.slots.len(), 6, "all six sources survived");
        for slot in &migrated.catalog.slots {
            assert!(!slot.address.is_blank(), "{} is unreachable", slot.name);
            assert!(!slot.name.trim().is_empty());
        }
        assert!(
            migrated.notes.iter().any(|note| note.slot == 200),
            "the operator is told slot 200 moved"
        );
    }
}
