//! TL-544 G1: Cue and Programmer fades between Angles and Target, and between Targets with a
//! different reference, move through the fade instead of holding the source and snapping.
//!
//! Every case renders the production Live family frame (`render_with_playback_events` on a
//! contract-1 desk with the all-family Live opt-in) and decodes the encoded native Pan/Tilt
//! through an independently compiled `CompiledPositionForward`. Interior samples must lie
//! strictly between the two settled endpoint poses, and the completed fade must equal the
//! destination's own steady output: the destination Target stays live after completion.
use super::*;
use crate::runtime::AppState;
use crate::runtime::tests::test_state_with_family_adapters;
use light_core::CueListId;
use light_engine::{CueListPlaybackAction, EnginePlaybackCommand, RenderResult};
use light_playback::{Cue, CueChange, CueList, CueNumber};

const FADE: u64 = 10_000;

pub(super) struct FadeDesk {
    pub state: AppState,
    pub clock: Arc<ManualClock>,
    pub programmers: ProgrammerRegistry,
    pub session: SessionId,
    pub mover: FixtureId,
    pub point: FixtureId,
    pub list: CueListId,
    data_dir: std::path::PathBuf,
}

impl Drop for FadeDesk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

pub(super) fn position() -> AttributeKey {
    AttributeKey("position".into())
}

/// The fixed-world floor target and a Point (with its own local offset) on the other side.
pub(super) fn origin_target() -> AttributeValue {
    target(TargetReference::Origin, [2.0, 3.0, 0.0])
}

pub(super) fn point_target(point: FixtureId) -> AttributeValue {
    target(
        TargetReference::Point { point_id: point.0 },
        [0.0, 0.0, 0.0],
    )
}

pub(super) fn cue_list(cues: Vec<Cue>) -> CueList {
    CueList {
        pool_number: None,
        legacy_pool_aliases: Vec::new(),
        id: CueListId::new(),
        name: "TL-544 G1".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues,
    }
}

pub(super) fn cue(number: u32, value: AttributeValue, fixture: FixtureId, fade: u64) -> Cue {
    let mut cue = Cue::new(CueNumber::try_from_legacy_f64(f64::from(number)).unwrap());
    cue.fade_millis = fade;
    cue.changes = vec![CueChange::set(fixture, position(), value)];
    cue
}

