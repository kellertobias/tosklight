/// The runtime consequence of an accepted playback control operation.
///
/// This domain type deliberately does not carry transport or persistence details. Callers can
/// retain durable runtime changes, publish transient control feedback, and skip exact no-ops.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlaybackRuntimeEffect {
    #[default]
    None,
    Transient,
    Durable,
}

impl PlaybackRuntimeEffect {
    pub const fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Durable, _) | (_, Self::Durable) => Self::Durable,
            (Self::Transient, _) | (_, Self::Transient) => Self::Transient,
            (Self::None, Self::None) => Self::None,
        }
    }

    pub const fn changed(self) -> bool {
        !matches!(self, Self::None)
    }

    pub const fn durable(self) -> bool {
        matches!(self, Self::Durable)
    }
}

/// A control result paired with its exact runtime consequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackMutation<T> {
    pub value: T,
    /// The consequence owned by the explicitly addressed Playback.
    pub addressed_effect: PlaybackRuntimeEffect,
    /// The aggregate consequence, including automatic changes to related Playbacks.
    pub effect: PlaybackRuntimeEffect,
}

impl<T> PlaybackMutation<T> {
    pub const fn new(value: T, effect: PlaybackRuntimeEffect) -> Self {
        Self {
            value,
            addressed_effect: effect,
            effect,
        }
    }

    pub const fn with_related_effect(
        value: T,
        addressed_effect: PlaybackRuntimeEffect,
        related_effect: PlaybackRuntimeEffect,
    ) -> Self {
        Self {
            value,
            addressed_effect,
            effect: addressed_effect.combine(related_effect),
        }
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> PlaybackMutation<U> {
        PlaybackMutation {
            value: map(self.value),
            addressed_effect: self.addressed_effect,
            effect: self.effect,
        }
    }
}
