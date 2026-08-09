//! Text sources.
//!
//! A text slot is stored configuration addressed exactly like a clip: a desk selects `(folder,
//! file)` in the text range and the catalog resolves it. So the contract here is a slot's content,
//! its appearance, and the three edits an operator makes — put one at an address, change one,
//! take one away.
//!
//! The kind is a string on the wire and an enumeration in the domain. Translating between them is
//! this adapter's job, and an unrecognized kind is refused by name rather than silently becoming
//! static text.

use std::time::Duration;

use media_domain::MediaAddress;
use media_domain::color::Tint;
use media_domain::text::{TextEntry, TextKind};
use media_domain::text_catalog::{Alignment, TextSlot, TextStyle};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::AddressView;

/// How a text entry is drawn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct TextStyleView {
    /// A family name this machine is asked for. An absent family falls back rather than failing.
    pub family: String,
    /// Height as a fraction of the output's height, so a look survives a change of resolution.
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    /// `left`, `center`, or `right`.
    pub alignment: String,
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

impl TextStyleView {
    pub fn of(style: &TextStyle) -> Self {
        Self {
            family: style.family.clone(),
            size: style.size,
            bold: style.bold,
            italic: style.italic,
            alignment: match style.alignment {
                Alignment::Left => "left",
                Alignment::Center => "center",
                Alignment::Right => "right",
            }
            .to_owned(),
            red: style.colour.red,
            green: style.colour.green,
            blue: style.colour.blue,
        }
    }

    /// The domain style this view describes, clamped to something that can be drawn.
    pub fn into_style(self) -> Result<TextStyle, TextEditError> {
        let alignment = match self.alignment.as_str() {
            "left" => Alignment::Left,
            "center" => Alignment::Center,
            "right" => Alignment::Right,
            other => {
                return Err(TextEditError::UnknownAlignment {
                    alignment: other.to_owned(),
                });
            }
        };
        let family = self.family.trim();
        Ok(TextStyle {
            // An empty family is not a font this machine can be asked for; the shipped default is.
            family: if family.is_empty() {
                TextStyle::default().family
            } else {
                family.to_owned()
            },
            size: self.size,
            bold: self.bold,
            italic: self.italic,
            alignment,
            colour: Tint::new(self.red, self.green, self.blue),
        }
        .clamped())
    }
}

/// One text slot, as the API reports it.
///
/// Every kind's payload is reported in its own field and left absent for the kinds that do not
/// have one, so an editor never has to guess what a shared field means for the kind on screen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct TextSlotView {
    pub address: AddressView,
    pub name: String,
    /// A disabled slot produces nothing, which is how an operator parks one without deleting it.
    pub enabled: bool,
    /// `static`, `clock`, `countdown-duration`, or `countdown-target`.
    pub kind: String,
    pub text: Option<String>,
    pub duration_seconds: Option<f64>,
    /// Rendered as a number rather than a `bigint`: a Unix millisecond stamp is well inside what a
    /// browser holds exactly, and a client should not need big-integer arithmetic to set a deadline.
    #[ts(type = "number | null")]
    pub target_unix_millis: Option<i64>,
    pub style: TextStyleView,
}

impl TextSlotView {
    pub fn of(slot: &TextSlot) -> Self {
        let (kind, text, duration_seconds, target_unix_millis) = match &slot.entry.kind {
            TextKind::Static { text } => ("static", Some(text.clone()), None, None),
            TextKind::Clock => ("clock", None, None, None),
            TextKind::CountdownFromDuration { duration } => (
                "countdown-duration",
                None,
                Some(duration.as_secs_f64()),
                None,
            ),
            TextKind::CountdownToTarget { target_unix_millis } => {
                ("countdown-target", None, None, Some(*target_unix_millis))
            }
        };
        Self {
            address: AddressView::of(slot.address),
            name: slot.name.clone(),
            enabled: slot.entry.enabled,
            kind: kind.to_owned(),
            text,
            duration_seconds,
            target_unix_millis,
            style: TextStyleView::of(&slot.style),
        }
    }
}

/// Why a text edit was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextEditError {
    #[error("no text kind is called {kind}")]
    UnknownKind { kind: String },
    #[error("no alignment is called {alignment}")]
    UnknownAlignment { alignment: String },
    #[error("a {kind} needs {field}")]
    MissingPayload {
        kind: &'static str,
        field: &'static str,
    },
    #[error("a countdown cannot run for a negative length of time")]
    NegativeDuration,
    #[error("a text source needs a name an operator can find it by")]
    EmptyName,
}