impl FadeDesk {
    /// A synthetic ±720° U16 mover hung 6 m above the origin, and a Point at (-3, 2, 0) m.
    pub fn new(cues: impl FnOnce(FixtureId, FixtureId) -> Vec<Cue>) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        let (state, data_dir) = test_state_with_family_adapters(
            programmers.clone(),
            Some(clock.clone()),
            PROGRAMMING_CONTRACT_VERSION,
        );
        let mover = FixtureId::new();
        let point_id = FixtureId::new();
        let mut fixture = patched(&moving_head(), mover, 1);
        fixture.location = FixtureLocation {
            x: 0,
            y: 0,
            z: 6000,
        };
        let aim = point(
            point_id,
            FixtureLocation {
                x: -3000,
                y: 2000,
                z: 0,
            },
        );
        let list = cue_list(cues(mover, point_id));
        let list_id = list.id;
        state
            .output
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture, aim].into(),
                cue_lists: vec![list].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        // The Point fixture's own parameters stay at their declared centre.
        for attribute in [
            "point.position.x",
            "point.position.y",
            "point.position.z",
            "point.rotation.x",
            "point.rotation.y",
            "point.rotation.z",
        ] {
            programmers.set(
                session,
                point_id,
                AttributeKey(attribute.into()),
                AttributeValue::Normalized(0.5),
            );
        }
        Self {
            state,
            clock,
            programmers,
            session,
            mover,
            point: point_id,
            list: list_id,
            data_dir,
        }
    }

    pub fn playback(&self, action: CueListPlaybackAction) {
        self.state
            .output
            .engine()
            .execute_playback(EnginePlaybackCommand::CueList {
                id: self.list,
                action,
            })
            .unwrap();
    }

    pub fn jump(&self, cue: u32) {
        self.playback(CueListPlaybackAction::Jump(
            CueNumber::try_from_legacy_f64(f64::from(cue)).unwrap(),
        ));
    }

    /// Render one production Live frame after `millis`.
    pub fn render(&self, millis: u64) -> RenderResult {
        self.clock.advance_millis(millis as i64);
        self.state
            .output
            .render_with_playback_events(
                &self.state.active_show.output_projection(),
                &self.state.playback.render_capability(),
                self.state.output.render_options(),
            )
            .unwrap()
            .rendered
    }

    /// Decoded commanded absolute Pan/Tilt of the mover in one rendered frame.
    pub fn axes_of(&self, rendered: &RenderResult) -> [f64; 2] {
        let native = &rendered
            .physical
            .instances
            .iter()
            .find(|output| output.instance_id == self.mover.0)
            .expect("mover output")
            .native_raw;
        let snapshot = self.state.output.engine().snapshot();
        let fixture = snapshot
            .fixtures
            .iter()
            .find(|f| f.fixture_id == self.mover)
            .unwrap();
        let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
        let forward = CompiledPositionForward::compile(
            profile,
            fixture.definition.mode_id.unwrap(),
            PositionInstallation {
                calibration: fixture.position_calibration.as_ref(),
                invert_pan: fixture.invert_pan,
                invert_tilt: fixture.invert_tilt,
                bracket_degrees: f64::from(fixture.bracket_angle),
            },
        )
        .unwrap()
        .unwrap();
        let mut commands = forward.create_commands();
        forward.decode_commands(native, &mut commands).unwrap();
        [PositionAxisRole::Pan, PositionAxisRole::Tilt].map(|role| {
            commands
                .iter()
                .find(|c| c.role == Some(role))
                .and_then(|c| c.absolute_degrees())
                .unwrap()
        })
    }

    pub fn axes(&self, millis: u64) -> [f64; 2] {
        let rendered = self.render(millis);
        self.axes_of(&rendered)
    }

    /// Settle well past any fade and render a few frames so continuity is accepted.
    pub fn settle(&self) -> [f64; 2] {
        self.axes(600_000);
        self.axes(25);
        self.axes(25)
    }
}

/// `value` lies strictly inside the open interval between `a` and `b` on every axis that moves.
pub(super) fn assert_between(label: &str, a: [f64; 2], value: [f64; 2], b: [f64; 2]) {
    let mut moved = false;
    for axis in 0..2 {
        let (low, high) = (a[axis].min(b[axis]), a[axis].max(b[axis]));
        if high - low < 0.5 {
            assert!(
                (value[axis] - a[axis]).abs() < 0.5,
                "{label}: axis {axis} {value:?} left a stationary axis {a:?} -> {b:?}"
            );
            continue;
        }
        moved = true;
        let margin = 0.1 * (high - low);
        assert!(
            value[axis] > low + margin && value[axis] < high - margin,
            "{label}: axis {axis} {value:?} is not moving between {a:?} and {b:?}"
        );
    }
    assert!(
        moved,
        "{label}: the endpoints {a:?} and {b:?} do not differ"
    );
}

pub(super) fn assert_near(label: &str, a: [f64; 2], b: [f64; 2]) {
    for axis in 0..2 {
        assert!(
            (a[axis] - b[axis]).abs() < 0.05,
            "{label}: axis {axis} {a:?} != {b:?}"
        );
    }
}

/// GO from `first` into `second` (faded) and return (source pose, mid samples, end pose).
fn cue_fade(first: AttributeValue, second: AttributeValue) -> ([f64; 2], [[f64; 2]; 3], [f64; 2]) {
    let desk = FadeDesk::new(|mover, _| {
        vec![
            cue(1, first.clone(), mover, 0),
            cue(2, second.clone(), mover, FADE),
        ]
    });
    desk.jump(1);
    let source = desk.settle();
    desk.playback(CueListPlaybackAction::Go);
    let quarter = desk.axes(FADE / 4);
    let half = desk.axes(FADE / 4);
    let three_quarters = desk.axes(FADE / 4);
    let end = desk.settle();
    (source, [quarter, half, three_quarters], end)
}

