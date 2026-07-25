use crate::*;

impl PlaybackEngine {
    pub fn record_activation(&mut self, number: u16, origin: PlaybackActivationOrigin) {
        let ordinal = self.next_activation_ordinal;
        self.next_activation_ordinal = ordinal.saturating_add(1);
        let Some(playback) = self
            .active
            .get_mut(&PlaybackKey::Number(number))
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