/// The content half of a slot, shared by the create and update bodies.
///
/// Kept as one function rather than two so a slot created with a kind and a slot changed to that
/// kind can never disagree about what its payload has to contain.
fn kind_of(
    kind: &str,
    text: Option<&String>,
    duration_seconds: Option<f64>,
    target_unix_millis: Option<i64>,
) -> Result<TextKind, TextEditError> {
    match kind {
        "static" => Ok(TextKind::Static {
            text: text.cloned().unwrap_or_default(),
        }),
        "clock" => Ok(TextKind::Clock),
        "countdown-duration" => {
            let seconds = duration_seconds.ok_or(TextEditError::MissingPayload {
                kind: "countdown",
                field: "durationSeconds",
            })?;
            if !seconds.is_finite() || seconds < 0.0 {
                return Err(TextEditError::NegativeDuration);
            }
            Ok(TextKind::CountdownFromDuration {
                duration: Duration::from_secs_f64(seconds),
            })
        }
        "countdown-target" => Ok(TextKind::CountdownToTarget {
            target_unix_millis: target_unix_millis.ok_or(TextEditError::MissingPayload {
                kind: "countdown",
                field: "targetUnixMillis",
            })?,
        }),
        other => Err(TextEditError::UnknownKind {
            kind: other.to_owned(),
        }),
    }
}

fn named(name: &str) -> Result<String, TextEditError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(TextEditError::EmptyName);
    }
    Ok(trimmed.to_owned())
}

/// Putting a new text source at an address.
///
/// The address is in the body rather than the path because it is what the request *chooses*: the
/// operand is the text catalog, and an address already in use is refused by the catalog itself.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateText {
    pub request_id: String,
    pub folder: u8,
    pub file: u8,
    pub name: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub target_unix_millis: Option<i64>,
    /// Absent means the shipped default appearance, which is what a new slot should look like.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<TextStyleView>,
}

impl CreateText {
    /// The slot this request describes. Address validity is the catalog's rule, not this one's.
    pub fn slot(&self) -> Result<TextSlot, TextEditError> {
        let kind = kind_of(
            &self.kind,
            self.text.as_ref(),
            self.duration_seconds,
            self.target_unix_millis,
        )?;
        let style = match self.style.clone() {
            Some(style) => style.into_style()?,
            None => TextStyle::default(),
        };
        Ok(TextSlot {
            address: MediaAddress::new(self.folder, self.file),
            name: named(&self.name)?,
            entry: TextEntry::new(kind),
            style,
        })
    }
}

/// An intent-shaped text edit: only the fields being changed.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateText {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Changing the kind carries that kind's payload with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub target_unix_millis: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<TextStyleView>,
}

impl UpdateText {
    /// Applies this edit to a stored slot, or says why it cannot be.
    pub fn apply(&self, slot: &mut TextSlot) -> Result<(), TextEditError> {
        // The kind is resolved before anything is written, so a refused payload leaves the stored
        // slot untouched rather than half changed.
        let kind = match &self.kind {
            Some(kind) => Some(kind_of(
                kind,
                self.text.as_ref(),
                self.duration_seconds,
                self.target_unix_millis,
            )?),
            // Editing a payload without naming a kind edits the kind that is already there.
            None => self.edited_payload(&slot.entry.kind)?,
        };
        let name = match &self.name {
            Some(name) => Some(named(name)?),
            None => None,
        };
        let style = match self.style.clone() {
            Some(style) => Some(style.into_style()?),
            None => None,
        };

        if let Some(kind) = kind {
            slot.entry.kind = kind;
        }
        if let Some(name) = name {
            slot.name = name;
        }
        if let Some(enabled) = self.enabled {
            slot.entry.enabled = enabled;
        }
        if let Some(style) = style {
            slot.style = style;
        }
        Ok(())
    }

    /// The stored kind with an edited payload, when the edit changed a payload and not the kind.
    fn edited_payload(&self, current: &TextKind) -> Result<Option<TextKind>, TextEditError> {
        let touched = self.text.is_some()
            || self.duration_seconds.is_some()
            || self.target_unix_millis.is_some();
        if !touched {
            return Ok(None);
        }
        let label = match current {
            TextKind::Static { .. } => "static",
            TextKind::Clock => "clock",
            TextKind::CountdownFromDuration { .. } => "countdown-duration",
            TextKind::CountdownToTarget { .. } => "countdown-target",
        };
        // Existing payloads fill in whatever the edit did not carry, so changing a countdown's
        // length does not require resending the words of a kind it is not.
        let text = match (self.text.as_ref(), current) {
            (Some(text), _) => Some(text.clone()),
            (None, TextKind::Static { text }) => Some(text.clone()),
            (None, _) => None,
        };
        let duration = match (self.duration_seconds, current) {
            (Some(seconds), _) => Some(seconds),
            (None, TextKind::CountdownFromDuration { duration }) => Some(duration.as_secs_f64()),
            (None, _) => None,
        };
        let target = match (self.target_unix_millis, current) {
            (Some(target), _) => Some(target),
            (None, TextKind::CountdownToTarget { target_unix_millis }) => Some(*target_unix_millis),
            (None, _) => None,
        };
        kind_of(label, text.as_ref(), duration, target).map(Some)
    }
}