fn steady(value: AttributeValue) -> [f64; 2] {
    let desk = FadeDesk::new(|mover, _| vec![cue(1, value.clone(), mover, 0)]);
    desk.jump(1);
    desk.settle()
}

fn assert_monotonic_fade(label: &str, source: [f64; 2], mid: [[f64; 2]; 3], end: [f64; 2]) {
    for sample in mid {
        assert_between(label, source, sample, end);
    }
    for axis in 0..2 {
        let direction = (end[axis] - source[axis]).signum();
        assert!(
            (mid[1][axis] - mid[0][axis]) * direction >= -1e-6
                && (mid[2][axis] - mid[1][axis]) * direction >= -1e-6,
            "{label}: axis {axis} reverses during the fade {mid:?}"
        );
    }
}

#[test]
fn cue_fade_from_angles_to_a_target_moves_through_solved_joints() {
    let (source, mid, end) = cue_fade(angles(-40.0, 30.0), origin_target());
    assert_near("source", source, steady(angles(-40.0, 30.0)));
    assert_near("destination", end, steady(origin_target()));
    assert_monotonic_fade("Angles -> Target", source, mid, end);
}

#[test]
fn cue_fade_from_a_target_to_angles_moves_through_solved_joints() {
    let (source, mid, end) = cue_fade(origin_target(), angles(-40.0, 30.0));
    assert_near("source", source, steady(origin_target()));
    assert_near("destination", end, steady(angles(-40.0, 30.0)));
    assert_monotonic_fade("Target -> Angles", source, mid, end);
}

#[test]
fn cue_fade_between_targets_with_different_references_moves_through_world_points() {
    let desk = FadeDesk::new(|mover, point| {
        vec![
            cue(1, origin_target(), mover, 0),
            cue(2, point_target(point), mover, FADE),
        ]
    });
    desk.jump(1);
    let source = desk.settle();
    desk.playback(CueListPlaybackAction::Go);
    let mid = [
        desk.axes(FADE / 4),
        desk.axes(FADE / 4),
        desk.axes(FADE / 4),
    ];
    let end = desk.settle();
    assert_near("source", source, steady(origin_target()));
    // The beam walks the straight world line between the two floor points: Pan moves
    // monotonically between its endpoints while Tilt follows the line's changing distance.
    let (low, high) = (source[0].min(end[0]), source[0].max(end[0]));
    assert!(
        high - low > 10.0,
        "the two references aim apart: {source:?} {end:?}"
    );
    for (index, sample) in mid.iter().enumerate() {
        assert!(
            sample[0] > low + 0.1 * (high - low) && sample[0] < high - 0.1 * (high - low),
            "sample {index} {sample:?} is not moving between {source:?} and {end:?}"
        );
    }
    let direction = (end[0] - source[0]).signum();
    assert!((mid[1][0] - mid[0][0]) * direction > 0.0 && (mid[2][0] - mid[1][0]) * direction > 0.0);
    // Completion lands on the live Point Target, not a frozen world point.
    let destination = FadeDesk::new(|mover, point| vec![cue(1, point_target(point), mover, 0)]);
    destination.jump(1);
    assert_near("destination", end, destination.settle());
}

#[test]
fn programmer_fade_from_angles_to_a_target_moves_and_lands_on_the_live_target() {
    let desk = FadeDesk::new(|_, _| vec![Cue::new(CueNumber::try_from_legacy_f64(1.0).unwrap())]);
    desk.state
        .output
        .engine()
        .set_control_timing([120.0; 5], FADE, 0, 0);
    desk.programmers
        .set(desk.session, desk.mover, position(), angles(-40.0, 30.0));
    let source = desk.settle();
    desk.programmers
        .set_faded(desk.session, desk.mover, position(), origin_target());
    let mid = [
        desk.axes(FADE / 4),
        desk.axes(FADE / 4),
        desk.axes(FADE / 4),
    ];
    let end = desk.settle();
    assert_near("destination", end, steady(origin_target()));
    assert_monotonic_fade("Programmer Angles -> Target", source, mid, end);
}
