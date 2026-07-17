#![forbid(unsafe_code)]
//! Deterministic bridge from fixture attributes and playbacks to immutable DMX universe frames.

use arc_swap::ArcSwap;
use light_core::{
    AttributeKey, AttributeValue, FixtureId, MergeMode, ProgrammerId, SharedClock, TimedValue,
    Universe,
};
use light_fixture::{
    PatchedFixture, SignalLossPolicy, encode_parameter, mix_color, validate_patch,
};
use light_output::{DmxFrame, OutputRoute};
use light_playback::{
    ActivePlayback, CueList, MoveInBlackCandidate, PlaybackDefinition, PlaybackEngine,
    PlaybackPage, PlaybackTarget, resolve,
};
use light_programmer::ProgrammerRegistry;
use light_programmer::{GroupDefinition, resolve_group};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use thiserror::Error;

fn value_for_ordered_position(
    value: &AttributeValue,
    index: usize,
    count: usize,
) -> AttributeValue {
    let AttributeValue::Spread(points) = value else {
        return value.clone();
    };
    if points.is_empty() {
        return AttributeValue::Normalized(0.0);
    }
    if points.len() == 1 || count <= 1 {
        return AttributeValue::Normalized(points[0]);
    }
    let position = index as f32 * (points.len() - 1) as f32 / (count - 1) as f32;
    let left = position.floor() as usize;
    let right = position.ceil() as usize;
    let progress = position - left as f32;
    AttributeValue::Normalized(points[left] + (points[right] - points[left]) * progress)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub fixtures: Vec<PatchedFixture>,
    pub cue_lists: Vec<CueList>,
    #[serde(default)]
    pub playbacks: Vec<PlaybackDefinition>,
    #[serde(default)]
    pub playback_pages: Vec<PlaybackPage>,
    pub routes: Vec<OutputRoute>,
    pub control_mappings: Vec<light_control::ControlMapping>,
    #[serde(default)]
    pub groups: Vec<GroupDefinition>,
    pub revision: u64,
}

