use crate::*;
use chrono::{DateTime, Utc};

/// One sampled volatile runtime row for a numbered Playback. Static topology (names, slot
/// layout, configuration) stays on the snapshot + revisioned event path; this row carries only
/// the values a running fade or button press changes between revisions.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlaybackTelemetrySample {
    pub playback_number: u16,
    pub enabled: bool,
    pub master: f32,
    pub current_cue_id: Option<Uuid>,
    pub current_cue_number: Option<f64>,
    /// 0..=1 progress into the current Cue transition, or `None` while no Cuelist is active.
    pub fade_progress: Option<f32>,
    pub flash: bool,
    pub temporary_active: bool,
    pub swap_active: bool,
}

impl PlaybackEngine {
    /// Samples the volatile runtime values of every numbered Playback at `at`.
    ///
    /// Derived read-only from already-published runtime state; the render loop is not touched.
    pub fn telemetry_samples_at(&self, at: DateTime<Utc>) -> Vec<PlaybackTelemetrySample> {
        let mut samples: Vec<PlaybackTelemetrySample> = self
            .runtime_status()
            .into_iter()
            .filter_map(|status| self.telemetry_sample_at(status, at))
            .collect();
        samples.sort_by_key(|sample| sample.playback_number);
        samples
    }

    fn telemetry_sample_at(
        &self,
        status: PlaybackRuntimeStatus,
        at: DateTime<Utc>,
    ) -> Option<PlaybackTelemetrySample> {
        let playback = status.playback;
        let number = playback.playback_number?;
        let fade_progress = self
            .cue_lists
            .get(&playback.cue_list_id)
            .map(|cue_list| self.fade_progress_at(&playback, cue_list, at));
        Some(PlaybackTelemetrySample {
            playback_number: number,
            enabled: playback.enabled,
            master: playback.master,
            current_cue_id: playback.current_cue_id,
            current_cue_number: playback.current_cue_number,
            fade_progress,
            flash: playback.flash,
            temporary_active: status.temporary_active,
            swap_active: status.swap_active,
        })
    }

    fn fade_progress_at(
        &self,
        playback: &ActivePlayback,
        cue_list: &CueList,
        at: DateTime<Utc>,
    ) -> f32 {
        let Some(current) = current_telemetry_cue(playback, cue_list) else {
            return 1.0;
        };
        let Some(compiled) = self.compiled_cue_lists.get(&playback.cue_list_id) else {
            return 1.0;
        };
        let cue_fade_millis = effective_cue_fade_millis(
            cue_list,
            current,
            playback,
            self.sequence_master_fade_millis,
            &self.speed_groups_bpm,
        );
        let completion = cue_completion_millis(cue_list, compiled, playback, cue_fade_millis);
        if completion == 0 {
            return 1.0;
        }
        let observed = playback.paused_at.unwrap_or(at).max(playback.activated_at);
        let elapsed = (observed - playback.activated_at).num_milliseconds().max(0) as u64;
        (elapsed as f64 / completion as f64).min(1.0) as f32
    }
}

fn current_telemetry_cue<'a>(playback: &ActivePlayback, cue_list: &'a CueList) -> Option<&'a Cue> {
    playback
        .current_cue_id
        .and_then(|id| cue_list.cues.iter().find(|cue| cue.id == id))
        .or_else(|| {
            playback
                .current_cue_number
                .and_then(|number| cue_list.cues.iter().find(|cue| cue.number == number))
        })
        .or_else(|| cue_list.cues.get(playback.cue_index))
}
