//! The virtual dimmer of a light-emitting head that has no Intensity channel.
//!
//! Such a head still has an Intensity: a virtual one the desk programs like any other, which the
//! masters scale before DMX and which reaches the light through the channels following it.

use super::{FixtureChannel, FixtureMode};
use std::collections::HashSet;
use uuid::Uuid;

impl FixtureChannel {
    fn carries_intensity(&self) -> bool {
        self.attribute.is_intensity() || self.fixture_attribute.is_intensity()
    }
}

impl FixtureMode {
    /// Whether this head has a virtual dimmer: it emits light, or has a channel following the
    /// virtual intensity, and no Intensity channel of its own.
    pub fn head_has_virtual_dimmer(&self, head_id: Uuid) -> bool {
        let mut channels = self
            .channels
            .iter()
            .filter(|channel| channel.head_id == head_id);
        let mut emits = false;
        for channel in channels.by_ref() {
            if channel.carries_intensity() {
                return false;
            }
            emits |=
                channel.reacts_to_virtual_intensity || channel.fixture_attribute.is_color_emitter();
        }
        emits
    }

    /// Makes the colour emitters of every head without an Intensity channel follow its virtual
    /// dimmer. This is the default a new mode starts from; an operator may later set a channel to
    /// Ignore or Inverse, so it is applied where a mode is created, never on load.
    pub fn default_virtual_dimmer_reactions(&mut self) {
        let dimmed: HashSet<Uuid> = self
            .channels
            .iter()
            .filter(|channel| channel.carries_intensity())
            .map(|channel| channel.head_id)
            .collect();
        for channel in &mut self.channels {
            if !dimmed.contains(&channel.head_id) && channel.fixture_attribute.is_color_emitter() {
                channel.reacts_to_virtual_intensity = true;
                channel.virtual_intensity_inverted = false;
            }
        }
    }
}
