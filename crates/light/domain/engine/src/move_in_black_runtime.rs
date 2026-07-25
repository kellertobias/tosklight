use crate::{MoveInBlackDiagnostic, MoveInBlackPosition, MoveInBlackState, PreparedCandidate};
use chrono::{DateTime, Duration, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue};
use light_playback::{ActivePlayback, MoveInBlackCandidate};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct MoveInBlackKey {
    pub(crate) playback_number: Option<u16>,
    pub(crate) cue_list_id: light_core::CueListId,
    pub(crate) fixture_id: FixtureId,
}

#[derive(Clone)]
pub(crate) struct MoveInBlackRuntime {
    candidate: MoveInBlackCandidate,
    enabled: bool,
    delay_millis: u64,
    state: MoveInBlackState,
    dark_since: Option<DateTime<Utc>>,
    delay_deadline: Option<DateTime<Utc>>,
    movement_started_at: Option<DateTime<Utc>>,
    movement_ends_at: Option<DateTime<Utc>>,
    from: HashMap<AttributeKey, AttributeValue>,
    current: HashMap<AttributeKey, AttributeValue>,
    changed_at: DateTime<Utc>,
    handoff_until: Option<DateTime<Utc>>,
    cancellation_reason: Option<String>,
}

impl MoveInBlackRuntime {
    pub(crate) fn new(candidate: &PreparedCandidate, now: DateTime<Utc>) -> Self {
        Self {
            candidate: candidate.candidate.clone(),
            enabled: candidate.enabled,
            delay_millis: candidate.delay_millis,
            state: if candidate.enabled {
                MoveInBlackState::Blocked
            } else {
                MoveInBlackState::Disabled
            },
            dark_since: None,
            delay_deadline: None,
            movement_started_at: None,
            movement_ends_at: None,
            from: candidate.base_position.clone(),
            current: candidate.base_position.clone(),
            changed_at: now,
            handoff_until: None,
            cancellation_reason: None,
        }
    }

    pub(crate) fn update(&mut self, candidate: PreparedCandidate, now: DateTime<Utc>) {
        let candidate_changed = self.candidate != candidate.candidate;
        let enabled_changed = self.enabled != candidate.enabled;
        let delay_changed = self.delay_millis != candidate.delay_millis;
        if candidate_changed {
            self.replace_candidate(candidate.candidate.clone(), &candidate.base_position, now);
        }
        self.enabled = candidate.enabled;
        self.delay_millis = candidate.delay_millis;
        if !self.enabled {
            self.disable(candidate.base_position);
            return;
        }
        if candidate.resolved_intensity > 0.0 {
            self.block(candidate.base_position);
            return;
        }
        self.update_dark_timing(candidate.base_position, enabled_changed, delay_changed, now);
        let deadline = self.delay_deadline.expect("dark runtime has a deadline");
        if now < deadline {
            self.state = MoveInBlackState::Delaying;
            self.current = self.from.clone();
            return;
        }
        if self.movement_started_at.is_none() {
            let started_at = if candidate_changed { now } else { deadline };
            self.start_movement(started_at);
        }
        self.current = self.values_at(now);
        self.state = if self.movement_ends_at.is_some_and(|ends_at| now >= ends_at) {
            MoveInBlackState::Completed
        } else {
            MoveInBlackState::Moving
        };
    }

    pub(crate) fn update_absent(
        &mut self,
        key: MoveInBlackKey,
        active: &[ActivePlayback],
        now: DateTime<Utc>,
    ) {
        if !self.enabled {
            self.state = MoveInBlackState::Disabled;
            self.cancellation_reason = None;
            return;
        }
        let target_active = active.iter().find(|playback| {
            playback.enabled
                && playback.playback_number == key.playback_number
                && playback.cue_list_id == key.cue_list_id
                && playback.current_cue_id == Some(self.candidate.target_cue_id)
        });
        if let Some(playback) = target_active
            && matches!(
                self.state,
                MoveInBlackState::Moving | MoveInBlackState::Completed
            )
        {
            self.update_handoff(playback, now);
        } else if self.state != MoveInBlackState::Cancelled {
            self.cancel(key, active);
        }
    }