/// Taking a text source away. It carries a request id like every other edit.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeleteText {
    pub request_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(kind: TextKind) -> TextSlot {
        TextSlot {
            address: MediaAddress::new(200, 3),
            name: "Stand by".to_owned(),
            entry: TextEntry::new(kind),
            style: TextStyle::default(),
        }
    }

    fn update(body: &str) -> UpdateText {
        serde_json::from_str(body).expect("a text edit")
    }

    #[test]
    fn every_kind_reports_its_own_payload_and_nothing_else() {
        let statically = TextSlotView::of(&slot(TextKind::Static {
            text: "Doors in five".to_owned(),
        }));
        assert_eq!(statically.kind, "static");
        assert_eq!(statically.text.as_deref(), Some("Doors in five"));
        assert_eq!(statically.duration_seconds, None);

        let countdown = TextSlotView::of(&slot(TextKind::CountdownFromDuration {
            duration: Duration::from_secs(600),
        }));
        assert_eq!(countdown.kind, "countdown-duration");
        assert_eq!(countdown.duration_seconds, Some(600.0));
        assert_eq!(countdown.text, None);

        let clock = TextSlotView::of(&slot(TextKind::Clock));
        assert_eq!(clock.kind, "clock");
        assert_eq!(clock.text, None);
        assert_eq!(clock.target_unix_millis, None);
    }

    #[test]
    fn a_new_slot_gets_the_shipped_appearance_when_none_was_chosen() {
        let create: CreateText = serde_json::from_str(
            r#"{"requestId":"a","folder":201,"file":4,"name":"Cue","kind":"static","text":"Go"}"#,
        )
        .unwrap();
        let slot = create.slot().expect("accepted");

        assert_eq!(slot.address, MediaAddress::new(201, 4));
        assert_eq!(slot.style, TextStyle::default());
        assert!(slot.entry.enabled, "a new slot is not parked");
    }

    #[test]
    fn changing_a_countdown_length_does_not_require_resending_its_kind() {
        let mut stored = slot(TextKind::CountdownFromDuration {
            duration: Duration::from_secs(600),
        });
        update(r#"{"requestId":"a","durationSeconds":90}"#)
            .apply(&mut stored)
            .expect("accepted");

        assert_eq!(
            stored.entry.kind,
            TextKind::CountdownFromDuration {
                duration: Duration::from_secs(90)
            }
        );
        assert_eq!(stored.name, "Stand by", "nothing else moved");
    }

    #[test]
    fn changing_the_kind_carries_that_kinds_payload() {
        let mut stored = slot(TextKind::Clock);
        update(r#"{"requestId":"a","kind":"static","text":"House open"}"#)
            .apply(&mut stored)
            .expect("accepted");
        assert_eq!(
            stored.entry.kind,
            TextKind::Static {
                text: "House open".to_owned()
            }
        );

        let mut missing = slot(TextKind::Clock);
        let error = update(r#"{"requestId":"b","kind":"countdown-duration"}"#)
            .apply(&mut missing)
            .expect_err("refused");
        assert_eq!(
            error,
            TextEditError::MissingPayload {
                kind: "countdown",
                field: "durationSeconds"
            }
        );
        assert_eq!(
            missing.entry.kind,
            TextKind::Clock,
            "a refused edit changed nothing"
        );
    }

    #[test]
    fn a_slot_can_be_parked_without_losing_what_it_says() {
        let mut stored = slot(TextKind::Static {
            text: "Interval".to_owned(),
        });
        update(r#"{"requestId":"a","enabled":false}"#)
            .apply(&mut stored)
            .expect("accepted");

        assert!(!stored.entry.enabled);
        assert_eq!(
            stored.entry.kind,
            TextKind::Static {
                text: "Interval".to_owned()
            }
        );
    }

    #[test]
    fn an_unknown_kind_or_alignment_is_refused_by_name() {
        let mut stored = slot(TextKind::Clock);
        assert_eq!(
            update(r#"{"requestId":"a","kind":"scrolling"}"#)
                .apply(&mut stored)
                .expect_err("refused"),
            TextEditError::UnknownKind {
                kind: "scrolling".to_owned()
            }
        );

        let error = update(
            r#"{"requestId":"b","style":{"family":"Inter","size":0.2,"bold":false,"italic":false,"alignment":"justified","red":1,"green":1,"blue":1}}"#,
        )
        .apply(&mut stored)
        .expect_err("refused");
        assert_eq!(
            error,
            TextEditError::UnknownAlignment {
                alignment: "justified".to_owned()
            }
        );
    }

    #[test]
    fn a_text_source_cannot_be_left_nameless() {
        let mut stored = slot(TextKind::Clock);
        assert_eq!(
            update(r#"{"requestId":"a","name":"   "}"#)
                .apply(&mut stored)
                .expect_err("refused"),
            TextEditError::EmptyName
        );
        assert_eq!(stored.name, "Stand by");
    }

    #[test]
    fn an_unusable_size_is_brought_back_into_range_rather_than_refused() {
        let mut stored = slot(TextKind::Clock);
        update(
            r#"{"requestId":"a","style":{"family":"  ","size":900,"bold":true,"italic":false,"alignment":"right","red":1,"green":0.5,"blue":0}}"#,
        )
        .apply(&mut stored)
        .expect("accepted");

        assert!(stored.style.size <= 2.0, "{}", stored.style.size);
        assert_eq!(stored.style.family, TextStyle::default().family);
        assert_eq!(stored.style.alignment, Alignment::Right);
        assert!(stored.style.bold);
    }
}
