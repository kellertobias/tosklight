use std::{fmt, sync::Arc};

pub const MAX_PLAYBACK_GROUP_ID_BYTES: usize = 256;

/// An opaque, bounded Group identifier safe to retain in idempotency keys and event identities.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct PlaybackGroupId(Arc<str>);

impl PlaybackGroupId {
    pub fn new(value: &str) -> Result<Self, PlaybackGroupIdError> {
        if value.is_empty() {
            return Err(PlaybackGroupIdError::Empty);
        }
        if value.len() > MAX_PLAYBACK_GROUP_ID_BYTES {
            return Err(PlaybackGroupIdError::TooLong);
        }
        if value.chars().any(char::is_control) {
            return Err(PlaybackGroupIdError::ControlCharacter);
        }
        Ok(Self(Arc::from(value)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PlaybackGroupId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PlaybackGroupId")
            .field(&self.as_str())
            .finish()
    }
}

impl fmt::Display for PlaybackGroupId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackGroupIdError {
    Empty,
    TooLong,
    ControlCharacter,
}

impl fmt::Display for PlaybackGroupIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "group_id must not be empty",
            Self::TooLong => "group_id must be at most 256 bytes",
            Self::ControlCharacter => "group_id must not contain control characters",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_opaque_unicode_identity() {
        let id = PlaybackGroupId::new(" Front · 1 ").unwrap();
        assert_eq!(id.as_str(), " Front · 1 ");
    }

    #[test]
    fn rejects_empty_oversized_and_control_identifiers() {
        assert_eq!(PlaybackGroupId::new(""), Err(PlaybackGroupIdError::Empty));
        assert_eq!(
            PlaybackGroupId::new(&"x".repeat(MAX_PLAYBACK_GROUP_ID_BYTES + 1)),
            Err(PlaybackGroupIdError::TooLong)
        );
        assert_eq!(
            PlaybackGroupId::new("front\n"),
            Err(PlaybackGroupIdError::ControlCharacter)
        );
    }
}