    pub(crate) fn contributes(
        &self,
        key: &MoveInBlackKey,
        present: &HashSet<MoveInBlackKey>,
        now: DateTime<Utc>,
    ) -> bool {
        self.enabled
            && (present.contains(key) || self.handoff_until.is_some_and(|until| now < until))
            && matches!(
                self.state,
                MoveInBlackState::Moving | MoveInBlackState::Completed
            )
    }

    pub(crate) fn timed_values(&self) -> Vec<TimedValue> {
        self.current
            .iter()
            .map(|(attribute, value)| TimedValue {
                fixture_id: self.candidate.fixture_id,
                attribute: attribute.clone(),
                value: value.clone(),
                priority: self.candidate.priority,
                changed_at: self.changed_at,
                programmer_order: 0,
                merge_mode: MergeMode::Ltp,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            })
            .collect()
    }

    pub(crate) fn diagnostic(&self) -> MoveInBlackDiagnostic {
        let mut positions = self
            .candidate
            .values
            .iter()
            .map(|value| MoveInBlackPosition {
                attribute: value.attribute.clone(),
                current: self
                    .current
                    .get(&value.attribute)
                    .cloned()
                    .unwrap_or_else(|| value.current.clone()),
                target: value.target.clone(),
            })
            .collect::<Vec<_>>();
        positions.sort_by(|left, right| left.attribute.cmp(&right.attribute));
        MoveInBlackDiagnostic {
            fixture_id: self.candidate.fixture_id,
            playback_number: self.candidate.playback_number,
            cue_list_id: self.candidate.cue_list_id,
            current_cue_id: self.candidate.current_cue_id,
            current_cue_number: self.candidate.current_cue_number,
            target_cue_id: self.candidate.target_cue_id,
            target_cue_number: self.candidate.target_cue_number,
            state: self.state,
            positions,
            dark_since: self.dark_since,
            delay_deadline: self.delay_deadline,
            movement_started_at: self.movement_started_at,
            movement_ends_at: self.movement_ends_at,
            cancellation_reason: self.cancellation_reason.clone(),
        }
    }

    fn replace_candidate(
        &mut self,
        candidate: MoveInBlackCandidate,
        base_position: &HashMap<AttributeKey, AttributeValue>,
        now: DateTime<Utc>,
    ) {
        let previous = self.values_at(now);
        let was_dark = self.dark_since;
        self.candidate = candidate;
        self.from = if was_dark.is_some() {
            previous
        } else {
            base_position.clone()
        };
        self.current = self.from.clone();
        self.movement_started_at = None;
        self.movement_ends_at = None;
        self.handoff_until = None;
        self.changed_at = now;
        self.cancellation_reason = Some("future_target_recalculated".into());
    }

    fn disable(&mut self, base_position: HashMap<AttributeKey, AttributeValue>) {
        self.state = MoveInBlackState::Disabled;
        self.clear_timing();
        self.from = base_position.clone();
        self.current = base_position;
        self.cancellation_reason = None;
    }

    fn block(&mut self, base_position: HashMap<AttributeKey, AttributeValue>) {
        self.cancellation_reason = self
            .dark_since
            .is_some()
            .then(|| "resolved_intensity_above_zero".into());
        self.state = MoveInBlackState::Blocked;
        self.clear_timing();
        self.from = base_position.clone();
        self.current = base_position;
    }

    fn clear_timing(&mut self) {
        self.dark_since = None;
        self.delay_deadline = None;
        self.movement_started_at = None;
        self.movement_ends_at = None;
        self.handoff_until = None;
    }

