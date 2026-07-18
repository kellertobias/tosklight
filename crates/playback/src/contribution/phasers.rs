use super::attributes::timed_value;
use super::{ContributionContext, PlaybackFrame};
use crate::*;

impl ContributionContext<'_> {
    pub(super) fn extend_phasers(
        &self,
        values: &mut Vec<PlaybackContribution>,
        frame: &PlaybackFrame<'_>,
    ) {
        let elapsed = phaser_elapsed(frame);
        for phaser in &frame.cue.phasers {
            for (index, fixture_id) in phaser.fixture_ids.iter().enumerate() {
                values.push(self.phaser_contribution(frame, phaser, index, *fixture_id, elapsed));
            }
        }
    }

    fn phaser_contribution(
        &self,
        frame: &PlaybackFrame<'_>,
        attribute_phaser: &AttributePhaser,
        fixture_index: usize,
        fixture_id: FixtureId,
        elapsed: f64,
    ) -> PlaybackContribution {
        let snap = (self.is_snap)(fixture_id, &attribute_phaser.attribute);
        let sequence_master = frame.master_for(snap);
        let sampled = attribute_phaser.phaser.sample(
            elapsed,
            fixture_index,
            attribute_phaser.fixture_ids.len(),
        );
        let base = frame
            .target
            .get(&(fixture_id, attribute_phaser.attribute.clone()))
            .and_then(AttributeValue::normalized)
            .unwrap_or(0.0);
        let mut level = match attribute_phaser.phaser.mode {
            PhaserMode::Absolute => sampled,
            PhaserMode::Relative => base + sampled,
        }
        .clamp(0.0, 1.0);
        if attribute_phaser.attribute.is_intensity() {
            level *= sequence_master;
        }
        PlaybackContribution {
            value: timed_value(
                frame,
                fixture_id,
                attribute_phaser.attribute.clone(),
                AttributeValue::Normalized(level),
            ),
            sequence_master,
            source: frame.source,
        }
    }
}

fn phaser_elapsed(frame: &PlaybackFrame<'_>) -> f64 {
    (frame.effective_now
        - frame.playback.activated_at
        - ChronoDuration::milliseconds(frame.cue.delay_millis as i64))
    .num_milliseconds()
    .max(0) as f64
        / 1000.0
}