impl EngineSnapshot {
    pub fn validate(&self) -> Result<(), EngineError> {
        validate_patch(&self.fixtures)?;
        let groups = self
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for group in &self.groups {
            if let Some(derived) = &group.derived_from {
                derived.rule.validate().map_err(EngineError::Invalid)?;
            }
            if !group.master.is_finite() || !(0.0..=1.0).contains(&group.master) {
                return Err(EngineError::Invalid(format!(
                    "group {} master must be within 0-1",
                    group.id
                )));
            }
            resolve_group(&group.id, &groups).map_err(EngineError::Invalid)?;
        }
        for cue_list in &self.cue_lists {
            cue_list.validate().map_err(EngineError::Invalid)?;
        }
        let mut playback_numbers = std::collections::HashSet::new();
        for playback in &self.playbacks {
            playback.validate().map_err(EngineError::Invalid)?;
            if !playback_numbers.insert(playback.number) {
                return Err(EngineError::Invalid("duplicate playback number".into()));
            }
            match &playback.target {
                PlaybackTarget::CueList { cue_list_id }
                    if !self.cue_lists.iter().any(|cue| cue.id == *cue_list_id) =>
                {
                    return Err(EngineError::Invalid(
                        "playback references a missing cue list".into(),
                    ));
                }
                PlaybackTarget::Group { group_id }
                    if !self.groups.iter().any(|group| group.id == *group_id) =>
                {
                    return Err(EngineError::Invalid(
                        "playback references a missing group".into(),
                    ));
                }
                _ => {}
            }
        }
        for page in &self.playback_pages {
            page.validate().map_err(EngineError::Invalid)?;
            if page
                .slots
                .values()
                .any(|number| !playback_numbers.contains(number))
            {
                return Err(EngineError::Invalid(
                    "page references a missing playback".into(),
                ));
            }
        }
        for route in &self.routes {
            if route.destination_universe == 0 || route.logical_universe == 0 {
                return Err(EngineError::Invalid(
                    "universe zero is not valid for show routes".into(),
                ));
            }
            if !(1..=light_output::DMX_SLOTS as u16).contains(&route.minimum_slots) {
                return Err(EngineError::Invalid(
                    "route minimum slots must be within 1-512".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RenderOptions {
    pub grand_master: f32,
    pub blackout: bool,
    pub control_loss_progress: Option<f32>,
}
impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            grand_master: 1.0,
            blackout: false,
            control_loss_progress: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderResult {
    pub universes: HashMap<Universe, DmxFrame>,
    /// Highest patched slot for each logical universe. This is kept separately from values so a
    /// patched channel whose default is zero still extends the network payload.
    pub patched_slots: HashMap<Universe, u16>,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveInBlackState {
    Disabled,
    Blocked,
    Delaying,
    Moving,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize)]
pub struct MoveInBlackPosition {
    pub attribute: AttributeKey,
    pub current: AttributeValue,
    pub target: AttributeValue,
}

#[derive(Clone, Debug, Serialize)]
pub struct MoveInBlackDiagnostic {
    pub fixture_id: FixtureId,
    pub playback_number: Option<u16>,
    pub cue_list_id: light_core::CueListId,
    pub current_cue_id: uuid::Uuid,
    pub current_cue_number: f64,
    pub target_cue_id: uuid::Uuid,
    pub target_cue_number: f64,
    pub state: MoveInBlackState,
    pub positions: Vec<MoveInBlackPosition>,
    pub dark_since: Option<chrono::DateTime<chrono::Utc>>,
    pub delay_deadline: Option<chrono::DateTime<chrono::Utc>>,
    pub movement_started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub movement_ends_at: Option<chrono::DateTime<chrono::Utc>>,
    pub cancellation_reason: Option<String>,
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("snapshot validation failed: {0}")]
    Invalid(String),
    #[error(transparent)]
    Fixture(#[from] light_fixture::FixtureError),
}

pub struct Engine {
    snapshot: ArcSwap<EngineSnapshot>,
    playback: RwLock<PlaybackEngine>,
    programmers: ProgrammerRegistry,
    timecode_frame: AtomicU64,
    programmer_fade_millis: AtomicU64,
    /// Exact BPM bits. AtomicU64 keeps snapshot recompilation lock-free without rounding the
    /// operator's decimal Speed Group value to an integer.
    speed_groups_bpm: [AtomicU64; 5],
    speed_groups_paused: [AtomicBool; 5],
    sequence_master_fade_millis: AtomicU64,
    programmer_transitions: Mutex<HashMap<ProgrammerTransitionKey, ProgrammerTransition>>,
    move_in_black: Mutex<HashMap<MoveInBlackKey, MoveInBlackRuntime>>,
    group_master_flashes: RwLock<HashMap<String, f32>>,
    clock: SharedClock,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct MoveInBlackKey {
    playback_number: Option<u16>,
    cue_list_id: light_core::CueListId,
    fixture_id: FixtureId,
}

#[derive(Clone)]
struct MoveInBlackRuntime {
    candidate: MoveInBlackCandidate,
    enabled: bool,
    delay_millis: u64,
    state: MoveInBlackState,
    dark_since: Option<chrono::DateTime<chrono::Utc>>,
    delay_deadline: Option<chrono::DateTime<chrono::Utc>>,
    movement_started_at: Option<chrono::DateTime<chrono::Utc>>,
    movement_ends_at: Option<chrono::DateTime<chrono::Utc>>,
    from: HashMap<AttributeKey, AttributeValue>,
    current: HashMap<AttributeKey, AttributeValue>,
    changed_at: chrono::DateTime<chrono::Utc>,
    handoff_until: Option<chrono::DateTime<chrono::Utc>>,
    cancellation_reason: Option<String>,
}

#[derive(Clone)]
struct ProgrammerTransition {
    changed_at: chrono::DateTime<chrono::Utc>,
    from: AttributeValue,
    target: AttributeValue,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ProgrammerTransitionKey {
    programmer_id: ProgrammerId,
    source: ProgrammerTransitionSource,
    fixture_id: FixtureId,
    attribute: AttributeKey,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ProgrammerTransitionSource {
    Programmer,
    Preload,
    Group(String),
    PreloadGroup(String),
}

impl Engine {
    pub fn new(programmers: ProgrammerRegistry) -> Self {
        let clock = programmers.clock();
        Self {
            snapshot: ArcSwap::from_pointee(EngineSnapshot::default()),
            playback: RwLock::new(PlaybackEngine::with_clock(Arc::clone(&clock))),
            programmers,
            timecode_frame: AtomicU64::new(u64::MAX),
            programmer_fade_millis: AtomicU64::new(0),
            speed_groups_bpm: [
                AtomicU64::new(120.0_f64.to_bits()),
                AtomicU64::new(90.0_f64.to_bits()),
                AtomicU64::new(60.0_f64.to_bits()),
                AtomicU64::new(30.0_f64.to_bits()),
                AtomicU64::new(15.0_f64.to_bits()),
            ],
            speed_groups_paused: std::array::from_fn(|_| AtomicBool::new(false)),
            sequence_master_fade_millis: AtomicU64::new(0),
            programmer_transitions: Mutex::new(HashMap::new()),
            move_in_black: Mutex::new(HashMap::new()),
            group_master_flashes: RwLock::new(HashMap::new()),
            clock,
        }
    }

    pub fn set_control_timing(
        &self,
        speed_groups_bpm: [f64; 5],
        programmer_fade_millis: u64,
        sequence_master_fade_millis: u64,
    ) {
        self.programmer_fade_millis
            .store(programmer_fade_millis.min(60_000), Ordering::Relaxed);
        let speed_groups_bpm = speed_groups_bpm.map(|bpm| {
            if bpm.is_finite() {
                bpm.clamp(0.1, 999.0)
            } else {
                120.0
            }
        });
        for (target, bpm) in self.speed_groups_bpm.iter().zip(speed_groups_bpm) {
            target.store(bpm.to_bits(), Ordering::Relaxed);
        }
        self.sequence_master_fade_millis
            .store(sequence_master_fade_millis.min(60_000), Ordering::Relaxed);
        self.playback
            .write()
            .set_control_timing(speed_groups_bpm, sequence_master_fade_millis);
    }

    pub fn set_speed_groups_paused(&self, paused: [bool; 5]) {
        for (target, paused) in self.speed_groups_paused.iter().zip(paused) {
            target.store(paused, Ordering::Relaxed);
        }
        self.playback.write().set_speed_groups_paused(paused);
    }

    pub fn clear_programmer_transitions(&self) {
        self.programmer_transitions.lock().clear();
    }

    pub fn move_in_black_runtime(&self) -> Vec<MoveInBlackDiagnostic> {
        let mut diagnostics = self
            .move_in_black
            .lock()
            .values()
            .map(|runtime| {
                let mut positions = runtime
                    .candidate
                    .values
                    .iter()
                    .map(|value| MoveInBlackPosition {
                        attribute: value.attribute.clone(),
                        current: runtime
                            .current
                            .get(&value.attribute)
                            .cloned()
                            .unwrap_or_else(|| value.current.clone()),
                        target: value.target.clone(),
                    })
                    .collect::<Vec<_>>();
                positions.sort_by(|left, right| left.attribute.cmp(&right.attribute));
                MoveInBlackDiagnostic {
                    fixture_id: runtime.candidate.fixture_id,
                    playback_number: runtime.candidate.playback_number,
                    cue_list_id: runtime.candidate.cue_list_id,
                    current_cue_id: runtime.candidate.current_cue_id,
                    current_cue_number: runtime.candidate.current_cue_number,
                    target_cue_id: runtime.candidate.target_cue_id,
                    target_cue_number: runtime.candidate.target_cue_number,
                    state: runtime.state,
                    positions,
                    dark_since: runtime.dark_since,
                    delay_deadline: runtime.delay_deadline,
                    movement_started_at: runtime.movement_started_at,
                    movement_ends_at: runtime.movement_ends_at,
                    cancellation_reason: runtime.cancellation_reason.clone(),
                }
            })
            .collect::<Vec<_>>();
        diagnostics.sort_by(|left, right| {
            left.playback_number
                .cmp(&right.playback_number)
                .then_with(|| left.fixture_id.0.cmp(&right.fixture_id.0))
        });
        diagnostics
    }

    fn move_in_black_contributions(
        &self,
        snapshot: &EngineSnapshot,
        candidates: Vec<MoveInBlackCandidate>,
        active: &[ActivePlayback],
        base_resolved: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Vec<TimedValue> {
        let mut runtimes = self.move_in_black.lock();
        let mut present = std::collections::HashSet::new();
        for candidate in candidates {
            let key = MoveInBlackKey {
                playback_number: candidate.playback_number,
                cue_list_id: candidate.cue_list_id,
                fixture_id: candidate.fixture_id,
            };
            present.insert(key);
            let patch = snapshot.fixtures.iter().find(|fixture| {
                fixture.fixture_id == candidate.fixture_id
                    || fixture
                        .logical_heads
                        .iter()
                        .any(|head| head.fixture_id == candidate.fixture_id)
            });
            let enabled = patch.is_some_and(|fixture| fixture.move_in_black_enabled);
            let delay_millis = patch
                .map(|fixture| fixture.move_in_black_delay_millis)
                .unwrap_or_default();
            let base_position = candidate
                .values
                .iter()
                .map(|value| {
                    (
                        value.attribute.clone(),
                        base_resolved
                            .get(&(candidate.fixture_id, value.attribute.clone()))
                            .cloned()
                            .unwrap_or_else(|| value.current.clone()),
                    )
                })
                .collect::<HashMap<_, _>>();
            let resolved_intensity = base_resolved
                .iter()
                .filter(|((fixture_id, attribute), _)| {
                    *fixture_id == candidate.fixture_id && attribute.is_intensity()
                })
                .filter_map(|(_, value)| value.normalized())
                .fold(0.0_f32, f32::max);

            let runtime = runtimes.entry(key).or_insert_with(|| MoveInBlackRuntime {
                candidate: candidate.clone(),
                enabled,
                delay_millis,
                state: if enabled {
                    MoveInBlackState::Blocked
                } else {
                    MoveInBlackState::Disabled
                },
                dark_since: None,
                delay_deadline: None,
                movement_started_at: None,
                movement_ends_at: None,
                from: base_position.clone(),
                current: base_position.clone(),
                changed_at: now,
                handoff_until: None,
                cancellation_reason: None,
            });

            let candidate_changed = runtime.candidate != candidate;
            let enabled_changed = runtime.enabled != enabled;
            let delay_changed = runtime.delay_millis != delay_millis;
            if candidate_changed {
                let previous = move_in_black_values_at(runtime, now);
                let was_dark = runtime.dark_since;
                runtime.candidate = candidate;
                runtime.from = if was_dark.is_some() {
                    previous
                } else {
                    base_position.clone()
                };
                runtime.current = runtime.from.clone();
                runtime.movement_started_at = None;
                runtime.movement_ends_at = None;
                runtime.handoff_until = None;
                runtime.changed_at = now;
                runtime.cancellation_reason = Some("future_target_recalculated".into());
            }
            runtime.enabled = enabled;
            runtime.delay_millis = delay_millis;

            if !enabled {
                runtime.state = MoveInBlackState::Disabled;
                runtime.dark_since = None;
                runtime.delay_deadline = None;
                runtime.movement_started_at = None;
                runtime.movement_ends_at = None;
                runtime.handoff_until = None;
                runtime.from = base_position.clone();
                runtime.current = base_position;
                runtime.cancellation_reason = None;
                continue;
            }

            if resolved_intensity > 0.0 {
                if runtime.dark_since.is_some() {
                    runtime.cancellation_reason = Some("resolved_intensity_above_zero".into());
                } else {
                    runtime.cancellation_reason = None;
                }
                runtime.state = MoveInBlackState::Blocked;
                runtime.dark_since = None;
                runtime.delay_deadline = None;
                runtime.movement_started_at = None;
                runtime.movement_ends_at = None;
                runtime.handoff_until = None;
                runtime.from = base_position.clone();
                runtime.current = base_position;
                continue;
            }

            if enabled_changed || runtime.dark_since.is_none() {
                runtime.dark_since = Some(now);
                runtime.delay_deadline = Some(
                    now + chrono::Duration::milliseconds(delay_millis.min(i64::MAX as u64) as i64),
                );
                runtime.movement_started_at = None;
                runtime.movement_ends_at = None;
                runtime.from = base_position.clone();
                runtime.current = base_position;
                runtime.changed_at = now;
                runtime.cancellation_reason = None;
            } else if delay_changed && let Some(dark_since) = runtime.dark_since {
                runtime.delay_deadline = Some(
                    dark_since
                        + chrono::Duration::milliseconds(delay_millis.min(i64::MAX as u64) as i64),
                );
                runtime.movement_started_at = None;
                runtime.movement_ends_at = None;
                runtime.from = base_position;
                runtime.changed_at = now;
            }

            let deadline = runtime.delay_deadline.expect("dark runtime has a deadline");
            if now < deadline {
                runtime.state = MoveInBlackState::Delaying;
                runtime.current = runtime.from.clone();
                continue;
            }
            if runtime.movement_started_at.is_none() {
                let started_at = if candidate_changed { now } else { deadline };
                let longest_fade = runtime
                    .candidate
                    .values
                    .iter()
                    .map(|value| value.fade_millis)
                    .max()
                    .unwrap_or(0);
                runtime.movement_started_at = Some(started_at);
                runtime.movement_ends_at = Some(
                    started_at
                        + chrono::Duration::milliseconds(longest_fade.min(i64::MAX as u64) as i64),
                );
                runtime.changed_at = started_at;
            }
            runtime.current = move_in_black_values_at(runtime, now);
            runtime.state = if runtime
                .movement_ends_at
                .is_some_and(|ends_at| now >= ends_at)
            {
                MoveInBlackState::Completed
            } else {
                MoveInBlackState::Moving
            };
        }

        for (key, runtime) in runtimes.iter_mut() {
            if present.contains(key) {
                continue;
            }
            if !runtime.enabled {
                runtime.state = MoveInBlackState::Disabled;
                runtime.cancellation_reason = None;
                continue;
            }
            let target_active = active.iter().find(|playback| {
                playback.enabled
                    && playback.playback_number == key.playback_number
                    && playback.cue_list_id == key.cue_list_id
                    && playback.current_cue_id == Some(runtime.candidate.target_cue_id)
            });
            if let Some(playback) = target_active
                && matches!(
                    runtime.state,
                    MoveInBlackState::Moving | MoveInBlackState::Completed
                )
            {
                if runtime.handoff_until.is_none() {
                    let current = move_in_black_values_at(runtime, now);
                    let longest_fade = runtime
                        .candidate
                        .values
                        .iter()
                        .map(|value| value.fade_millis)
                        .max()
                        .unwrap_or(0);
                    runtime.from = current;
                    runtime.movement_started_at = Some(playback.activated_at);
                    runtime.movement_ends_at = Some(
                        playback.activated_at
                            + chrono::Duration::milliseconds(
                                longest_fade.min(i64::MAX as u64) as i64
                            ),
                    );
                    runtime.handoff_until = runtime.movement_ends_at;
                    runtime.changed_at = playback.activated_at + chrono::Duration::microseconds(1);
                    runtime.cancellation_reason = None;
                }
                runtime.current = move_in_black_values_at(runtime, now);
                runtime.state = if move_in_black_is_at_target(runtime)
                    || runtime.handoff_until.is_some_and(|until| now >= until)
                {
                    MoveInBlackState::Completed
                } else {
                    MoveInBlackState::Moving
                };
            } else if runtime.state != MoveInBlackState::Cancelled {
                runtime.state = MoveInBlackState::Cancelled;
                runtime.handoff_until = None;
                runtime.cancellation_reason = Some(
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
        }

        runtimes
            .iter()
            .filter(|(key, runtime)| {
                runtime.enabled
                    && (present.contains(key)
                        || runtime.handoff_until.is_some_and(|until| now < until))
                    && matches!(
                        runtime.state,
                        MoveInBlackState::Moving | MoveInBlackState::Completed
                    )
            })
            .flat_map(|(_, runtime)| {
                runtime.current.iter().map(|(attribute, value)| TimedValue {
                    fixture_id: runtime.candidate.fixture_id,
                    attribute: attribute.clone(),
                    value: value.clone(),
                    priority: runtime.candidate.priority,
                    changed_at: runtime.changed_at,
                    programmer_order: 0,
                    merge_mode: MergeMode::Ltp,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                })
            })
            .collect()
    }

    fn faded_programmer_value(
        &self,
        mut value: TimedValue,
        now: chrono::DateTime<chrono::Utc>,
        underlying: Option<&AttributeValue>,
        programmer_id: ProgrammerId,
        source: ProgrammerTransitionSource,
    ) -> TimedValue {
        let duration = value
            .fade_millis
            .unwrap_or_else(|| self.programmer_fade_millis.load(Ordering::Relaxed));
        if duration == 0 || value.value.normalized().is_none() {
            return value;
        }
        let key = ProgrammerTransitionKey {
            programmer_id,
            source,
            fixture_id: value.fixture_id,
            attribute: value.attribute.clone(),
        };
        let mut transitions = self.programmer_transitions.lock();
        let transition = transitions
            .entry(key)
            .or_insert_with(|| ProgrammerTransition {
                changed_at: value.changed_at,
                from: underlying
                    .cloned()
                    .unwrap_or(AttributeValue::Normalized(0.0)),
                target: value.value.clone(),
            });
        let interpolate = |transition: &ProgrammerTransition| {
            let elapsed = (now - transition.changed_at).num_milliseconds().max(0) as u64;
            let elapsed = elapsed.saturating_sub(value.delay_millis.unwrap_or(0));
            let progress = (elapsed as f32 / duration as f32).clamp(0.0, 1.0);
            match (transition.from.normalized(), transition.target.normalized()) {
                (Some(from), Some(target)) => {
                    AttributeValue::Normalized(from + (target - from) * progress)
                }
                _ => transition.target.clone(),
            }
        };
        if transition.changed_at != value.changed_at || transition.target != value.value {
            let from = interpolate(transition);
            *transition = ProgrammerTransition {
                changed_at: value.changed_at,
                from,
                target: value.value.clone(),
            };
        }
        value.value = interpolate(transition);
        value
    }

    pub fn replace_snapshot(&self, snapshot: EngineSnapshot) -> Result<(), EngineError> {
        self.replace_snapshot_with_playback_policy(snapshot, true)
    }

    /// Validates every runtime-dependent part of a candidate snapshot without mutating the live
    /// engine. Server persistence uses this preflight so an invalid Chaser or playback assignment
    /// cannot be written first and rejected only during the subsequent live-engine refresh.
    pub fn validate_snapshot_for_runtime(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Result<(), EngineError> {
        snapshot.validate()?;
        self.compile_playback(snapshot).map(|_| ())
    }

    pub fn replace_snapshot_releasing_playback(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<(), EngineError> {
        self.replace_snapshot_with_playback_policy(snapshot, false)
    }
    fn replace_snapshot_with_playback_policy(
        &self,
        snapshot: EngineSnapshot,
        preserve_playback: bool,
    ) -> Result<(), EngineError> {
        snapshot.validate()?;
        let (active_playbacks, dynamics_paused_at) = if preserve_playback {
            let playback = self.playback.read();
            (
                playback.active_for_snapshot(&snapshot.cue_lists, self.clock.now()),
                playback.dynamics_paused_since(),
            )
        } else {
            (Vec::new(), None)
        };
        let (mut playback, groups) = self.compile_playback(&snapshot)?;
        self.programmers.refresh_live_selections(&groups);
        playback.restore_active(active_playbacks);
        playback.restore_dynamics_paused_since(dynamics_paused_at);
        *self.playback.write() = playback;
        self.snapshot.store(Arc::new(snapshot));
        Ok(())
    }

    fn compile_playback(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Result<(PlaybackEngine, HashMap<String, GroupDefinition>), EngineError> {
        let mut playback = PlaybackEngine::with_clock(Arc::clone(&self.clock));
        playback.set_control_timing(
            self.speed_groups_bpm
                .each_ref()
                .map(|bpm| f64::from_bits(bpm.load(Ordering::Relaxed))),
            self.sequence_master_fade_millis.load(Ordering::Relaxed),
        );
        playback.set_speed_groups_paused(
            self.speed_groups_paused
                .each_ref()
                .map(|paused| paused.load(Ordering::Relaxed)),
        );
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for source in &snapshot.cue_lists {
            let mut cue_list = source.clone();
            for cue in &mut cue_list.cues {
                let mut expanded_addresses = cue
                    .changes
                    .iter()
                    .map(|change| (change.fixture_id, change.attribute.clone()))
                    .collect::<std::collections::HashSet<_>>();
                for change in cue.group_changes.clone() {
                    if let Ok(fixtures) = resolve_group(&change.group_id, &groups) {
                        let count = fixtures.len();
                        for (index, fixture_id) in fixtures.into_iter().enumerate() {
                            if expanded_addresses.insert((fixture_id, change.attribute.clone())) {
                                cue.changes.push(light_playback::CueChange {
                                    fixture_id,
                                    attribute: change.attribute.clone(),
                                    value: change.value.as_ref().map(|value| {
                                        value_for_ordered_position(value, index, count)
                                    }),
                                    automatic_restore: false,
                                    fade_millis: change.fade_millis,
                                    delay_millis: change.delay_millis,
                                });
                            }
                        }
                    }
                }
                for phaser in &mut cue.phasers {
                    for group_id in &phaser.group_ids {
                        if let Ok(fixtures) = resolve_group(group_id, &groups) {
                            for fixture in fixtures {
                                if !phaser.fixture_ids.contains(&fixture) {
                                    phaser.fixture_ids.push(fixture);
                                }
                            }
                        }
                    }
                }
            }
            playback.register(cue_list).map_err(EngineError::Invalid)?;
        }
        for definition in snapshot.playbacks.clone() {
            playback
                .register_definition(definition)
                .map_err(EngineError::Invalid)?;
        }
        Ok((playback, groups))
    }

    pub fn snapshot(&self) -> Arc<EngineSnapshot> {
        self.snapshot.load_full()
    }
    pub fn playback(&self) -> &RwLock<PlaybackEngine> {
        &self.playback
    }
    pub fn set_timecode_frame(&self, frame: Option<u64>) {
        self.timecode_frame
            .store(frame.unwrap_or(u64::MAX), Ordering::Relaxed);
    }

    /// Sets a transient group flash level without changing the group's fader value.
    pub fn set_group_master_flash(&self, group_id: String, value: f32) {
        let mut flashes = self.group_master_flashes.write();
        if value <= 0.0 {
            flashes.remove(&group_id);
        } else {
            flashes.insert(group_id, value.clamp(0.0, 1.0));
        }
    }

    pub fn group_master_flash(&self, group_id: &str) -> f32 {
        self.group_master_flashes
            .read()
            .get(group_id)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn render(&self, options: RenderOptions) -> Result<RenderResult, EngineError> {
        let snapshot = self.snapshot.load_full();
        let now = self.clock.now();
        let resolved = self.resolved_values_at(&snapshot, now);
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        let group_master_flashes = self.group_master_flashes.read();
        let mut universes = HashMap::new();
        let mut patched_slots: HashMap<Universe, u16> = HashMap::new();
        for fixture in &snapshot.fixtures {
            let mut patches = vec![(fixture.universe, fixture.address)];
            patches.extend(
                fixture
                    .multipatch
                    .iter()
                    .map(|instance| (instance.universe, instance.address)),
            );
            for (universe, address) in patches {
                let (Some(universe), Some(address)) = (universe, address) else {
                    continue;
                };
                let frame = universes.entry(universe).or_insert([0; 512]);
                let last_slot = address
                    .saturating_sub(1)
                    .saturating_add(fixture.definition.footprint)
                    .min(light_output::DMX_SLOTS as u16);
                patched_slots
                    .entry(universe)
                    .and_modify(|current| *current = (*current).max(last_slot))
                    .or_insert(last_slot);
                let mut instance = fixture.clone();
                instance.universe = Some(universe);
                instance.address = Some(address);
                render_fixture(
                    frame,
                    &instance,
                    &resolved,
                    options,
                    &groups,
                    &group_master_flashes,
                )?;
            }
        }
        Ok(RenderResult {
            universes,
            patched_slots,
            revision: snapshot.revision,
        })
    }

    /// Returns the same merged abstract attributes that feed DMX rendering. Consumers such as
    /// visualizers can use this without attempting to reverse fixture-specific DMX encoding.
    pub fn resolved_values(&self) -> HashMap<(FixtureId, AttributeKey), AttributeValue> {
        let snapshot = self.snapshot.load_full();
        self.resolved_values_at(&snapshot, self.clock.now())
    }

    fn resolved_values_at(
        &self,
        snapshot: &EngineSnapshot,
        now: chrono::DateTime<chrono::Utc>,
    ) -> HashMap<(FixtureId, AttributeKey), AttributeValue> {
        let timecode = self.timecode_frame.load(Ordering::Relaxed);
        let (mut contributions, move_in_black_candidates, active_playbacks) = {
            let mut playback = self.playback.write();
            playback.tick(now, (timecode != u64::MAX).then_some(timecode));
            (
                playback.contributions_at(now),
                playback.move_in_black_candidates(),
                playback.runtime(),
            )
        };
        // A newly faded programmer source starts at the resolved playback underneath it. This is
        // especially visible for Preload: GO must not introduce a zero/default frame before the
        // temporary programmer contribution takes ownership.
        let programmer_underlay = resolve(contributions.clone());
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for programmer in self.programmers.active() {
            let programmer_id = programmer.id;
            let programmer_priority = programmer.priority;
            let mut scoped_contributions = Vec::new();
            for (value, source) in programmer
                .values
                .into_iter()
                .map(|value| (value, ProgrammerTransitionSource::Programmer))
                .chain(
                    programmer
                        .preload_active
                        .into_iter()
                        .map(|value| (value, ProgrammerTransitionSource::Preload)),
                )
            {
                scoped_contributions.push(if value.fade {
                    let underlying =
                        programmer_underlay.get(&(value.fixture_id, value.attribute.clone()));
                    self.faded_programmer_value(value, now, underlying, programmer_id, source)
                } else {
                    value
                });
            }
            for (group_id, attributes, source) in
                programmer
                    .group_values
                    .into_iter()
                    .map(|(group_id, attributes)| {
                        let source = ProgrammerTransitionSource::Group(group_id.clone());
                        (group_id, attributes, source)
                    })
                    .chain(programmer.preload_group_active.into_iter().map(
                        |(group_id, attributes)| {
                            let source = ProgrammerTransitionSource::PreloadGroup(group_id.clone());
                            (group_id, attributes, source)
                        },
                    ))
            {
                let Ok(fixtures) = resolve_group(&group_id, &groups) else {
                    continue;
                };
                let count = fixtures.len();
                for (index, fixture_id) in fixtures.into_iter().enumerate() {
                    for (attribute, scoped) in &attributes {
                        let value = TimedValue {
                            fixture_id,
                            attribute: attribute.clone(),
                            value: value_for_ordered_position(&scoped.value, index, count),
                            priority: programmer_priority,
                            changed_at: scoped.changed_at,
                            programmer_order: scoped.programmer_order,
                            merge_mode: MergeMode::Ltp,
                            fade: scoped.fade,
                            fade_millis: scoped.fade_millis,
                            delay_millis: scoped.delay_millis,
                        };
                        scoped_contributions.push(if value.fade {
                            let underlying = programmer_underlay
                                .get(&(value.fixture_id, value.attribute.clone()));
                            self.faded_programmer_value(
                                value,
                                now,
                                underlying,
                                programmer_id,
                                source.clone(),
                            )
                        } else {
                            value
                        });
                    }
                }
            }
            // Fixture and live-Group values remain LTP within one programmer. Only after that
            // programmer-local scope has one winner per address do intensity values participate in
            // desk-wide HTP against other programmers and playbacks at the same numeric priority.
            let mut programmer_winners = HashMap::new();
            for value in scoped_contributions {
                let key = (value.fixture_id, value.attribute.clone());
                let replace = programmer_winners
                    .get(&key)
                    .is_none_or(|current: &TimedValue| {
                        (value.changed_at, value.programmer_order)
                            > (current.changed_at, current.programmer_order)
                    });
                if replace {
                    programmer_winners.insert(key, value);
                }
            }
            contributions.extend(programmer_winners.into_values().map(|mut value| {
                value.merge_mode = if value.attribute.is_intensity() {
                    MergeMode::Htp
                } else {
                    MergeMode::Ltp
                };
                value
            }));
        }
        for group in &snapshot.groups {
            let Ok(fixtures) = resolve_group(&group.id, &groups) else {
                continue;
            };
            for fixture_id in fixtures {
                for (attribute, value) in &group.programming {
                    contributions.push(TimedValue {
                        fixture_id,
                        attribute: attribute.clone(),
                        value: value.clone(),
                        priority: 0,
                        changed_at: now,
                        programmer_order: 0,
                        merge_mode: if attribute.is_intensity() {
                            MergeMode::Htp
                        } else {
                            MergeMode::Ltp
                        },
                        fade: false,
                        fade_millis: None,
                        delay_millis: None,
                    });
                }
            }
        }
        let base_resolved = resolve(contributions.clone());
        contributions.extend(self.move_in_black_contributions(
            snapshot,
            move_in_black_candidates,
            &active_playbacks,
            &base_resolved,
            now,
        ));
        resolve(contributions)
    }
}

fn move_in_black_values_at(
    runtime: &MoveInBlackRuntime,
    now: chrono::DateTime<chrono::Utc>,
) -> HashMap<AttributeKey, AttributeValue> {
    let Some(started_at) = runtime.movement_started_at else {
        return runtime.from.clone();
    };
    runtime
        .candidate
        .values
        .iter()
        .map(|target| {
            let from = runtime
                .from
                .get(&target.attribute)
                .unwrap_or(&target.current);
            let elapsed = (now - started_at).num_milliseconds().max(0) as u64;
            let progress = if target.fade_millis == 0 {
                1.0
            } else {
                (elapsed as f32 / target.fade_millis as f32).clamp(0.0, 1.0)
            };
            let value = match (from.normalized(), target.target.normalized()) {
                (Some(from), Some(to)) => AttributeValue::Normalized(from + (to - from) * progress),
                _ if progress >= 1.0 => target.target.clone(),
                _ => from.clone(),
            };
            (target.attribute.clone(), value)
        })
        .collect()
}

fn move_in_black_is_at_target(runtime: &MoveInBlackRuntime) -> bool {
    runtime.candidate.values.iter().all(|target| {
        runtime
            .current
            .get(&target.attribute)
            .is_some_and(
                |current| match (current.normalized(), target.target.normalized()) {
                    (Some(current), Some(target)) => (current - target).abs() <= f32::EPSILON * 8.0,
                    _ => current == &target.target,
                },
            )
    })
}
fn render_fixture(
    frame: &mut DmxFrame,
    fixture: &PatchedFixture,
    resolved: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
    options: RenderOptions,
    groups: &HashMap<String, GroupDefinition>,
    group_master_flashes: &HashMap<String, f32>,
) -> Result<(), EngineError> {
    let Some(address) = fixture.address else {
        return Ok(());
    };
    for head in &fixture.definition.heads {
        let owner = if head.shared {
            fixture.fixture_id
        } else {
            fixture
                .logical_heads
                .iter()
                .find(|patched| patched.head_index == head.index)
                .map(|patched| patched.fixture_id)
                .unwrap_or(fixture.fixture_id)
        };
        let group_scale = groups
            .values()
            .filter(|group| group.playback_fader.is_some())
            .filter_map(|group| {
                resolve_group(&group.id, groups)
                    .ok()
                    .filter(|members| members.contains(&owner))
                    .map(|_| {
                        group
                            .master
                            .max(group_master_flashes.get(&group.id).copied().unwrap_or(0.0))
                            .clamp(0.0, 1.0)
                    })
            })
            .reduce(f32::max)
            .unwrap_or(1.0);
        let mut abstract_values: HashMap<AttributeKey, AttributeValue> = resolved
            .iter()
            .filter(|((fixture_id, _), _)| *fixture_id == owner)
            .map(|((_, attribute), value)| (attribute.clone(), value.clone()))
            .collect();
        if let Some(progress) = options.control_loss_progress {
            match fixture.definition.effective_signal_loss_policy() {
                SignalLossPolicy::HoldLast => {}
                SignalLossPolicy::ImmediateSafe => {
                    apply_safe_values(&mut abstract_values, &fixture.definition.safe_values, 1.0)
                }
                SignalLossPolicy::FadeToSafe { .. } => apply_safe_values(
                    &mut abstract_values,
                    &fixture.definition.safe_values,
                    progress.clamp(0.0, 1.0),
                ),
            }
        }
        if fixture.definition.hazardous && options.blackout {
            for (attribute, value) in &fixture.definition.safe_values {
                abstract_values.insert(attribute.clone(), value.clone());
            }
        }
        let intensity_key = AttributeKey::intensity();
        let intensity = if options.blackout {
            0.0
        } else {
            abstract_values
                .get(&intensity_key)
                .and_then(AttributeValue::normalized)
                .unwrap_or(1.0)
                * group_scale
                * options.grand_master.clamp(0.0, 1.0)
        };
        let has_physical_dimmer = head
            .parameters
            .iter()
            .any(|parameter| parameter.attribute.is_intensity() && !parameter.virtual_dimmer);
        if let (Some(AttributeValue::ColorXyz(color)), Some(calibration)) = (
            abstract_values.get(&AttributeKey("color".into())),
            &fixture.definition.color_calibration,
        ) {
            let mut levels = mix_color(*color, calibration)?;
            if !has_physical_dimmer {
                for level in &mut levels {
                    *level *= intensity;
                }
            }
            for (emitter, level) in calibration.emitters.iter().zip(levels) {
                abstract_values
                    .entry(AttributeKey(format!(
                        "color.emitter.{}",
                        emitter.name.to_lowercase()
                    )))
                    .or_insert(AttributeValue::Normalized(level));
            }
        }
        for parameter in &head.parameters {
            let mut level = abstract_values
                .get(&parameter.attribute)
                .and_then(AttributeValue::normalized)
                .unwrap_or(parameter.default);
            if parameter.attribute.is_intensity() {
                level *= group_scale;
                level *= options.grand_master.clamp(0.0, 1.0);
                if options.blackout {
                    level = 0.0;
                }
            }
            if parameter.virtual_dimmer {
                level *= intensity;
            }
            if parameter.components.is_empty() {
                continue;
            }
            encode_parameter(frame, address, parameter, level)?;
        }
        for (attribute, value) in &abstract_values {
            if let (Some(offset), AttributeValue::RawDmx(raw)) = (
                attribute
                    .0
                    .strip_prefix("dmx.")
                    .and_then(|offset| offset.parse::<u16>().ok()),
                value,
            ) && offset < fixture.definition.footprint
            {
                frame[usize::from(address - 1 + offset)] = *raw;
            }
        }
    }
    Ok(())
}

fn apply_safe_values(
    values: &mut HashMap<AttributeKey, AttributeValue>,
    safe: &std::collections::BTreeMap<AttributeKey, AttributeValue>,
    progress: f32,
) {
    for (attribute, target) in safe {
        let value = match (values.get(attribute), target) {
            (Some(AttributeValue::Normalized(current)), AttributeValue::Normalized(target)) => {
                AttributeValue::Normalized(current + (target - current) * progress)
            }
            _ if progress >= 1.0 => target.clone(),
            (Some(current), _) => current.clone(),
            (None, AttributeValue::Normalized(target)) => {
                AttributeValue::Normalized(target * progress)
            }
            _ => continue,
        };
        values.insert(attribute.clone(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration as ChronoDuration, TimeZone, Utc};
    use light_core::{ApplicationClock, ManualClock, SessionId, UserId};
    use light_fixture::{
        ByteOrder, ChannelComponent, FixtureDefinition, LogicalHead, MultiPatchInstance, Parameter,
        PatchedHead,
    };
    use light_playback::{
        Cue, CueChange, CueListMode, IntensityPriorityMode, PlaybackButtonAction,
        PlaybackFaderMode, RestartMode, WrapMode,
    };
    use std::{collections::BTreeMap, sync::Arc};

    fn fixture() -> (PatchedFixture, FixtureId) {
        let physical = FixtureId::new();
        let logical = FixtureId::new();
        let parameter = Parameter {
            attribute: AttributeKey::intensity(),
            components: vec![ChannelComponent {
                offset: 0,
                byte_order: ByteOrder::MsbFirst,
            }],
            default: 0.0,
            virtual_dimmer: false,
            metadata: light_fixture::ParameterMetadata::default(),
            capabilities: vec![],
        };
        (
            PatchedFixture {
                fixture_id: physical,
                fixture_number: None,
                name: "Cell".into(),
                layer_id: "default".into(),
                definition: FixtureDefinition {
                    schema_version: 1,
                    id: FixtureId::new(),
                    revision: 1,
                    manufacturer: "Test".into(),
                    device_type: "other".into(),
                    name: "Cell".into(),
                    model: "Cell".into(),
                    mode: "1ch".into(),
                    footprint: 1,
                    heads: vec![LogicalHead {
                        index: 1,
                        name: "Cell".into(),
                        shared: false,
                        parameters: vec![parameter],
                    }],
                    color_calibration: None,
                    physical: Default::default(),
                    model_asset: None,
                    icon_asset: None,
                    hazardous: false,
                    direct_control_protocols: Vec::new(),
                    signal_loss_policy: SignalLossPolicy::HoldLast,
                    safe_values: BTreeMap::new(),
                },
                universe: Some(1),
                address: Some(1),
                direct_control: None,
                location: Default::default(),
                rotation: Default::default(),
                logical_heads: vec![PatchedHead {
                    head_index: 1,
                    fixture_id: logical,
                }],
                multipatch: Vec::new(),
                move_in_black_enabled: true,
                move_in_black_delay_millis: 0,
            },
            logical,
        )
    }

    fn moving_fixture(
        address: u16,
        enabled: bool,
        delay_millis: u64,
    ) -> (PatchedFixture, FixtureId) {
        let (mut fixture, logical) = fixture();
        fixture.address = Some(address);
        fixture.definition.footprint = 2;
        fixture.definition.heads[0].parameters.push(Parameter {
            attribute: AttributeKey("pan".into()),
            components: vec![ChannelComponent {
                offset: 1,
                byte_order: ByteOrder::MsbFirst,
            }],
            default: 0.0,
            virtual_dimmer: false,
            metadata: light_fixture::ParameterMetadata::default(),
            capabilities: vec![],
        });
        fixture.move_in_black_enabled = enabled;
        fixture.move_in_black_delay_millis = delay_millis;
        (fixture, logical)
    }

    fn mib_snapshot(fixtures: Vec<PatchedFixture>, fixture_ids: &[FixtureId]) -> EngineSnapshot {
        let mut first = Cue::new(1.0);
        let mut dark = Cue::new(2.0);
        dark.fade_millis = 2_000;
        let mut lit = Cue::new(3.0);
        for fixture_id in fixture_ids {
            first.changes.push(CueChange::set(
                *fixture_id,
                AttributeKey::intensity(),
                AttributeValue::Normalized(1.0),
            ));
            first.changes.push(CueChange::set(
                *fixture_id,
                AttributeKey("pan".into()),
                AttributeValue::Normalized(0.2),
            ));
            dark.changes.push(CueChange::set(
                *fixture_id,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.0),
            ));
            lit.changes.push(CueChange::set(
                *fixture_id,
                AttributeKey::intensity(),
                AttributeValue::Normalized(1.0),
            ));
            let mut position = CueChange::set(
                *fixture_id,
                AttributeKey("pan".into()),
                AttributeValue::Normalized(0.8),
            );
            position.fade_millis = Some(3_000);
            lit.changes.push(position);
        }
        let cue_list = CueList {
            id: light_core::CueListId::new(),
            name: "MIB".into(),
            priority: 10,
            mode: CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: IntensityPriorityMode::Htp,
            wrap_mode: Some(WrapMode::Off),
            restart_mode: RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            speed_multiplier: 1.0,
            cues: vec![first, dark, lit],
        };
        let playback = PlaybackDefinition {
            number: 1,
            name: "MIB".into(),
            target: PlaybackTarget::CueList {
                cue_list_id: cue_list.id,
            },
            buttons: [
                PlaybackButtonAction::GoMinus,
                PlaybackButtonAction::Go,
                PlaybackButtonAction::Flash,
            ],
            button_count: 3,
            fader: PlaybackFaderMode::Master,
            has_fader: true,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        };
        EngineSnapshot {
            fixtures,
            cue_lists: vec![cue_list],
            playbacks: vec![playback],
            playback_pages: vec![],
            routes: vec![],
            control_mappings: vec![],
            groups: vec![],
            revision: 1,
        }
    }

    fn normalized(
        values: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
        fixture_id: FixtureId,
        attribute: &str,
    ) -> f32 {
        values[&(fixture_id, AttributeKey(attribute.into()))]
            .normalized()
            .unwrap()
    }

    #[test]
    fn patched_multipatch_instances_duplicate_output_while_visual_only_instances_do_not() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (mut fixture, logical) = fixture();
        fixture.multipatch = vec![
            MultiPatchInstance {
                id: FixtureId::new().0,
                name: "Patched clone".into(),
                universe: Some(1),
                address: Some(8),
                location: Default::default(),
                rotation: Default::default(),
            },
            MultiPatchInstance {
                id: FixtureId::new().0,
                name: "Visualizer clone".into(),
                universe: None,
                address: None,
                location: Default::default(),
                rotation: Default::default(),
            },
        ];
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![],
                playbacks: vec![],
                playback_pages: vec![],
                routes: vec![],
                control_mappings: vec![],
                groups: vec![],
                revision: 1,
            })
            .unwrap();
        let result = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(result.universes[&1][0], 128);
        assert_eq!(result.universes[&1][7], 128);
        assert_eq!(
            result.universes[&1]
                .iter()
                .filter(|value| **value != 0)
                .count(),
            2
        );
    }

    #[test]
    fn logical_head_programmer_value_renders_to_physical_patch() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (fixture, logical) = fixture();
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![],
                playbacks: vec![],
                playback_pages: vec![],
                routes: vec![],
                control_mappings: vec![],
                groups: vec![],
                revision: 7,
            })
            .unwrap();
        let result = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(result.universes[&1][0], 128);
        assert_eq!(result.revision, 7);
        assert_eq!(
            engine
                .resolved_values()
                .get(&(logical, AttributeKey::intensity())),
            Some(&AttributeValue::Normalized(0.5))
        );
    }

    #[test]
    fn parent_programmer_value_does_not_fan_out_to_child_heads() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (fixture, _) = fixture();
        programmers.set(
            session,
            fixture.fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            0
        );
    }

    #[test]
    fn master_only_group_fader_does_not_scale_child_heads() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (mut fixture, child) = fixture();
        fixture.definition.footprint = 2;
        let mut master_parameter = fixture.definition.heads[0].parameters[0].clone();
        master_parameter.components[0].offset = 1;
        fixture.definition.heads.insert(
            0,
            LogicalHead {
                index: 0,
                name: "Master".into(),
                shared: true,
                parameters: vec![master_parameter],
            },
        );
        let master = fixture.fixture_id;
        for fixture_id in [master, child] {
            programmers.set(
                session,
                fixture_id,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.8),
            );
        }
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    id: "master".into(),
                    name: "Master only".into(),
                    fixtures: vec![master],
                    master: 0.5,
                    playback_fader: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        let rendered = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(rendered.universes[&1][0], 204);
        assert_eq!(rendered.universes[&1][1], 102);
    }

    #[test]
    fn grand_master_and_blackout_affect_intensity() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (fixture, logical) = fixture();
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![],
                playbacks: vec![],
                playback_pages: vec![],
                routes: vec![],
                control_mappings: vec![],
                groups: vec![],
                revision: 1,
            })
            .unwrap();
        assert_eq!(
            engine
                .render(RenderOptions {
                    grand_master: 0.5,
                    blackout: false,
                    control_loss_progress: None,
                })
                .unwrap()
                .universes[&1][0],
            128
        );
        assert_eq!(
            engine
                .render(RenderOptions {
                    grand_master: 1.0,
                    blackout: true,
                    control_loss_progress: None,
                })
                .unwrap()
                .universes[&1][0],
            0
        );
    }

    #[test]
    fn group_masters_scale_before_encoding_and_use_highest_master() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (fixture, logical) = fixture();
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.8),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![],
                playbacks: vec![],
                playback_pages: vec![],
                routes: vec![],
                control_mappings: vec![],
                groups: vec![
                    GroupDefinition {
                        id: "a".into(),
                        name: "A".into(),
                        fixtures: vec![logical],
                        master: 0.5,
                        playback_fader: Some(1),
                        ..Default::default()
                    },
                    GroupDefinition {
                        id: "b".into(),
                        name: "B".into(),
                        fixtures: vec![logical],
                        master: 0.75,
                        playback_fader: Some(2),
                        ..Default::default()
                    },
                    GroupDefinition {
                        id: "unassigned".into(),
                        name: "Unassigned".into(),
                        fixtures: vec![logical],
                        master: 1.0,
                        playback_fader: None,
                        ..Default::default()
                    },
                ],
                revision: 1,
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            153
        );
    }

    #[test]
    fn group_master_flash_is_temporary_and_does_not_move_the_fader() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (fixture, logical) = fixture();
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.8),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    id: "front".into(),
                    name: "Front".into(),
                    fixtures: vec![logical],
                    master: 0.25,
                    playback_fader: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();

        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            51
        );
        engine.set_group_master_flash("front".into(), 1.0);
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            204
        );
        assert_eq!(engine.snapshot().groups[0].master, 0.25);
        engine.set_group_master_flash("front".into(), 0.0);
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            51
        );
    }
    #[test]
    fn logical_head_master_does_not_limit_sibling_heads() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let physical = FixtureId::new();
        let first = FixtureId::new();
        let second = FixtureId::new();
        let parameter = |offset| Parameter {
            attribute: AttributeKey::intensity(),
            components: vec![ChannelComponent {
                offset,
                byte_order: light_fixture::ByteOrder::MsbFirst,
            }],
            default: 0.0,
            virtual_dimmer: false,
            metadata: light_fixture::ParameterMetadata::default(),
            capabilities: vec![],
        };
        let fixture = PatchedFixture {
            fixture_id: physical,
            fixture_number: None,
            name: "Two cell".into(),
            layer_id: "default".into(),
            definition: FixtureDefinition {
                schema_version: 1,
                id: FixtureId::new(),
                revision: 1,
                manufacturer: "Test".into(),
                device_type: "other".into(),
                name: "Two cell".into(),
                model: "Two cell".into(),
                mode: "2ch".into(),
                footprint: 2,
                heads: vec![
                    LogicalHead {
                        index: 1,
                        name: "One".into(),
                        shared: false,
                        parameters: vec![parameter(0)],
                    },
                    LogicalHead {
                        index: 2,
                        name: "Two".into(),
                        shared: false,
                        parameters: vec![parameter(1)],
                    },
                ],
                color_calibration: None,
                physical: Default::default(),
                model_asset: None,
                icon_asset: None,
                hazardous: false,
                direct_control_protocols: vec![],
                signal_loss_policy: SignalLossPolicy::HoldLast,
                safe_values: BTreeMap::new(),
            },
            universe: Some(1),
            address: Some(1),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![
                PatchedHead {
                    head_index: 1,
                    fixture_id: first,
                },
                PatchedHead {
                    head_index: 2,
                    fixture_id: second,
                },
            ],
            multipatch: vec![],
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
        };
        for fixture_id in [first, second] {
            programmers.set(
                session,
                fixture_id,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.8),
            );
        }
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    id: "first".into(),
                    name: "First".into(),
                    fixtures: vec![first],
                    master: 0.5,
                    playback_fader: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        let rendered = engine.render(RenderOptions::default()).unwrap();
        let frame = &rendered.universes[&1];
        assert_eq!(frame[0], 102);
        assert_eq!(frame[1], 204);
    }
    #[test]
    fn group_ltp_uses_operator_edit_time_not_render_time() {
        let programmers = ProgrammerRegistry::default();
        let group_session = light_core::SessionId::new();
        let direct_session = light_core::SessionId::new();
        programmers.start(group_session, light_core::UserId::new());
        programmers.start(direct_session, light_core::UserId::new());
        let (mut fixture, logical) = fixture();
        fixture.definition.heads[0].parameters[0].attribute = AttributeKey("pan".into());
        programmers.set_group(
            group_session,
            "position".into(),
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.2),
        );
        programmers.set(
            direct_session,
            logical,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.8),
        );
        let engine = Engine::new(programmers.clone());
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    id: "position".into(),
                    name: "Position".into(),
                    fixtures: vec![logical],
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            204
        );
        programmers.set_group(
            group_session,
            "position".into(),
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.1),
        );
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            26
        );
    }

    #[test]
    fn programmer_intensity_is_ltp_within_one_programmer_and_htp_between_programmers() {
        let programmers = ProgrammerRegistry::default();
        let first = light_core::SessionId::new();
        let second = light_core::SessionId::new();
        programmers.start(first, light_core::UserId::new());
        programmers.start(second, light_core::UserId::new());
        let (fixture, logical) = fixture();
        programmers.set_group(
            first,
            "wash".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.8),
        );
        programmers.set(
            first,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.3),
        );
        programmers.set(
            second,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.6),
        );
        let engine = Engine::new(programmers.clone());
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    id: "wash".into(),
                    name: "Wash".into(),
                    fixtures: vec![logical],
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();

        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            153,
            "the first programmer resolves its newer 30% fixture value before cross-source HTP chooses the second programmer's 60%",
        );
        assert!(programmers.set_priority(second, 110));
        programmers.set(
            second,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.2),
        );
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            51,
            "numeric priority resolves before HTP magnitude",
        );
        assert!(programmers.set_priority(second, 90));
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            77,
            "changing programmer priority retags its existing values and reveals the higher-priority programmer",
        );
    }

    #[test]
    fn empty_group_programming_becomes_effective_when_members_are_added() {
        let programmers = ProgrammerRegistry::default();
        let (fixture, logical) = fixture();
        let engine = Engine::new(programmers);
        let group = GroupDefinition {
            id: "template".into(),
            name: "Template".into(),
            programming: HashMap::from([(
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.6),
            )]),
            fixtures: vec![],
            ..Default::default()
        };
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture.clone()],
                groups: vec![group.clone()],
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            0
        );
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    fixtures: vec![logical],
                    ..group
                }],
                revision: 2,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            153
        );
    }
    #[test]
    fn session_group_programmer_remains_live_across_membership_changes() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        let frozen_session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        programmers.start(frozen_session, light_core::UserId::new());
        programmers.select_expression(
            session,
            vec![],
            light_programmer::SelectionExpression::LiveGroup {
                group_id: "template".into(),
                rule: light_programmer::SelectionRule::All,
            },
        );
        programmers.select_expression(
            frozen_session,
            vec![],
            light_programmer::SelectionExpression::FrozenGroup {
                group_id: "template".into(),
                source_revision: 0,
            },
        );
        programmers.set_group(
            session,
            "template".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.6),
        );
        let (fixture, logical) = fixture();
        let observed = programmers.clone();
        let engine = Engine::new(programmers);
        let group = GroupDefinition {
            id: "template".into(),
            name: "Template".into(),
            fixtures: vec![],
            ..Default::default()
        };
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture.clone()],
                groups: vec![group.clone()],
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            0
        );
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    fixtures: vec![logical],
                    ..group
                }],
                revision: 2,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            153
        );
        assert_eq!(observed.get(session).unwrap().selected, vec![logical]);
        assert!(observed.get(frozen_session).unwrap().selected.is_empty());
    }
    #[test]
    fn explicit_cue_change_wins_when_group_expansion_targets_same_attribute() {
        let programmers = ProgrammerRegistry::default();
        let (fixture, logical) = fixture();
        let mut cue = light_playback::Cue::new(1.0);
        cue.changes.push(light_playback::CueChange::set(
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        ));
        cue.group_changes.push(light_playback::GroupCueChange {
            group_id: "group".into(),
            attribute: AttributeKey::intensity(),
            value: Some(AttributeValue::Normalized(0.5)),
            fade_millis: None,
            delay_millis: None,
            automatic_restore: false,
        });
        let cue_list = light_playback::CueList {
            id: light_core::CueListId::new(),
            name: "Deduplicated".into(),
            priority: 10,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_step_millis: 1_000,
            chaser_xfade_millis: 0,
            speed_group: None,
            speed_multiplier: 1.0,
            cues: vec![cue],
        };
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![cue_list],
                groups: vec![GroupDefinition {
                    id: "group".into(),
                    name: "Group".into(),
                    fixtures: vec![logical],
                    master: 1.0,
                    playback_fader: None,
                    programming: Default::default(),
                    derived_from: None,
                    frozen_from: None,
                    color: None,
                    icon: None,
                }],
                revision: 1,
                ..Default::default()
            })
            .expect("overlapping group and fixture cue values must compile");
    }

    #[test]
    fn active_group_cue_survives_snapshot_swap_and_gains_new_members() {
        let programmers = ProgrammerRegistry::default();
        let (first, first_logical) = fixture();
        let (mut second, second_logical) = fixture();
        second.address = Some(2);
        let list_id = light_core::CueListId::new();
        let mut cue = light_playback::Cue::new(1.0);
        cue.group_changes.push(light_playback::GroupCueChange {
            group_id: "live".into(),
            attribute: AttributeKey::intensity(),
            value: Some(AttributeValue::Normalized(0.6)),
            fade_millis: None,
            delay_millis: None,
            automatic_restore: false,
        });
        let list = light_playback::CueList {
            id: list_id,
            name: "Live group".into(),
            priority: 10,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_step_millis: 1_000,
            chaser_xfade_millis: 0,
            speed_group: None,
            speed_multiplier: 1.0,
            cues: vec![cue],
        };
        let engine = Engine::new(programmers);
        let snapshot = |members| EngineSnapshot {
            fixtures: vec![first.clone(), second.clone()],
            cue_lists: vec![list.clone()],
            groups: vec![GroupDefinition {
                id: "live".into(),
                name: "Live".into(),
                fixtures: members,
                master: 0.5,
                playback_fader: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        };
        engine
            .replace_snapshot(snapshot(vec![first_logical]))
            .unwrap();
        engine
            .playback()
            .write()
            .go_at(
                list_id,
                chrono::Utc::now() - chrono::Duration::milliseconds(1),
            )
            .unwrap();
        let playback_values = engine
            .playback()
            .write()
            .contributions_at(chrono::Utc::now());
        assert!(
            playback_values
                .iter()
                .any(|value| value.fixture_id == first_logical
                    && value.attribute.is_intensity()
                    && value.value.normalized().is_some_and(|level| level > 0.59))
        );
        let before = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(before.universes[&1][0], 77);
        assert_eq!(before.universes[&1][1], 0);
        engine
            .replace_snapshot(snapshot(vec![first_logical, second_logical]))
            .unwrap();
        assert_eq!(engine.playback().read().active().len(), 1);
        let after = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(after.universes[&1][0], 77);
        assert_eq!(after.universes[&1][1], 77);
        engine
            .replace_snapshot_releasing_playback(snapshot(vec![first_logical, second_logical]))
            .unwrap();
        assert!(engine.playback().read().active().is_empty());
    }

    #[test]
    fn unpatched_group_member_keeps_programming_but_outputs_no_dmx() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        programmers.set_group(
            session,
            "look".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        let (patched, patched_logical) = fixture();
        let (mut unpatched, unpatched_logical) = fixture();
        unpatched.universe = None;
        unpatched.address = None;
        let group = GroupDefinition {
            id: "look".into(),
            name: "Look".into(),
            fixtures: vec![patched_logical, unpatched_logical],
            master: 1.0,
            playback_fader: None,
            ..Default::default()
        };
        let snapshot = |unpatched_fixture: PatchedFixture| EngineSnapshot {
            fixtures: vec![patched.clone(), unpatched_fixture],
            cue_lists: vec![],
            playbacks: vec![],
            playback_pages: vec![],
            routes: vec![],
            control_mappings: vec![],
            groups: vec![group.clone()],
            revision: 1,
        };
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(snapshot(unpatched.clone()))
            .unwrap();
        let resolved = engine.resolved_values();
        assert_eq!(
            resolved
                .get(&(unpatched_logical, AttributeKey::intensity()))
                .and_then(AttributeValue::normalized),
            Some(0.5),
        );
        assert_eq!(group.fixtures, vec![patched_logical, unpatched_logical]);
        let rendered = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(rendered.universes[&1][0], 128);
        assert_eq!(rendered.universes[&1][1], 0);

        unpatched.universe = Some(1);
        unpatched.address = Some(2);
        engine.replace_snapshot(snapshot(unpatched)).unwrap();
        let repatched = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(repatched.universes[&1][0], 128);
        assert_eq!(repatched.universes[&1][1], 128);
    }

    #[test]
    fn hazardous_fixture_defaults_to_immediate_safe_on_control_loss() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (mut fixture, logical) = fixture();
        fixture.definition.hazardous = true;
        fixture
            .definition
            .safe_values
            .insert(AttributeKey::intensity(), AttributeValue::Normalized(0.0));
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![],
                playbacks: vec![],
                playback_pages: vec![],
                routes: vec![],
                control_mappings: vec![],
                groups: vec![],
                revision: 1,
            })
            .unwrap();
        let rendered = engine
            .render(RenderOptions {
                grand_master: 1.0,
                blackout: false,
                control_loss_progress: Some(0.0),
            })
            .unwrap();
        assert_eq!(rendered.universes[&1][0], 0);
    }

    #[test]
    fn move_in_black_waits_for_resolved_darkness_then_prepositions_only_enabled_fixture() {
        let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let clock = Arc::new(ManualClock::new(started));
        let shared: SharedClock = clock.clone();
        let programmers = ProgrammerRegistry::with_clock(shared);
        let (enabled_fixture, enabled) = moving_fixture(1, true, 1_000);
        let (disabled_fixture, disabled) = moving_fixture(10, false, 1_000);
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(mib_snapshot(
                vec![enabled_fixture, disabled_fixture],
                &[enabled, disabled],
            ))
            .unwrap();
        engine.playback().write().go_playback(1).unwrap();
        engine.playback().write().go_playback(1).unwrap();

        clock.set(started + ChronoDuration::milliseconds(1_999));
        let values = engine.resolved_values();
        assert!(normalized(&values, enabled, "intensity") > 0.0);
        assert_eq!(normalized(&values, enabled, "pan"), 0.2);
        let runtime = engine.move_in_black_runtime();
        assert_eq!(
            runtime
                .iter()
                .find(|item| item.fixture_id == enabled)
                .unwrap()
                .state,
            MoveInBlackState::Blocked
        );

        clock.set(started + ChronoDuration::milliseconds(2_000));
        let values = engine.resolved_values();
        assert_eq!(normalized(&values, enabled, "intensity"), 0.0);
        let runtime = engine.move_in_black_runtime();
        let enabled_runtime = runtime
            .iter()
            .find(|item| item.fixture_id == enabled)
            .unwrap();
        assert_eq!(enabled_runtime.state, MoveInBlackState::Delaying);
        assert_eq!(enabled_runtime.dark_since, Some(clock.now()));
        assert_eq!(
            enabled_runtime.delay_deadline,
            Some(started + ChronoDuration::milliseconds(3_000))
        );
        assert_eq!(
            runtime
                .iter()
                .find(|item| item.fixture_id == disabled)
                .unwrap()
                .state,
            MoveInBlackState::Disabled
        );

        clock.set(started + ChronoDuration::milliseconds(2_999));
        assert_eq!(normalized(&engine.resolved_values(), enabled, "pan"), 0.2);
        clock.set(started + ChronoDuration::milliseconds(3_000));
        assert_eq!(normalized(&engine.resolved_values(), enabled, "pan"), 0.2);
        assert_eq!(
            engine
                .move_in_black_runtime()
                .iter()
                .find(|item| item.fixture_id == enabled)
                .unwrap()
                .movement_started_at,
            Some(started + ChronoDuration::milliseconds(3_000))
        );

        clock.set(started + ChronoDuration::milliseconds(4_500));
        let values = engine.resolved_values();
        assert!((normalized(&values, enabled, "pan") - 0.5).abs() < 0.001);
        assert_eq!(normalized(&values, disabled, "pan"), 0.2);

        clock.set(started + ChronoDuration::milliseconds(6_000));
        let values = engine.resolved_values();
        assert!((normalized(&values, enabled, "pan") - 0.8).abs() < 0.001);
        assert_eq!(normalized(&values, disabled, "pan"), 0.2);
        assert_eq!(
            engine
                .move_in_black_runtime()
                .iter()
                .find(|item| item.fixture_id == enabled)
                .unwrap()
                .state,
            MoveInBlackState::Completed
        );

        engine.playback().write().go_playback(1).unwrap();
        let values = engine.resolved_values();
        assert!(
            (normalized(&values, enabled, "pan") - 0.8).abs() < 0.001,
            "the completed hidden move must hand off without jumping back"
        );
        assert_eq!(normalized(&values, disabled, "pan"), 0.2);
    }

    #[test]
    fn move_in_black_is_blocked_and_restarts_its_delay_after_intensity_returns() {
        let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let clock = Arc::new(ManualClock::new(started));
        let shared: SharedClock = clock.clone();
        let programmers = ProgrammerRegistry::with_clock(shared);
        let session = SessionId::new();
        programmers.start(session, UserId::new());
        let (fixture, logical) = moving_fixture(1, true, 1_000);
        let engine = Engine::new(programmers.clone());
        engine
            .replace_snapshot(mib_snapshot(vec![fixture], &[logical]))
            .unwrap();
        engine.playback().write().go_playback(1).unwrap();
        engine.playback().write().go_playback(1).unwrap();
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.2),
        );

        clock.set(started + ChronoDuration::milliseconds(5_000));
        engine.resolved_values();
        let runtime = engine.move_in_black_runtime();
        let runtime = runtime
            .iter()
            .find(|item| item.fixture_id == logical)
            .unwrap();
        assert_eq!(runtime.state, MoveInBlackState::Blocked);
        assert_eq!(runtime.dark_since, None);

        programmers.clear(session);
        engine.resolved_values();
        let runtime = engine.move_in_black_runtime();
        let runtime = runtime
            .iter()
            .find(|item| item.fixture_id == logical)
            .unwrap();
        assert_eq!(runtime.dark_since, Some(clock.now()));
        assert_eq!(
            runtime.delay_deadline,
            Some(started + ChronoDuration::milliseconds(6_000))
        );

        clock.set(started + ChronoDuration::milliseconds(5_500));
        programmers.start(session, UserId::new());
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.2),
        );
        engine.resolved_values();
        assert_eq!(
            engine
                .move_in_black_runtime()
                .iter()
                .find(|item| item.fixture_id == logical)
                .unwrap()
                .state,
            MoveInBlackState::Blocked
        );

        clock.set(started + ChronoDuration::milliseconds(6_000));
        programmers.clear(session);
        engine.resolved_values();
        let runtime = engine.move_in_black_runtime();
        let runtime = runtime
            .iter()
            .find(|item| item.fixture_id == logical)
            .unwrap();
        assert_eq!(runtime.dark_since, Some(clock.now()));
        assert_eq!(
            runtime.delay_deadline,
            Some(started + ChronoDuration::milliseconds(7_000)),
            "returning to dark starts a fresh complete delay"
        );
    }

    #[test]
    fn move_in_black_obeys_same_priority_ltp_and_numeric_priority() {
        let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let clock = Arc::new(ManualClock::new(started));
        let shared: SharedClock = clock.clone();
        let programmers = ProgrammerRegistry::with_clock(shared);
        let (fixture, logical) = moving_fixture(1, true, 0);
        let mut snapshot = mib_snapshot(vec![fixture], &[logical]);

        let mut newer_list = snapshot.cue_lists[0].clone();
        newer_list.id = light_core::CueListId::new();
        newer_list.name = "Newer MIB".into();
        newer_list.cues[2]
            .changes
            .iter_mut()
            .find(|change| change.attribute == AttributeKey("pan".into()))
            .unwrap()
            .value = Some(AttributeValue::Normalized(0.4));
        let mut newer_playback = snapshot.playbacks[0].clone();
        newer_playback.number = 2;
        newer_playback.name = "Newer MIB".into();
        newer_playback.target = PlaybackTarget::CueList {
            cue_list_id: newer_list.id,
        };
        snapshot.cue_lists.push(newer_list);
        snapshot.playbacks.push(newer_playback);

        let engine = Engine::new(programmers);
        engine.replace_snapshot(snapshot.clone()).unwrap();
        for playback in [1, 2] {
            engine.playback().write().go_playback(playback).unwrap();
            engine.playback().write().go_playback(playback).unwrap();
        }

        clock.set(started + ChronoDuration::milliseconds(2_000));
        engine.resolved_values();
        clock.set(started + ChronoDuration::milliseconds(5_000));
        engine.resolved_values();

        snapshot.cue_lists[1].cues[2]
            .changes
            .iter_mut()
            .find(|change| change.attribute == AttributeKey("pan".into()))
            .unwrap()
            .value = Some(AttributeValue::Normalized(0.6));
        snapshot.revision += 1;
        engine.replace_snapshot(snapshot.clone()).unwrap();
        engine.resolved_values();

        clock.set(started + ChronoDuration::milliseconds(6_500));
        let values = engine.resolved_values();
        assert!(
            (normalized(&values, logical, "pan") - 0.5).abs() < 0.001,
            "the recalculated same-priority MIB target is the newer LTP source"
        );

        snapshot.cue_lists[0].priority = 20;
        snapshot.revision += 1;
        engine.replace_snapshot(snapshot).unwrap();
        let values = engine.resolved_values();
        assert!(
            (normalized(&values, logical, "pan") - 0.8).abs() < 0.001,
            "numeric priority overrides a newer lower-priority MIB source"
        );
    }

    #[test]
    fn programmer_fade_starts_from_resolved_playback_underlay_and_release_reveals_it() {
        let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let clock = Arc::new(ManualClock::new(started));
        let shared: SharedClock = clock.clone();
        let programmers = ProgrammerRegistry::with_clock(shared);
        let session = SessionId::new();
        programmers.start(session, UserId::new());
        let (fixture, logical) = fixture();
        let mut snapshot = mib_snapshot(vec![fixture], &[logical]);
        snapshot.cue_lists[0].cues[0]
            .changes
            .iter_mut()
            .find(|change| change.attribute.is_intensity())
            .unwrap()
            .value = Some(AttributeValue::Normalized(0.25));
        let engine = Engine::new(programmers.clone());
        engine.set_control_timing([120.0; 5], 1_000, 0);
        engine.replace_snapshot(snapshot).unwrap();
        engine.playback().write().go_playback(1).unwrap();

        clock.set(started + ChronoDuration::seconds(5));
        assert!((normalized(&engine.resolved_values(), logical, "intensity") - 0.25).abs() < 0.001);
        programmers.set_faded(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.8),
        );
        assert!((normalized(&engine.resolved_values(), logical, "intensity") - 0.25).abs() < 0.001);

        clock.set(started + ChronoDuration::milliseconds(5_500));
        assert!(
            (normalized(&engine.resolved_values(), logical, "intensity") - 0.525).abs() < 0.001,
            "the programmer transition interpolates from the live playback, not zero"
        );
        programmers.clear(session);
        assert!(
            (normalized(&engine.resolved_values(), logical, "intensity") - 0.25).abs() < 0.001,
            "release immediately reveals the unchanged playback underlay"
        );
    }

    #[test]
    fn overlapping_preload_group_fades_keep_edit_order_at_one_commit_timestamp() {
        let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let clock = Arc::new(ManualClock::new(started));
        let shared: SharedClock = clock.clone();
        let programmers = ProgrammerRegistry::with_clock(shared);
        let session = SessionId::new();
        programmers.start(session, UserId::new());
        let (fixture, logical) = fixture();
        let engine = Engine::new(programmers.clone());
        engine.set_control_timing([120.0; 5], 3_000, 0);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![
                    GroupDefinition {
                        id: "1".into(),
                        name: "Broad".into(),
                        fixtures: vec![logical],
                        ..Default::default()
                    },
                    GroupDefinition {
                        id: "2".into(),
                        name: "Subset".into(),
                        fixtures: vec![logical],
                        ..Default::default()
                    },
                ],
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        assert!(programmers.arm_preload(session, true));
        assert!(programmers.set_group_faded(
            session,
            "1".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        ));
        assert!(programmers.set_group_faded_with_timing(
            session,
            "2".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.7),
            Some(1_000),
            None,
        ));
        let committed_at = started + ChronoDuration::seconds(2);
        assert!(programmers.activate_preload_at(session, committed_at));
        let active = programmers.get(session).unwrap();
        assert_eq!(
            active.preload_group_active["1"][&AttributeKey::intensity()].changed_at,
            committed_at
        );
        assert_eq!(
            active.preload_group_active["2"][&AttributeKey::intensity()].changed_at,
            committed_at
        );
        assert!(
            active.preload_group_active["2"][&AttributeKey::intensity()].programmer_order
                > active.preload_group_active["1"][&AttributeKey::intensity()].programmer_order
        );

        for millis in (2_000..=3_000).step_by(25) {
            clock.set(started + ChronoDuration::milliseconds(millis));
            engine.resolved_values();
        }
        assert!(
            (normalized(&engine.resolved_values(), logical, "intensity") - 0.7).abs() < 0.001,
            "rendering one group must not continually restart another group's explicit fade"
        );
    }

    #[test]
    fn programmer_master_fade_interpolates_live_values() {
        let engine = Engine::new(ProgrammerRegistry::default());
        engine.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 1_000, 0);
        let now = chrono::Utc::now();
        let value = TimedValue {
            fixture_id: FixtureId::new(),
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(1.0),
            priority: 100,
            changed_at: now - chrono::Duration::milliseconds(500),
            programmer_order: 0,
            merge_mode: MergeMode::Htp,
            fade: true,
            fade_millis: None,
            delay_millis: None,
        };
        let faded = engine.faded_programmer_value(
            value,
            now,
            None,
            ProgrammerId::new(),
            ProgrammerTransitionSource::Programmer,
        );
        assert!(
            faded
                .value
                .normalized()
                .is_some_and(|level| (level - 0.5).abs() < 0.02)
        );
    }
}
