use crate::*;

impl PlaybackEngine {
    pub(crate) fn take_transition_ordinal(&mut self) -> u64 {
        let ordinal = self.next_transition_ordinal;
        self.next_transition_ordinal = ordinal.saturating_add(1);
        ordinal
    }

    pub(crate) fn observe_restored_transition_ordinal(&mut self, ordinal: u64) {
        if let Some(next) = ordinal.checked_add(1) {
            self.next_transition_ordinal = self.next_transition_ordinal.max(next);
        }
    }

    pub fn record_activation(&mut self, number: u16, origin: PlaybackActivationOrigin) {
        let Ok(identity) = PlaybackIdentity::physical(number) else {
            return;
        };
        self.record_activation_at(identity, origin);
    }

    pub fn record_activation_at(
        &mut self,
        identity: PlaybackIdentity,
        origin: PlaybackActivationOrigin,
    ) {
        let ordinal = self.next_activation_ordinal;
        self.next_activation_ordinal = ordinal.saturating_add(1);
        let Ok(key) = self.runtime_key_at(identity) else {
            return;
        };
        let Some(playback) = self
            .active
            .get_mut(&key)
            .filter(|playback| playback.enabled)
        else {
            return;
        };
        playback.activation = Some(PlaybackActivationProvenance {
            ordinal,
            at: origin.at,
            desk_id: origin.desk_id,
            surface: origin.surface,
            exclusion_scope: origin.exclusion_scope,
        });
    }

    pub(crate) fn observe_restored_activation(
        &mut self,
        activation: Option<&PlaybackActivationProvenance>,
    ) {
        let Some(next) = activation.and_then(|activation| activation.ordinal.checked_add(1)) else {
            return;
        };
        self.next_activation_ordinal = self.next_activation_ordinal.max(next);
    }
}
