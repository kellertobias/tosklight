use crate::*;

impl PlaybackEngine {
    /// Orders a Cue transition starting at `at`, which the next output frame has to carry.
    pub(crate) fn take_transition_ordinal(&mut self, at: DateTime<Utc>) -> u64 {
        self.change_lead.mark(at.timestamp_micros());
        let ordinal = self.next_transition_ordinal;
        self.next_transition_ordinal = ordinal.saturating_add(1);
        ordinal
    }

    /// TL-659: the earliest Cue transition due by `sampled_at`, taken by the output frame
    /// sampled then. Call only from the Live output lane, under the Playback lock.
    pub fn claim_change_lead_start(&mut self, sampled_at: DateTime<Utc>) -> Option<i64> {
        self.change_lead.claim(sampled_at.timestamp_micros())
    }

    pub(crate) fn observe_restored_transition_ordinal(&mut self, ordinal: u64) {
        if let Some(next) = ordinal.checked_add(1) {
            self.next_transition_ordinal = self.next_transition_ordinal.max(next);
        }
    }

    pub(crate) fn take_source_occurrence_ordinal(&mut self) -> Option<u64> {
        crate::source_evidence::take_occurrence_ordinal(&mut self.next_source_occurrence_ordinal)
    }

    /// Greatest allocated or reserved provenance ordinal, independent of LTP arbitration.
    pub fn source_occurrence_watermark(&self) -> u64 {
        self.next_source_occurrence_ordinal
            .checked_sub(1)
            .unwrap_or(u64::MAX)
    }

    /// Reserve historical IDs before restoring a source catalogue. MAX exhausts future exact
    /// occurrences; values keep running with unknown evidence rather than reusing an identity.
    pub fn reserve_source_occurrence_watermark(&mut self, watermark: u64) {
        if self.next_source_occurrence_ordinal == 0 {
            return;
        }
        self.next_source_occurrence_ordinal = watermark
            .checked_add(1)
            .map(|next| next.max(self.next_source_occurrence_ordinal))
            .unwrap_or(0);
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