    fn update_dark_timing(
        &mut self,
        base_position: HashMap<AttributeKey, AttributeValue>,
        enabled_changed: bool,
        delay_changed: bool,
        now: DateTime<Utc>,
    ) {
        if enabled_changed || self.dark_since.is_none() {
            self.dark_since = Some(now);
            self.delay_deadline = Some(now + duration(self.delay_millis));
            self.movement_started_at = None;
            self.movement_ends_at = None;
            self.from = base_position.clone();
            self.current = base_position;
            self.changed_at = now;
            self.cancellation_reason = None;
        } else if delay_changed && let Some(dark_since) = self.dark_since {
            self.delay_deadline = Some(dark_since + duration(self.delay_millis));
            self.movement_started_at = None;
            self.movement_ends_at = None;
            self.from = base_position;
            self.changed_at = now;
        }
    }

    fn start_movement(&mut self, started_at: DateTime<Utc>) {
        self.movement_started_at = Some(started_at);
        self.movement_ends_at = Some(started_at + duration(self.longest_fade()));
        self.changed_at = started_at;
    }

    fn update_handoff(&mut self, playback: &ActivePlayback, now: DateTime<Utc>) {
        if self.handoff_until.is_none() {
            self.from = self.values_at(now);
            self.movement_started_at = Some(playback.activated_at);
            self.movement_ends_at = Some(playback.activated_at + duration(self.longest_fade()));
            self.handoff_until = self.movement_ends_at;
            self.changed_at = playback.activated_at + Duration::microseconds(1);
            self.cancellation_reason = None;
        }
        self.current = self.values_at(now);
        self.state = if self.is_at_target() || self.handoff_until.is_some_and(|until| now >= until)
        {
            MoveInBlackState::Completed
        } else {
            MoveInBlackState::Moving
        };
    }

    fn cancel(&mut self, key: MoveInBlackKey, active: &[ActivePlayback]) {
        self.state = MoveInBlackState::Cancelled;
        self.handoff_until = None;
        self.cancellation_reason = Some(
            if active.iter().any(|playback| {
                playback.playback_number == key.playback_number
                    && playback.cue_list_id == key.cue_list_id
            }) {
                "future_target_invalidated".into()
            } else {
                "cuelist_released".into()
            },
        );
    }

    fn values_at(&self, now: DateTime<Utc>) -> HashMap<AttributeKey, AttributeValue> {
        let Some(started_at) = self.movement_started_at else {
            return self.from.clone();
        };
        self.candidate
            .values
            .iter()
            .map(|target| {
                let from = self.from.get(&target.attribute).unwrap_or(&target.current);
                let elapsed = (now - started_at).num_milliseconds().max(0) as u64;
                let progress = if target.fade_millis == 0 {
                    1.0
                } else {
                    (elapsed as f32 / target.fade_millis as f32).clamp(0.0, 1.0)
                };
                let value = interpolate_value(from, &target.target, progress);
                (target.attribute.clone(), value)
            })
            .collect()
    }

    fn is_at_target(&self) -> bool {
        self.candidate.values.iter().all(|target| {
            self.current
                .get(&target.attribute)
                .is_some_and(|current| values_equal(current, &target.target))
        })
    }

    fn longest_fade(&self) -> u64 {
        self.candidate
            .values
            .iter()
            .map(|value| value.fade_millis)
            .max()
            .unwrap_or(0)
    }
}

fn duration(millis: u64) -> Duration {
    Duration::milliseconds(millis.min(i64::MAX as u64) as i64)
}

fn interpolate_value(
    from: &AttributeValue,
    target: &AttributeValue,
    progress: f32,
) -> AttributeValue {
    match (from.normalized(), target.normalized()) {
        (Some(from), Some(to)) => AttributeValue::Normalized(from + (to - from) * progress),
        _ if progress >= 1.0 => target.clone(),
        _ => from.clone(),
    }
}

fn values_equal(current: &AttributeValue, target: &AttributeValue) -> bool {
    match (current.normalized(), target.normalized()) {
        (Some(current), Some(target)) => (current - target).abs() <= f32::EPSILON * 8.0,
        _ => current == target,
    }
}
