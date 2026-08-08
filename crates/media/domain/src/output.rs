//! Output identity and presentation.
//!
//! One Media Server process hosts one or more logical outputs. The first release ships a single
//! output, but every surface — the API, the React UI, the persistence schema, CITP announcements,
//! logs, health state, and GDTF exports — addresses outputs by this stable identifier from the
//! start, so adding output two never means replacing singleton state.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable identity of one logical output. It survives renames, monitor changes, and
/// configuration migrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputId(Uuid);

impl OutputId {
    /// Mints a new identity for an output the operator has just created.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Rebuilds an identity read back from persisted configuration.
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// The underlying value, for persistence and wire encoding only.
    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for OutputId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for OutputId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// The operator-visible name of an output. Never an identity: two outputs may share a name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputName(String);

impl OutputName {
    /// Trims surrounding whitespace and falls back to a stable placeholder when nothing is left,
    /// so an output is never nameless in the UI, in logs, or in CITP announcements.
    pub fn new(value: impl Into<String>) -> Self {
        let trimmed = value.into().trim().to_owned();
        if trimmed.is_empty() {
            Self("Output".to_owned())
        } else {
            Self(trimmed)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for OutputName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// How an output paces its render clock.
///
/// The legacy application requested a fixed 60 fps. That is legacy behavior, not the timing
/// contract: each output owns its clock so two outputs on displays with different refresh rates
/// never share a global frame counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PresentationMode {
    /// Follow the real refresh timing of the owning monitor. The renderer picks a supported
    /// vsynchronized presentation mode from the surface's own capabilities rather than assuming
    /// any particular rate.
    #[default]
    DisplaySynchronized,
    /// Schedule against monotonic deadlines at a fixed rate. Serves off-screen, streaming, and
    /// test use.
    FixedFps { frames_per_second: u16 },
    /// Present as fast as the surface allows. Diagnostic only.
    Unlocked,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_names_are_trimmed_and_never_empty() {
        assert_eq!(OutputName::new("  Main  ").as_str(), "Main");
        assert_eq!(OutputName::new("   ").as_str(), "Output");
        assert_eq!(OutputName::new("").as_str(), "Output");
    }

    #[test]
    fn output_identities_are_distinct_and_round_trip() {
        let first = OutputId::new();
        let second = OutputId::new();
        assert_ne!(first, second);
        assert_eq!(OutputId::from_uuid(first.as_uuid()), first);
    }

    #[test]
    fn display_synchronized_is_the_default_presentation_mode() {
        assert_eq!(
            PresentationMode::default(),
            PresentationMode::DisplaySynchronized
        );
    }
}
