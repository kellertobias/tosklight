//! TL-615 captured Position tracking and fit-cache workloads.
//!
//! Drives the actual path: engine capture, hybrid static/Dynamic preparation with the
//! `PositionFrameObserver`, captured geometry, calibrated fitting and the engine's native
//! finalizer, on contract-enabled *test* Engine/Runtime instances. The production programming
//! contract is not touched. The deterministic smoke runs in normal test runs and asserts only
//! functional behavior. The 300/1,000-instance benchmarks are `#[ignore]` manual release runs
//! that write an immutable report; they assert functional behavior and record timing without
//! any threshold.
//!
//! This is gated software evidence. `ManualClock` advancement is simulated workload time, never
//! an observed output, PSN or presentation rate. PSN delivery, native Stage, operator readouts,
//! production cadence and physical lamps are outside this microbenchmark.
use super::current_cohort::shared_rig;
use super::*;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// `Rig::capture` advances the ManualClock by this much. Simulated, never observed.
const SIMULATED_FRAME_MILLIS: u64 = 25;
/// Fixed identity seed: every fixture, copy, logical head and Point ID derives from it.
const SEED: u64 = 0x7156_1500_0000_0001;
const MAX_CONVERGENCE_FRAMES: usize = 12;
const ROOTS_PER_AIM: usize = 4;
const ROOTS_PER_MOUNT: usize = 4;

fn id(kind: u8, index: usize) -> FixtureId {
    FixtureId(Uuid::from_u128(
        (u128::from(SEED) << 64) | (u128::from(kind) << 32) | index as u128,
    ))
}
const TARGET_ROOT: u8 = 1;
const TARGET_COPY: u8 = 2;
const ANGLE_ROOT: u8 = 3;
const SHARED_ROOT: u8 = 4;
const SHARED_HEAD: u8 = 5;
const AIM_POINT: u8 = 7;
const MOUNT_POINT: u8 = 8;

/// Requested physical instances and how they are realized. Every physical instance is a root
/// or an independently placed multipatch copy; a shared-axis root is one physical instance
/// carrying two logical-head owners.
#[derive(Clone, Copy, Debug)]
struct Layout {
    requested: usize,
    target_roots: usize,
    angle_roots: usize,
    shared_roots: usize,
    aims: usize,
    mounts: usize,
}
impl Layout {
    fn new(requested: usize) -> Self {
        assert!(
            requested >= 10,
            "the layout needs at least 10 physical instances"
        );
        let shared_roots = (requested / 10).max(1);
        let mut angle_roots = (requested / 10).max(1);
        let rest = requested - shared_roots - angle_roots;
        let target_roots = rest / 2;
        angle_roots += rest % 2;
        Self {
            requested,
            target_roots,
            angle_roots,
            shared_roots,
            aims: target_roots.div_ceil(ROOTS_PER_AIM).max(1),
            mounts: target_roots.div_ceil(ROOTS_PER_MOUNT).max(1),
        }
    }
    /// Aim and mount groupings are deliberately orthogonal: roots sharing an aim Point use
    /// different mount Points, so aim and mount motion dirty different root sets.
    fn aim_of(&self, root: usize) -> usize {
        (root / ROOTS_PER_AIM).min(self.aims - 1)
    }
    fn mount_of(&self, root: usize) -> usize {
        root % self.mounts
    }
    fn physical_instances(&self) -> usize {
        2 * self.target_roots + self.angle_roots + self.shared_roots
    }
    fn owners(&self) -> usize {
        self.target_roots + self.angle_roots + 2 * self.shared_roots
    }
    fn json(&self, rig: &Rig) -> Value {
        let snapshot = rig.engine.snapshot();
        let realized_roots = snapshot
            .fixtures
            .iter()
            .filter(|fixture| fixture.address.is_some())
            .count();
        let realized_copies: usize = snapshot
            .fixtures
            .iter()
            .map(|fixture| fixture.multipatch.len())
            .sum();
        let realized_logical: usize = snapshot
            .fixtures
            .iter()
            .map(|fixture| fixture.logical_heads.len())
            .sum();
        let realized_points = snapshot
            .fixtures
            .iter()
            .filter(|fixture| fixture.address.is_none())
            .count();
        json!({
            "requested": {
                "physicalInstances": self.requested,
            },
            "planned": {
                "physicalInstances": self.physical_instances(),
                "targetRootsWithOneCopy": self.target_roots,
                "angleRoots": self.angle_roots,
                "sharedAxisRoots": self.shared_roots,
                "logicalHeadOwners": 2 * self.shared_roots,
                "positionOwners": self.owners(),
                "aimPoints": self.aims,
                "mountPoints": self.mounts,
                "rootsPerAimPoint": ROOTS_PER_AIM,
                "rootsPerMountPoint": ROOTS_PER_MOUNT,
            },
            "realized": {
                "physicalInstances": realized_roots + realized_copies,
                "patchedRoots": realized_roots,
                "physicalCopies": realized_copies,
                "logicalHeads": realized_logical,
                "points": realized_points,
                "fixtures": snapshot.fixtures.len(),
            },
        })
    }
}

/// Moving-head profile with one unrelated native control beside Pan/Tilt, so a native edit
/// can be made in an otherwise fixed Position profile.
fn bench_head() -> FixtureProfile {
    let mut profile = moving_head();
    profile.name = "TL-615 two-axis with an unrelated native control".into();
    let head = profile.modes[0].heads[0].id;
    profile.modes[0].channels.push(channel(head, "frost", 5));
    profile.modes[0].splits[0].footprint = 6;
    profile.validate().unwrap();
    profile
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PointAxis {
    Translate,
    Rotate,
}
#[derive(Clone, Debug)]
enum Motion {
    None,
    Aim(PointAxis, Vec<usize>),
    Mount(PointAxis, Vec<usize>),
    /// Toggle the unrelated native control on these target roots.
    NativeEdit(Vec<usize>),
}
struct Scenario {
    name: &'static str,
    description: &'static str,
    motion: Motion,
}

/// The first Point or root of a kind.
fn one() -> Vec<usize> {
    vec![0]
}
/// Evenly spaced, deterministic ~10% subset (at least one).
fn subset(n: usize) -> Vec<usize> {
    let count = n.div_ceil(10).max(1);
    (0..count).map(|index| index * n / count).collect()
}
fn all(n: usize) -> Vec<usize> {
    (0..n).collect()
}

fn scenarios(layout: &Layout) -> Vec<Scenario> {
    use PointAxis::{Rotate, Translate};
    vec![
        Scenario {
            name: "warm-unchanged",
            description: "accepted fits converged; no Point, Target or native change",
            motion: Motion::None,
        },
        Scenario {
            name: "aim-one-translate",
            description: "one aim Point translates every frame",
            motion: Motion::Aim(Translate, one()),
        },
        Scenario {
            name: "aim-subset-translate",
            description: "~10% of aim Points translate every frame",
            motion: Motion::Aim(Translate, subset(layout.aims)),
        },
        Scenario {
            name: "aim-all-translate",
            description: "every aim Point translates every frame; mounts fixed",
            motion: Motion::Aim(Translate, all(layout.aims)),
        },
        Scenario {
            name: "aim-subset-rotate",
            description: "~10% of aim Points rotate; referenced local offsets move the world Target",
            motion: Motion::Aim(Rotate, subset(layout.aims)),
        },
        Scenario {
            name: "mount-one-translate",
            description: "one mounting Point translates every frame",
            motion: Motion::Mount(Translate, one()),
        },
        Scenario {
            name: "mount-subset-translate",
            description: "~10% of mounting Points translate every frame",
            motion: Motion::Mount(Translate, subset(layout.mounts)),
        },
        Scenario {
            name: "mount-all-translate",
            description: "every mounting Point translates every frame; aims fixed",
            motion: Motion::Mount(Translate, all(layout.mounts)),
        },
        Scenario {
            name: "mount-subset-rotate",
            description: "~10% of mounting Points rotate every frame",
            motion: Motion::Mount(Rotate, subset(layout.mounts)),
        },
        Scenario {
            name: "native-edit-one",
            description: "unrelated native control toggles on one target root; Position inputs fixed",
            motion: Motion::NativeEdit(one()),
        },
        Scenario {
            name: "native-edit-subset",
            description: "unrelated native control toggles on ~10% of target roots; Position inputs fixed",
            motion: Motion::NativeEdit(subset(layout.target_roots)),
        },
    ]
}

struct Owner {
    target: FixtureId,
    /// Stored programmer value at construction; it must never be replaced by fitted output.
    requested: AttributeValue,
    root: FixtureId,
    destinations: Vec<FixtureId>,
}

struct Bench {
    layout: Layout,
    rig: Rig,
    targets: Vec<FixtureId>,
    copies: Vec<FixtureId>,
    shared: Vec<(FixtureId, [FixtureId; 2])>,
    aims: Vec<FixtureId>,
    mounts: Vec<FixtureId>,
    owners: Vec<Owner>,
    /// Current normalized Point/native control values, toggled deterministically.
    values: HashMap<(FixtureId, &'static str), f32>,
    lane: PhysicalAdapterLane<PositionAdapter>,
    runtime: DynamicRuntime,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
    previous_native: HashMap<Uuid, Vec<u32>>,
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameTiming {
    capture: Duration,
    prepare: Duration,
    finalize: Duration,
}
impl FrameTiming {
    fn total(&self) -> Duration {
        self.capture + self.prepare + self.finalize
    }
}
#[derive(Clone, Debug, Default)]
struct FrameObservation {
    timing: FrameTiming,
    fits: u64,
    fit_cache_hits: u64,
    candidate_evaluations: u64,
    compiles: u64,
    changed_points: usize,
    dirty_instances: usize,
    reused_fit_rows: usize,
    geometry_dirty_rows: usize,
    /// Destinations whose complete native output changed against the previous frame.
    native_changed_instances: usize,
    /// Destinations whose Pan/Tilt commands changed against the previous frame.
    position_native_changed_instances: usize,
}

/// Exact expectation derived from the workload model, independently of the adapter.
struct Expectation {
    points: BTreeSet<Uuid>,
    dirty: BTreeSet<(Uuid, Uuid)>,
}

impl Bench {
    fn new(requested: usize) -> Self {
        let layout = Layout::new(requested);
        let profile = bench_head();
        let mut slot = 0usize;
        // Valid, non-overlapping universe addresses: 64 eight-channel slots per universe.
        let mut address = || {
            let universe = 1 + slot / 64;
            let start = 1 + (slot % 64) * 8;
            slot += 1;
            (universe as u16, start as u16)
        };
        let mut fixture_number = 0u32;
        let mut next_number = || {
            fixture_number += 1;
            Some(fixture_number)
        };
        let aims = (0..layout.aims)
            .map(|index| id(AIM_POINT, index))
            .collect::<Vec<_>>();
        let mounts = (0..layout.mounts)
            .map(|index| id(MOUNT_POINT, index))
            .collect::<Vec<_>>();
        let mut fixtures = Vec::new();
        let mut owners = Vec::new();
        let mut targets = Vec::new();
        let mut copies = Vec::new();
        for index in 0..layout.target_roots {
            let root = id(TARGET_ROOT, index);
            let copy = id(TARGET_COPY, index);
            let (universe, start) = address();
            let mut fixture = patched(&profile, root, start);
            fixture.universe = Some(universe);
            fixture.fixture_number = next_number();
            fixture.position_master = Some(mounts[layout.mount_of(index)].0);
            fixture.location = FixtureLocation {
                x: (index % 40) as i32 * 750 - 15_000,
                y: (index / 40) as i32 * 400,
                z: 3000,
            };
            let (copy_universe, copy_start) = address();
            fixture.multipatch = vec![MultiPatchInstance {
                id: copy.0,
                universe: Some(copy_universe),
                address: Some(copy_start),
                location: FixtureLocation {
                    x: fixture.location.x + 600,
                    y: fixture.location.y + 200,
                    z: fixture.location.z + 500,
                },
                ..Default::default()
            }];
            let offset = [[1., 0., 0.], [0., 0., 0.5], [-0.5, 0.5, 0.]][index % 3];
            let requested = target(
                TargetReference::Point {
                    point_id: aims[layout.aim_of(index)].0,
                },
                offset,
            );
            owners.push(Owner {
                target: root,
                requested,
                root,
                destinations: vec![root, copy],
            });
            fixtures.push(fixture);
            targets.push(root);
            copies.push(copy);
        }
        for index in 0..layout.angle_roots {
            let root = id(ANGLE_ROOT, index);
            let (universe, start) = address();
            let mut fixture = patched(&profile, root, start);
            fixture.universe = Some(universe);
            fixture.fixture_number = next_number();
            fixture.location = FixtureLocation {
                x: (index % 40) as i32 * 750 - 15_000,
                y: -2000,
                z: 4000,
            };
            owners.push(Owner {
                target: root,
                requested: angles(
                    30. + (index % 7) as f32 * 15.,
                    20. + (index % 5) as f32 * 10.,
                ),
                root,
                destinations: vec![root],
            });
            fixtures.push(fixture);
        }
        // Shared-axis logical peers: the existing shared-Pan/independent-Tilt profile and its
        // exactly encoded Origin Targets, re-identified per root with fixed IDs.
        let template = shared_rig();
        let shared_fixture = template.rig.engine.snapshot().fixtures[0].clone();
        let mut shared = Vec::new();
        for index in 0..layout.shared_roots {
            let root = id(SHARED_ROOT, index);
            let heads = [id(SHARED_HEAD, 2 * index), id(SHARED_HEAD, 2 * index + 1)];
            let (universe, start) = address();
            let mut fixture = shared_fixture.clone();
            fixture.fixture_id = root;
            fixture.fixture_number = next_number();
            fixture.universe = Some(universe);
            fixture.address = Some(start);
            for (head, id) in fixture.logical_heads.iter_mut().zip(heads) {
                head.fixture_id = id;
            }
            for (head, requested) in heads.iter().zip(&template.targets) {
                owners.push(Owner {
                    target: *head,
                    requested: requested.clone(),
                    root,
                    destinations: vec![root],
                });
            }
            fixtures.push(fixture);
            shared.push((root, heads));
        }
        for (index, aim) in aims.iter().enumerate() {
            fixtures.push(point(
                *aim,
                FixtureLocation {
                    x: index as i32 * 1000 - 15_000,
                    y: 15_000,
                    z: 0,
                },
            ));
        }
        for (index, mount) in mounts.iter().enumerate() {
            fixtures.push(point(
                *mount,
                FixtureLocation {
                    x: index as i32 * 200,
                    y: 0,
                    z: 2000,
                },
            ));
        }
        let rig = Rig::new(fixtures, targets[0]);
        let mut values = HashMap::new();
        for aim in &aims {
            // Rotated aim Points: every referenced local offset is rotated into the world.
            rig.set(*aim, "point.rotation.z", 0.6);
            values.insert((*aim, "point.rotation.z"), 0.6);
        }
        for owner in &owners {
            rig.programmers.set(
                rig.session,
                owner.target,
                ProgrammingOwner::Position.key(),
                owner.requested.clone(),
            );
        }
        Self {
            layout,
            rig,
            targets,
            copies,
            shared,
            aims,
            mounts,
            owners,
            values,
            lane: PhysicalAdapterLane::live(PositionAdapter::default()),
            runtime: DynamicRuntime::with_programming_contract_support(
                PROGRAMMING_CONTRACT_VERSION,
            ),
            origins: Default::default(),
            scratch: Default::default(),
            previous_native: HashMap::new(),
        }
    }

    fn accepted(&self) -> Arc<super::super::tracking::TrackingSnapshot> {
        self.lane.adapter().tracking.borrow().snapshot().unwrap()
    }

    /// Toggle between two values that both differ from the initial value.
    fn toggle(&mut self, fixture: FixtureId, attribute: &'static str, a: f32, b: f32) {
        let next = match self.values.get(&(fixture, attribute)) {
            Some(current) if *current == a => b,
            _ => a,
        };
        self.values.insert((fixture, attribute), next);
        self.rig.set(fixture, attribute, next);
    }

    /// Apply one frame of motion and return the exact expected dependency change, when the
    /// workload model defines one.
    fn apply(&mut self, motion: &Motion) -> Option<Expectation> {
        let mut points = BTreeSet::new();
        let mut roots = Vec::new();
        match motion {
            Motion::None => {}
            Motion::Aim(axis, indices) | Motion::Mount(axis, indices) => {
                let aim = matches!(motion, Motion::Aim(..));
                for &index in indices {
                    let point = if aim {
                        self.aims[index]
                    } else {
                        self.mounts[index]
                    };
                    match (axis, aim) {
                        (PointAxis::Translate, true) => {
                            self.toggle(point, "point.position.x", 0.51, 0.52)
                        }
                        (PointAxis::Translate, false) => {
                            self.toggle(point, "point.position.z", 0.505, 0.51)
                        }
                        (PointAxis::Rotate, true) => {
                            self.toggle(point, "point.rotation.z", 0.62, 0.64)
                        }
                        (PointAxis::Rotate, false) => {
                            self.toggle(point, "point.rotation.z", 0.55, 0.56)
                        }
                    }
                    points.insert(point.0);
                    roots.extend((0..self.layout.target_roots).filter(|&root| {
                        if aim {
                            self.layout.aim_of(root) == index
                        } else {
                            self.layout.mount_of(root) == index
                        }
                    }));
                }
            }
            Motion::NativeEdit(indices) => {
                for &index in indices {
                    let root = self.targets[index];
                    self.toggle(root, "frost", 0.25, 0.75);
                }
                // Conservative full-baseline memo behavior is observed, not prescribed.
                return None;
            }
        }
        let dirty = roots
            .into_iter()
            .flat_map(|index| {
                let root = self.targets[index].0;
                [(root, root), (root, self.copies[index].0)]
            })
            .collect();
        Some(Expectation { points, dirty })
    }

    /// One actual Live frame: capture, hybrid preparation with the Position observer, and the
    /// engine finalizer. Mirrors `prepare_live_engine_inner` with phase timestamps added.
    fn frame(
        &mut self,
    ) -> Result<(PublishedPhysicalFrame<PositionAdapter>, FrameTiming), DynamicRuntimeError> {
        let started = Instant::now();
        let capture = self.rig.capture();
        let captured = Instant::now();
        let engine = &self.rig.engine;
        let snapshot = capture.snapshot();
        let addresser = capture.frame_addresser();
        let now_millis = capture.sampled_at().timestamp_millis() as u64;
        let speeds = [DynamicSpeedTransport {
            effective_bpm: 120.,
            phase_origin_millis: 0,
            phase_reference_millis: now_millis,
            beat_phase: (now_millis as f64 / 500.).rem_euclid(1.),
            phase_advancing: true,
        }; 5];
        let inputs = CapturedDynamicInputs {
            now: capture.sampled_at(),
            speed_transports: &speeds,
            rate: 40,
            snapshot: &snapshot,
            programmer_values: capture.dynamic_programmer_values(),
            programmer_rows: Some(capture.dynamic_programmer_rows()),
            cue_values: capture.cue_dynamic_values(),
            dynamic_playbacks: capture.dynamic_playbacks(),
            playback_paused: capture.playback_dynamics_paused(),
            addresser: &addresser,
            extra_programmer_values: &[],
            programmer_reconciliation_cache: None,
            force_source_reconciliation: false,
        };
        let lane = &self.lane;
        let scratch = &mut self.scratch;
        let mut candidate = self.origins.clone();
        let mut prepared_at = None;
        let output = self.runtime.with_output_frame_transaction(
            &mut DynamicOutputFrameScratch::default(),
            |runtime| {
                let prepared = prepare_captured_hybrid_frame_with_observer(
                    engine,
                    &capture,
                    &[],
                    runtime,
                    &mut candidate,
                    &inputs,
                    scratch,
                    lane,
                    None,
                    &mut PositionFrameObserver::new(lane),
                )?;
                prepared_at = Some(Instant::now());
                finalize_live_physical_frame(engine, &capture, lane, prepared)
            },
        );
        let finished = Instant::now();
        if output.is_ok() {
            self.origins = candidate;
        }
        let prepared_at = prepared_at.unwrap_or(finished);
        Ok((
            output?,
            FrameTiming {
                capture: captured - started,
                prepare: prepared_at - captured,
                finalize: finished - prepared_at,
            },
        ))
    }

    /// Run and check one frame. Functional failures are collected, never mixed with timing.
    fn step(
        &mut self,
        expectation: Option<&Expectation>,
        failures: &mut Vec<String>,
    ) -> FrameObservation {
        let before = self.lane.adapter().counters();
        let (output, timing) = match self.frame() {
            Ok(output) => output,
            Err(error) => {
                failures.push(format!("frame rejected: {error}"));
                return FrameObservation::default();
            }
        };
        let after = self.lane.adapter().counters();
        let mut observation = FrameObservation {
            timing,
            fits: after.fits - before.fits,
            fit_cache_hits: after.fit_cache_hits - before.fit_cache_hits,
            candidate_evaluations: after.candidate_evaluations - before.candidate_evaluations,
            compiles: after.compiles - before.compiles,
            ..Default::default()
        };
        let accepted = self.accepted();
        observation.changed_points = accepted.changed_points().len();
        observation.dirty_instances = accepted.dirty_instances().len();
        let mut fail = |message: String| {
            if failures.len() < 64 {
                failures.push(message);
            }
        };
        if accepted.token() != &output.token {
            fail("accepted tracking token differs from the finalized frame".into());
        }
        if !output.requirements.is_empty() {
            fail(format!(
                "{} unexpected requirements",
                output.requirements.len()
            ));
        }
        if !self.runtime.snapshot().instances.is_empty() {
            fail("static geometry updates invented a Dynamic instance".into());
        }
        if output.results.len() != self.owners.len() {
            fail(format!(
                "{} Position rows for {} owners",
                output.results.len(),
                self.owners.len()
            ));
        }
        let native = output
            .rendered
            .physical
            .instances
            .iter()
            .map(|instance| (instance.instance_id, instance))
            .collect::<HashMap<_, _>>();
        let rows = output
            .results
            .iter()
            .map(|row| (row.target, row))
            .collect::<HashMap<_, _>>();
        let total = self.layout.physical_instances() as u64;
        if observation.fits + observation.fit_cache_hits != total {
            fail(format!(
                "fits {} + cache hits {} != {total} physical instances",
                observation.fits, observation.fit_cache_hits
            ));
        }
        if let Some(expected) = expectation {
            let changed = accepted
                .changed_points()
                .iter()
                .map(|point| point.0)
                .collect::<BTreeSet<_>>();
            if changed != expected.points {
                fail(format!(
                    "changed Points {} != expected {}",
                    changed.len(),
                    expected.points.len()
                ));
            }
            let dirty = accepted
                .dirty_instances()
                .iter()
                .map(|row| (row.root.0, row.destination.0))
                .collect::<BTreeSet<_>>();
            if dirty != expected.dirty {
                fail(format!(
                    "dirty instances {} != expected {}",
                    dirty.len(),
                    expected.dirty.len()
                ));
            }
            if observation.fits != expected.dirty.len() as u64 {
                fail(format!(
                    "fits {} != {} dependent instances",
                    observation.fits,
                    expected.dirty.len()
                ));
            }
        }
        for owner in &self.owners {
            let Some(row) = rows.get(&owner.target) else {
                fail(format!("owner {} has no row", owner.target.0));
                continue;
            };
            if row.value != owner.requested {
                fail(format!("owner {} stored value replaced", owner.target.0));
            }
            if row.requested != *intent(&owner.requested).unwrap() {
                fail(format!("owner {} original Intent replaced", owner.target.0));
            }
            if row.quality.held {
                fail(format!("owner {} held", owner.target.0));
            }
            if row.quality.reused_fits > 0 {
                observation.reused_fit_rows += 1;
            }
            if row.quality.geometry_dirty {
                observation.geometry_dirty_rows += 1;
            }
            let dependent = expectation.map(|expected| {
                owner
                    .destinations
                    .iter()
                    .any(|destination| expected.dirty.contains(&(owner.root.0, destination.0)))
            });
            match dependent {
                Some(true) if row.quality.reused_fits != 0 || !row.quality.geometry_dirty => {
                    fail(format!("dependent owner {} reused a fit", owner.target.0))
                }
                Some(false)
                    if row.quality.reused_fits != owner.destinations.len()
                        || row.quality.geometry_dirty =>
                {
                    fail(format!(
                        "unrelated owner {} did not reuse its accepted fit",
                        owner.target.0
                    ))
                }
                _ => {}
            }
            for outcome in &row.achieved.outcomes {
                if outcome.result.status != PositionFitStatus::Fitted {
                    fail(format!(
                        "owner {} status {:?}",
                        owner.target.0, outcome.result.status
                    ));
                }
                if matches!(
                    outcome.result.requested,
                    Some(PositionFitRequest::Target { .. })
                ) && !outcome
                    .result
                    .angular_error_degrees
                    .is_some_and(|error| error < 0.05)
                {
                    fail(format!("owner {} Target error too large", owner.target.0));
                }
                let Some(instance) = native.get(&outcome.destination.0) else {
                    fail(format!(
                        "destination {} not rendered",
                        outcome.destination.0
                    ));
                    continue;
                };
                if !instance.complete {
                    fail(format!(
                        "destination {} incomplete native",
                        outcome.destination.0
                    ));
                }
                for write in row
                    .writes
                    .iter()
                    .filter(|write| write.slot.destination == outcome.destination)
                {
                    if instance.native_raw[write.slot.channel_index as usize] != write.raw {
                        fail(format!(
                            "destination {} finalizer did not encode the fitted command",
                            outcome.destination.0
                        ));
                    }
                }
            }
        }
        // Frame-to-frame native comparison. Pan/Tilt are channel indices 0 and 1.
        let mut current = HashMap::with_capacity(self.previous_native.len());
        for owner in &self.owners {
            for destination in &owner.destinations {
                let Some(instance) = native.get(&destination.0) else {
                    continue;
                };
                if current.contains_key(&destination.0) {
                    continue;
                }
                let raw = instance.native_raw.to_vec();
                if let Some(previous) = self.previous_native.get(&destination.0) {
                    if previous != &raw {
                        observation.native_changed_instances += 1;
                    }
                    if previous[..2] != raw[..2] {
                        observation.position_native_changed_instances += 1;
                        let unrelated = expectation.is_some_and(|expected| {
                            !owner.destinations.iter().any(|destination| {
                                expected.dirty.contains(&(owner.root.0, destination.0))
                            })
                        });
                        if unrelated {
                            fail(format!(
                                "unrelated destination {} changed Pan/Tilt",
                                destination.0
                            ));
                        }
                    }
                }
                current.insert(destination.0, raw);
            }
        }
        self.previous_native = current;
        observation
    }

    /// Run until a frame fits nothing and every physical instance is a cache hit.
    fn converge(&mut self, failures: &mut Vec<String>) -> Option<usize> {
        for frame in 1..=MAX_CONVERGENCE_FRAMES {
            let observation = self.step(None, failures);
            if observation.fits == 0
                && observation.fit_cache_hits == self.layout.physical_instances() as u64
            {
                return Some(frame);
            }
        }
        failures.push(format!(
            "no accepted fit convergence within {MAX_CONVERGENCE_FRAMES} unchanged frames"
        ));
        None
    }

    /// Every shared-axis peer pair carries the identical accepted memo Arc.
    fn shared_peers_share_memo(&self) -> bool {
        self.shared.iter().all(|(_, heads)| {
            let memo = |head: FixtureId| {
                self.lane
                    .continuity(head, ProgrammingOwner::Position)
                    .and_then(|continuity| continuity.instances[0].fit_memo.clone())
            };
            matches!((memo(heads[0]), memo(heads[1])), (Some(a), Some(b)) if Arc::ptr_eq(&a, &b))
        })
    }
}

struct ScenarioRun {
    name: &'static str,
    description: &'static str,
    moved: Value,
    convergence_frames: Option<usize>,
    warmup: Vec<FrameObservation>,
    samples: Vec<FrameObservation>,
    failures: Vec<String>,
    shared_memo_identity: bool,
}

fn run_scenario(
    bench: &mut Bench,
    scenario: &Scenario,
    warmup: usize,
    samples: usize,
) -> ScenarioRun {
    let mut failures = Vec::new();
    let convergence_frames = bench.converge(&mut failures);
    let shared_memo_identity = bench.shared_peers_share_memo();
    if !shared_memo_identity {
        failures.push("shared-axis peers do not carry one accepted memo".into());
    }
    let mut run = |count: usize, failures: &mut Vec<String>| {
        (0..count)
            .map(|_| {
                let expectation = bench.apply(&scenario.motion);
                bench.step(expectation.as_ref(), failures)
            })
            .collect::<Vec<_>>()
    };
    let warmup = run(warmup, &mut failures);
    let samples = run(samples, &mut failures);
    let moved = match &scenario.motion {
        Motion::None => json!({"kind": "none"}),
        Motion::Aim(axis, indices) => json!({
            "kind": "aim-points", "axis": format!("{axis:?}"), "count": indices.len(),
            "dependentInstancesPerFrame": 2 * (0..bench.layout.target_roots)
                .filter(|&root| indices.contains(&bench.layout.aim_of(root))).count(),
        }),
        Motion::Mount(axis, indices) => json!({
            "kind": "mount-points", "axis": format!("{axis:?}"), "count": indices.len(),
            "dependentInstancesPerFrame": 2 * (0..bench.layout.target_roots)
                .filter(|&root| indices.contains(&bench.layout.mount_of(root))).count(),
        }),
        Motion::NativeEdit(indices) => json!({
            "kind": "unrelated-native-control", "editedTargetRoots": indices.len(),
            "editedPhysicalInstances": 2 * indices.len(),
        }),
    };
    ScenarioRun {
        name: scenario.name,
        description: scenario.description,
        moved,
        convergence_frames,
        warmup,
        samples,
        failures,
        shared_memo_identity,
    }
}

fn run_all(
    requested: usize,
    warmup: usize,
    samples: usize,
) -> (Bench, Vec<ScenarioRun>, Vec<PendingRun>) {
    let mut bench = Bench::new(requested);
    let runs = scenarios(&bench.layout)
        .iter()
        .map(|scenario| run_scenario(&mut bench, scenario, warmup, samples))
        .collect();
    let pending = run_pending(&mut bench, warmup, samples);
    (bench, runs, pending)
}

// ---------------------------------------------------------------------------------------------
// Pending paired preload: RetainedPreloadHybridEvaluator + PositionPreloadObserver over
// PhysicalPreloadLanes, consuming one retained input per window (mirrors the TL-556 test).

use crate::runtime::dynamic_snapshot_publication::{
    DynamicSnapshotPublication, RetainedFrameCapture,
};
use crate::runtime::output_scheduler::dynamic_projection::retained_preload_hybrid::{
    PendingHybridResult, RetainedPreloadHybridEvaluator,
};
use crate::runtime::preload::retained_history::paired::{
    PairedPendingHistory, PendingPairEvaluator, PendingPairWindowOutcome,
};
use crate::runtime::preload::retained_history::{
    PendingEpisodeKey, PendingHistoryLimits, PendingHistoryPosition, PendingHistorySeed,
};

type BenchPair = PairedPendingHistory<PendingHybridResult<PhysicalHeadResult<PositionAdapter>>>;

fn consume_window(
    pair: &mut BenchPair,
    input: Arc<crate::runtime::dynamic_snapshot_publication::RetainedInputCapture>,
    live: &DynamicRuntime,
    evaluator: &mut impl PendingPairEvaluator<PendingHybridResult<PhysicalHeadResult<PositionAdapter>>>,
) -> PendingPairWindowOutcome {
    let (before, after) = pair.positions();
    let before_controls = live.controls_since(before.controls).unwrap().unwrap();
    let after_controls = live.controls_since(after.controls).unwrap().unwrap();
    let capacity = |n| std::num::NonZeroUsize::new(n).unwrap();
    let prepared = pair
        .prepare_window(
            &[input],
            &[],
            &before_controls,
            &[],
            &after_controls,
            PendingHistoryLimits {
                attempts: capacity(8),
                cold_changes: capacity(8),
                controls: capacity(64),
            },
        )
        .unwrap();
    pair.consume_window(prepared, evaluator)
}

#[derive(Clone, Copy, Debug, Default)]
struct PendingObservation {
    /// Engine capture plus retained-input publication.
    retain: Duration,
    /// Window preparation and paired Before/After evaluation, fitting and finalization.
    consume: Duration,
    fits: u64,
    fit_cache_hits: u64,
    candidate_evaluations: u64,
    dirty_instances: [usize; 2],
}
struct PendingRun {
    name: &'static str,
    moved_aims: usize,
    convergence_windows: Option<usize>,
    warmup: Vec<PendingObservation>,
    samples: Vec<PendingObservation>,
    failures: Vec<String>,
}

/// Runs after every Live scenario: arming preload is the last mutation of this Bench.
fn run_pending(bench: &mut Bench, warmup: usize, samples: usize) -> Vec<PendingRun> {
    let live_tracking = bench.accepted();
    let total = bench.layout.physical_instances() as u64;
    let subset_aims = subset(bench.layout.aims);
    let expected_points = subset_aims
        .iter()
        .map(|&index| bench.aims[index].0)
        .collect::<BTreeSet<_>>();
    let expected_dirty = (0..bench.layout.target_roots)
        .filter(|&root| subset_aims.contains(&bench.layout.aim_of(root)))
        .flat_map(|index| {
            let root = bench.targets[index].0;
            [(root, root), (root, bench.copies[index].0)]
        })
        .collect::<BTreeSet<_>>();
    let mut values = std::mem::take(&mut bench.values);
    let aims = bench.aims.clone();
    let rig = &bench.rig;
    assert!(rig.programmers.arm_preload(rig.session, true));
    let snapshot = rig.engine.snapshot();
    let publication = DynamicSnapshotPublication::new(snapshot.clone());
    let mut live = DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    let (cold, controls) = publication
        .begin_retained_history(
            &mut live,
            &snapshot,
            std::num::NonZeroUsize::new(64).unwrap(),
        )
        .unwrap();
    let programmer = rig
        .engine
        .prepare_output_frame(Default::default())
        .programmer()
        .identity
        .unwrap();
    let activation = Uuid::from_u128(u128::from(SEED) << 64 | 0x7156_ffff);
    let branches = [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease];
    let seed = |branch| PendingHistorySeed {
        key: PendingEpisodeKey {
            activation,
            programmer,
            branch,
        },
        runtime: live.fork_for_pending_preview(),
        origins: Default::default(),
        snapshot: snapshot.clone(),
        position: PendingHistoryPosition {
            inputs: publication.input_capture_cursor().unwrap(),
            cold,
            controls,
        },
        live_sample: live.committed_sample_boundary(),
    };
    let mut pair: BenchPair =
        PairedPendingHistory::new(seed(branches[0]), seed(branches[1])).unwrap();
    // Retained-capture selection instants are simulated, like the ManualClock.
    let started = Instant::now();
    let mut selected = 0u64;
    let mut retained = || {
        let cursor = publication.input_capture_cursor().unwrap();
        let captured = RetainedFrameCapture::select(
            rig.capture(),
            &publication,
            started + Duration::from_millis(selected * 40),
        );
        selected += 1;
        publication.retain_accepted_input(
            &live,
            captured.retained().unwrap(),
            &[],
            &[DynamicSpeedTransport {
                effective_bpm: 120.,
                phase_origin_millis: 0,
                phase_reference_millis: 0,
                beat_phase: 0.,
                phase_advancing: true,
            }; 5],
            40,
            None,
        );
        publication.input_captures_since(cursor).unwrap().remove(0)
    };
    let lanes = PhysicalPreloadLanes::new(PositionAdapter::default(), PositionAdapter::default());
    let mut evaluator = RetainedPreloadHybridEvaluator::new_with_observer(
        &rig.engine,
        programmer,
        &lanes,
        PositionPreloadObserver::new(&lanes),
    );
    let counters = || branches.map(|branch| lanes.lane(branch).adapter().counters());
    // `None` settles without motion or cleanliness checks; `Some(moving)` is a measured window.
    let mut window = |phase: Option<bool>,
                      values: &mut HashMap<(FixtureId, &'static str), f32>,
                      failures: &mut Vec<String>| {
        let moving = phase == Some(true);
        if moving {
            for &index in &subset_aims {
                let key = (aims[index], "point.position.x");
                let next = if values.get(&key) == Some(&0.51) {
                    0.52
                } else {
                    0.51
                };
                values.insert(key, next);
                rig.set(aims[index], "point.position.x", next);
            }
        }
        let before = counters();
        let t0 = Instant::now();
        let input = retained();
        let t1 = Instant::now();
        let outcome = consume_window(&mut pair, input, &live, &mut evaluator);
        let t2 = Instant::now();
        let after = counters();
        let delta = |f: fn(&PositionAdapterCounters) -> u64| {
            (0..2).map(|i| f(&after[i]) - f(&before[i])).sum::<u64>()
        };
        let mut observation = PendingObservation {
            retain: t1 - t0,
            consume: t2 - t1,
            fits: delta(|c| c.fits),
            fit_cache_hits: delta(|c| c.fit_cache_hits),
            candidate_evaluations: delta(|c| c.candidate_evaluations),
            dirty_instances: [0; 2],
        };
        if outcome.successful_attempts != 1
            || !outcome.failed_attempts.is_empty()
            || outcome.stopped.is_some()
        {
            failures.push(format!(
                "paired window: {} successful, {} failed",
                outcome.successful_attempts,
                outcome.failed_attempts.len()
            ));
            return observation;
        }
        if !live.snapshot().instances.is_empty() {
            failures.push("static Pending window invented a Dynamic instance".into());
        }
        if observation.fits + observation.fit_cache_hits != 2 * total {
            failures.push(format!(
                "paired fits {} + hits {} != 2 x {total}",
                observation.fits, observation.fit_cache_hits
            ));
        }
        for (index, branch) in branches.into_iter().enumerate() {
            let Some(accepted) = lanes.lane(branch).adapter().tracking.borrow().snapshot() else {
                failures.push(format!("{branch:?} has no accepted tracking"));
                continue;
            };
            if Arc::ptr_eq(&accepted, &live_tracking) {
                failures.push("a Pending branch borrowed Live tracking".into());
            }
            observation.dirty_instances[index] = accepted.dirty_instances().len();
            if moving {
                let points = accepted
                    .changed_points()
                    .iter()
                    .map(|point| point.0)
                    .collect::<BTreeSet<_>>();
                let dirty = accepted
                    .dirty_instances()
                    .iter()
                    .map(|row| (row.root.0, row.destination.0))
                    .collect::<BTreeSet<_>>();
                if points != expected_points || dirty != expected_dirty {
                    failures.push(format!(
                        "{branch:?}: changed {} / dirty {} != expected {} / {}",
                        points.len(),
                        dirty.len(),
                        expected_points.len(),
                        expected_dirty.len()
                    ));
                }
            }
        }
        if phase.is_some() {
            let expected_fits = if moving {
                2 * expected_dirty.len() as u64
            } else {
                0
            };
            let expected_hits = 2 * total - expected_fits;
            if (observation.fits, observation.fit_cache_hits) != (expected_fits, expected_hits) {
                failures.push(format!(
                    "paired fits {} / hits {} != expected {expected_fits} / {expected_hits}",
                    observation.fits, observation.fit_cache_hits
                ));
            }
        }
        if phase == Some(false) && observation.dirty_instances != [0, 0] {
            failures.push("unchanged paired window dirtied instances".into());
        }
        observation
    };
    let mut runs = Vec::new();
    for (name, moving) in [
        ("pending-pair-warm-unchanged", false),
        ("pending-pair-aim-subset-translate", true),
    ] {
        let mut failures = Vec::new();
        // Accepted numerical seeds may need another iteration. Converge both independent
        // branches before measuring unchanged reuse or dependent-only invalidation.
        let mut convergence_windows = None;
        for count in 1..=MAX_CONVERGENCE_FRAMES {
            let observation = window(None, &mut values, &mut failures);
            if observation.dirty_instances == [0, 0]
                && observation.fits == 0
                && observation.fit_cache_hits == 2 * total
            {
                convergence_windows = Some(count);
                break;
            }
        }
        if convergence_windows.is_none() {
            failures.push("paired lanes did not settle".into());
        }
        let warmup = (0..warmup)
            .map(|_| window(Some(moving), &mut values, &mut failures))
            .collect();
        let samples = (0..samples)
            .map(|_| window(Some(moving), &mut values, &mut failures))
            .collect();
        runs.push(PendingRun {
            name,
            moved_aims: if moving { subset_aims.len() } else { 0 },
            convergence_windows,
            warmup,
            samples,
            failures,
        });
    }
    if !Arc::ptr_eq(&bench.accepted(), &live_tracking) {
        runs[0]
            .failures
            .push("Pending evaluation replaced Live tracking".into());
    }
    drop(evaluator);
    bench.values = values;
    runs
}

fn pending_json(run: &PendingRun) -> Value {
    let series =
        |f: &dyn Fn(&PendingObservation) -> f64| run.samples.iter().map(f).collect::<Vec<_>>();
    let wall = "std::time::Instant (monotonic) around test-side phase boundaries";
    let counter = "sum of Before and After PositionAdapter::counters deltas per window";
    json!({
        "name": run.name,
        "motion": {"kind": if run.moved_aims > 0 { "aim-points" } else { "none" }, "count": run.moved_aims},
        "convergenceWindowsBeforeScenario": run.convergence_windows
            .map_or_else(|| unavailable("did not converge"), |windows| json!(windows)),
        "warmupWindows": run.warmup.len(),
        "sampleWindows": run.samples.len(),
        "timing": {
            "endToEnd": distribution(&series(&|w| micros(w.retain + w.consume)), "us", wall),
            "phases": {
                "captureAndRetain": distribution(&series(&|w| micros(w.retain)), "us",
                    "Engine::prepare_output_frame + RetainedFrameCapture + retain_accepted_input"),
                "pairedEvaluation": distribution(&series(&|w| micros(w.consume)), "us",
                    "prepare_window + consume_window: Before and After preparation, fitting, paired finalization"),
                "perBranch": unavailable("the paired evaluator exposes no per-branch timing without production instrumentation"),
            },
        },
        "counters": {
            "fits": distribution(&series(&|w| w.fits as f64), "count", counter),
            "fitCacheHits": distribution(&series(&|w| w.fit_cache_hits as f64), "count", counter),
            "candidateEvaluations": distribution(&series(&|w| w.candidate_evaluations as f64), "count", counter),
            "dirtyInstancesBefore": distribution(&series(&|w| w.dirty_instances[0] as f64), "count", "Before lane accepted TrackingSnapshot"),
            "dirtyInstancesAfter": distribution(&series(&|w| w.dirty_instances[1] as f64), "count", "After lane accepted TrackingSnapshot"),
        },
        "functional": {"passed": run.failures.is_empty(), "failures": run.failures},
    })
}

// ---------------------------------------------------------------------------------------------
// Deterministic smoke (normal test runs; functional assertions only, no timing).

#[test]
fn captured_position_tracking_smoke_reuses_accepted_fits_and_refits_only_dependents() {
    let (bench, runs, pending) = run_all(20, 1, 2);
    let layout = bench.layout;
    assert_eq!(layout.physical_instances(), 20);
    assert_eq!(
        (layout.target_roots, layout.angle_roots, layout.shared_roots),
        (8, 2, 2)
    );
    assert_eq!((layout.aims, layout.mounts), (2, 2));
    let snapshot = bench.rig.engine.snapshot();
    let numbers = snapshot
        .fixtures
        .iter()
        .filter_map(|fixture| fixture.fixture_number)
        .collect::<BTreeSet<_>>();
    assert_eq!(numbers.len(), 12, "unique fixture numbers per patched root");
    for run in &runs {
        assert!(
            run.failures.is_empty(),
            "{}: functional failures {:?}",
            run.name,
            run.failures
        );
        assert!(run.convergence_frames.is_some(), "{}", run.name);
        assert!(run.shared_memo_identity, "{}", run.name);
    }
    let by_name = |name: &str| runs.iter().find(|run| run.name == name).unwrap();
    for frame in &by_name("warm-unchanged").samples {
        assert_eq!((frame.fits, frame.fit_cache_hits), (0, 20));
        assert_eq!((frame.changed_points, frame.dirty_instances), (0, 0));
        assert_eq!(frame.reused_fit_rows, layout.owners());
        assert_eq!(frame.native_changed_instances, 0);
    }
    // One aim Point: its four roots and their copies refit; everything else reuses.
    for name in ["aim-one-translate", "aim-subset-translate"] {
        for frame in &by_name(name).samples {
            assert_eq!((frame.fits, frame.fit_cache_hits), (8, 12), "{name}");
            assert_eq!((frame.changed_points, frame.dirty_instances), (1, 8));
            assert_eq!(frame.position_native_changed_instances, 8, "{name}");
        }
    }
    for name in ["aim-all-translate", "mount-all-translate"] {
        for frame in &by_name(name).samples {
            assert_eq!((frame.fits, frame.fit_cache_hits), (16, 4), "{name}");
            assert_eq!((frame.changed_points, frame.dirty_instances), (2, 16));
        }
    }
    for name in ["mount-one-translate", "mount-subset-rotate"] {
        for frame in &by_name(name).samples {
            assert_eq!((frame.fits, frame.fit_cache_hits), (8, 12), "{name}");
            assert_eq!(frame.position_native_changed_instances, 8, "{name}");
        }
    }
    // Rotating an aim Point refits all four dependent roots and copies conservatively, but the
    // root whose local offset lies on the rotation axis ([0, 0, 0.5] about Z) keeps its world
    // Target, so its refit reproduces the same Pan/Tilt commands.
    for frame in &by_name("aim-subset-rotate").samples {
        assert_eq!((frame.fits, frame.fit_cache_hits), (8, 12));
        assert_eq!((frame.changed_points, frame.dirty_instances), (1, 8));
        assert_eq!(frame.position_native_changed_instances, 6);
    }
    // Unrelated native edit: Position commands never move and unrelated owners still reuse.
    // Whether the edited root/copy memo misses is observed and reported, not prescribed.
    for frame in &by_name("native-edit-one").samples {
        assert!(frame.fits <= 2, "only the edited root/copy may miss");
        assert!(frame.fit_cache_hits >= 18);
        assert_eq!(frame.position_native_changed_instances, 0);
        assert_eq!(
            frame.native_changed_instances, 2,
            "the edit reaches root and copy"
        );
        assert_eq!((frame.changed_points, frame.dirty_instances), (0, 0));
    }
    // Actual paired Pending preload lanes: independent tracking, same reuse/invalidation.
    for run in &pending {
        assert!(run.failures.is_empty(), "{}: {:?}", run.name, run.failures);
        assert!(run.convergence_windows.is_some(), "{}", run.name);
    }
    // Different capture bundles and render revisions remain in the same accepted memo
    // domain. Only actual dirty dependents refit, independently in Before and After.
    for window in &pending[0].samples {
        assert_eq!((window.fits, window.fit_cache_hits), (0, 40));
        assert_eq!(window.dirty_instances, [0, 0]);
    }
    for window in &pending[1].samples {
        assert_eq!((window.fits, window.fit_cache_hits), (16, 24));
        assert_eq!(window.dirty_instances, [8, 8]);
    }
}

// ---------------------------------------------------------------------------------------------
// Manual release-mode benchmarks: `#[ignore]`, no timing thresholds, immutable reports.

fn env_usize(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a positive integer")),
        Err(_) => default,
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .to_path_buf()
}

/// Canonical performance root: explicit `LIGHT_PERFORMANCE_DIR`, else
/// `$LIGHT_ARTIFACTS_DIR/performance`, else `<repo>/.artifacts/performance`.
fn performance_root() -> (PathBuf, &'static str) {
    let repository = repository_root();
    let resolve = |name: &str, value: String| {
        assert!(!value.is_empty(), "{name} override cannot be empty");
        let path = PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            repository.join(path)
        }
    };
    if let Ok(value) = std::env::var("LIGHT_PERFORMANCE_DIR") {
        return (
            resolve("LIGHT_PERFORMANCE_DIR", value),
            "LIGHT_PERFORMANCE_DIR",
        );
    }
    if let Ok(value) = std::env::var("LIGHT_ARTIFACTS_DIR") {
        return (
            resolve("LIGHT_ARTIFACTS_DIR", value).join("performance"),
            "LIGHT_ARTIFACTS_DIR/performance",
        );
    }
    (
        repository.join(".artifacts/performance"),
        "default .artifacts/performance",
    )
}

fn unavailable(reason: &str) -> Value {
    json!({"status": "unavailable", "reason": reason})
}
fn measured(value: Value, unit: &str, source: &str) -> Value {
    json!({"status": "measured", "value": value, "unit": unit, "source": source})
}

/// Nearest-rank distribution. Percentile resolution is limited by the sample count.
fn distribution(values: &[f64], unit: &str, source: &str) -> Value {
    if values.is_empty() {
        return unavailable("no samples recorded");
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank =
        |p: f64| sorted[((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1];
    json!({
        "status": "measured",
        "unit": unit,
        "source": source,
        "n": sorted.len(),
        "min": sorted[0],
        "p50": rank(0.50),
        "p95": rank(0.95),
        "p99": rank(0.99),
        "max": sorted[sorted.len() - 1],
        "mean": sorted.iter().sum::<f64>() / sorted.len() as f64,
        "percentileMethod": "nearest-rank",
    })
}

fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e6
}

fn scenario_json(run: &ScenarioRun) -> Value {
    let series =
        |f: &dyn Fn(&FrameObservation) -> f64| run.samples.iter().map(f).collect::<Vec<_>>();
    let wall = "std::time::Instant (monotonic) around test-side phase boundaries";
    let counter = "PositionAdapter::counters delta per accepted frame";
    let tracking = "accepted TrackingSnapshot of the Live lane";
    json!({
        "name": run.name,
        "description": run.description,
        "motion": run.moved,
        "convergenceFramesBeforeScenario": run.convergence_frames
            .map_or_else(|| unavailable("did not converge"), |frames| json!(frames)),
        "warmupFrames": run.warmup.len(),
        "sampleFrames": run.samples.len(),
        "acceptedSampleFrames": run.samples.iter().filter(|frame| frame.timing.total() > Duration::ZERO).count(),
        "simulatedWorkloadMillis": run.samples.len() as u64 * SIMULATED_FRAME_MILLIS,
        "timing": {
            "endToEnd": distribution(&series(&|frame| micros(frame.timing.total())), "us", wall),
            "phases": {
                "capture": distribution(&series(&|frame| micros(frame.timing.capture)), "us",
                    "Engine::prepare_output_frame (programmer/cue capture and native snapshot)"),
                "prepare": distribution(&series(&|frame| micros(frame.timing.prepare)), "us",
                    "prepare_captured_hybrid_frame_with_observer: static/Dynamic preparation, geometry, tracking census, fitting"),
                "finalize": distribution(&series(&|frame| micros(frame.timing.finalize)), "us",
                    "finalize_live_physical_frame: native projection/encoding, engine render, lane acceptance"),
                "geometry": unavailable("inside prepare; no existing API separates it without production instrumentation"),
                "fitting": unavailable("inside prepare; no existing API separates it without production instrumentation"),
                "nativeEncoding": unavailable("inside finalize; no existing API separates it without production instrumentation"),
            },
        },
        "counters": {
            "fits": distribution(&series(&|frame| frame.fits as f64), "count", counter),
            "fitCacheHits": distribution(&series(&|frame| frame.fit_cache_hits as f64), "count", counter),
            "candidateEvaluations": distribution(&series(&|frame| frame.candidate_evaluations as f64), "count", counter),
            "descriptorCompiles": distribution(&series(&|frame| frame.compiles as f64), "count", counter),
            "changedPoints": distribution(&series(&|frame| frame.changed_points as f64), "count", tracking),
            "dirtyInstances": distribution(&series(&|frame| frame.dirty_instances as f64), "count", tracking),
            "rowsReusingFits": distribution(&series(&|frame| frame.reused_fit_rows as f64), "count", "PositionQuality::reused_fits > 0"),
            "rowsGeometryDirty": distribution(&series(&|frame| frame.geometry_dirty_rows as f64), "count", "PositionQuality::geometry_dirty"),
            "instancesWithChangedNative": distribution(&series(&|frame| frame.native_changed_instances as f64), "count", "finalized native_raw vs previous frame"),
            "instancesWithChangedPanTilt": distribution(&series(&|frame| frame.position_native_changed_instances as f64), "count", "finalized native_raw[0..2] vs previous frame"),
        },
        "frames": run.samples.iter().map(|frame| json!({
            "captureUs": micros(frame.timing.capture),
            "prepareUs": micros(frame.timing.prepare),
            "finalizeUs": micros(frame.timing.finalize),
            "fits": frame.fits,
            "fitCacheHits": frame.fit_cache_hits,
            "candidateEvaluations": frame.candidate_evaluations,
            "changedPoints": frame.changed_points,
            "dirtyInstances": frame.dirty_instances,
        })).collect::<Vec<_>>(),
        "functional": {
            "passed": run.failures.is_empty(),
            "sharedAxisPeersShareAcceptedMemo": run.shared_memo_identity,
            "failures": run.failures,
        },
    })
}

fn binary_identity() -> Value {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let Ok(path) = std::env::current_exe() else {
        return unavailable("current_exe unavailable");
    };
    let Ok(mut file) = std::fs::File::open(&path) else {
        return unavailable("test binary unreadable");
    };
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut bytes = 0u64;
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                hasher.update(&buffer[..read]);
                bytes += read as u64;
            }
            Err(_) => return unavailable("test binary read failed"),
        }
    }
    json!({
        "status": "recorded",
        "kind": "cargo test binary (light-headless-runtime lib tests)",
        "path": path.display().to_string(),
        "sha256": hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "bytes": bytes,
    })
}

fn supplied(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn source_identity() -> Value {
    match supplied("LIGHT_POSITION_BENCH_SOURCE_SHA256") {
        Some(sha) => json!({
            "status": "supplied",
            "sourceSha256": sha,
            "manifest": supplied("LIGHT_POSITION_BENCH_SOURCE_MANIFEST")
                .map_or_else(|| unavailable("no manifest path supplied"), Value::String),
            "origin": supplied("LIGHT_POSITION_BENCH_SOURCE_ORIGIN")
                .map_or_else(|| unavailable("no origin supplied"), Value::String),
            "verifiedByBenchmark": false,
        }),
        None => unavailable(
            "set LIGHT_POSITION_BENCH_SOURCE_SHA256 (for example from tools/semantic-source-manifest.mjs)",
        ),
    }
}

fn host_identity() -> Value {
    json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "logicalCpus": std::thread::available_parallelism()
            .map_or_else(|_| unavailable("available_parallelism failed"), |n| json!(n.get())),
        "cpuModel": supplied("LIGHT_POSITION_BENCH_HOST_CPU")
            .map_or_else(|| unavailable("not supplied (LIGHT_POSITION_BENCH_HOST_CPU)"), Value::String),
        "memory": unavailable("not collected by the Rust test harness"),
        "gpu": unavailable("not used: no native Stage or presentation in this microbenchmark"),
        "loadIsolation": unavailable("other processes were not controlled"),
    })
}

fn summary_markdown(report: &Value) -> String {
    let mut text = String::new();
    text.push_str("# Captured Position tracking benchmark (TL-615)\n\n");
    text.push_str("Gated software evidence only: not production cadence, PSN delivery, native Stage, physical lamp or frame-rate acceptance. ManualClock time is simulated.\n\n");
    text.push_str(&format!(
        "- Build profile: {}\n- Requested/realized physical instances: {}/{}\n- Warmup/sample frames per scenario: {}/{}\n- Functional result: {}\n\n",
        report["build"]["profile"],
        report["workload"]["requested"]["physicalInstances"],
        report["workload"]["realized"]["physicalInstances"],
        report["parameters"]["warmupFrames"],
        report["parameters"]["sampleFrames"],
        if report["functional"]["passed"] == json!(true) { "passed" } else { "FAILED" },
    ));
    text.push_str("| Scenario | p50 us | p95 us | p99 us | fits/frame p50 | hits/frame p50 | candidate evals p50 | dirty instances p50 |\n|---|---:|---:|---:|---:|---:|---:|---:|\n");
    for scenario in report["scenarios"].as_array().into_iter().flatten().chain(
        report["pendingPairScenarios"]
            .as_array()
            .into_iter()
            .flatten(),
    ) {
        let e2e = &scenario["timing"]["endToEnd"];
        let counters = &scenario["counters"];
        let f = |value: &Value| {
            value
                .as_f64()
                .map_or_else(|| "unavailable".into(), |v| format!("{v:.0}"))
        };
        text.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            scenario["name"].as_str().unwrap_or_default(),
            f(&e2e["p50"]),
            f(&e2e["p95"]),
            f(&e2e["p99"]),
            f(&counters["fits"]["p50"]),
            f(&counters["fitCacheHits"]["p50"]),
            f(&counters["candidateEvaluations"]["p50"]),
            f(if counters["dirtyInstances"].is_null() {
                &counters["dirtyInstancesBefore"]["p50"]
            } else {
                &counters["dirtyInstances"]["p50"]
            }),
        ));
    }
    text.push_str("\nPending pair rows are per paired window: counters sum the Before and After lanes; dirty instances are the Before lane's. Timing excludes workload mutation and functional checks.\n");
    text
}

fn manual_benchmark(default_instances: usize) {
    let started_at = chrono::Utc::now();
    let started = Instant::now();
    let requested = env_usize("LIGHT_POSITION_BENCH_INSTANCES", default_instances);
    let warmup = env_usize("LIGHT_POSITION_BENCH_WARMUP", 5).min(100);
    let samples = env_usize("LIGHT_POSITION_BENCH_SAMPLES", 30).clamp(1, 1000);
    let (bench, runs, pending) = run_all(requested, warmup, samples);
    let elapsed = started.elapsed();
    let passed = runs.iter().all(|run| run.failures.is_empty())
        && pending.iter().all(|run| run.failures.is_empty());
    let report = json!({
        "schema": "tosklight.position-tracking-benchmark/v1",
        "issue": "TL-615",
        "evidenceClass": "gated-software-microbenchmark",
        "claims": {
            "productionCadence": false,
            "observedOutputRate": false,
            "psnDelivery": false,
            "nativeStage": false,
            "physicalAcceptance": false,
            "frameRateAcceptance": false,
            "statement": "Actual captured Position observer/adapter/native finalizer on contract-enabled test Engine/Runtime instances. Not production, physical, PSN, native Stage or frame-rate acceptance.",
        },
        "run": {
            "startedAtUtc": started_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "elapsedMillis": elapsed.as_secs_f64() * 1e3,
            "path": "Live: Engine::prepare_output_frame -> prepare_captured_hybrid_frame_with_observer(PositionFrameObserver) -> finalize_live_physical_frame",
            "pendingPairPath": "Pending: RetainedFrameCapture -> PairedPendingHistory window -> RetainedPreloadHybridEvaluator(PositionPreloadObserver over PhysicalPreloadLanes) -> paired finalization",
        },
        "parameters": {
            "seed": format!("{SEED:#018x}"),
            "warmupFrames": warmup,
            "sampleFrames": samples,
            "maxConvergenceFrames": MAX_CONVERGENCE_FRAMES,
        },
        "time": {
            "simulatedMillisPerFrame": SIMULATED_FRAME_MILLIS,
            "simulatedKind": "ManualClock advancement per capture: simulated workload time",
            "observedOutputRate": unavailable("not an output loop; frames run back to back"),
            "observedPsnRate": unavailable("no PSN source or delivery"),
            "wallClock": "std::time::Instant (monotonic)",
        },
        "build": {
            "profile": if cfg!(debug_assertions) { "debug-assertions (not release)" } else { "release (debug_assertions off)" },
            "binary": binary_identity(),
            "source": source_identity(),
            "testProgrammingContract": PROGRAMMING_CONTRACT_VERSION,
            "productionProgrammingContract": unavailable("not read; the benchmark uses only contract-enabled test Engine/Runtime instances"),
        },
        "host": host_identity(),
        "workload": bench.layout.json(&bench.rig),
        "scenarios": runs.iter().map(scenario_json).collect::<Vec<_>>(),
        "pendingPairScenarios": pending.iter().map(pending_json).collect::<Vec<_>>(),
        "functional": {
            "passed": passed,
            "failedScenarios": runs.iter().filter(|run| !run.failures.is_empty()).map(|run| run.name)
                .chain(pending.iter().filter(|run| !run.failures.is_empty()).map(|run| run.name))
                .collect::<Vec<_>>(),
        },
    });
    let (root, origin) = performance_root();
    let parent = root.join("position-tracking");
    std::fs::create_dir_all(&parent).unwrap();
    let run_id = format!(
        "{}-n{requested}-pid{}",
        started_at.format("%Y%m%dT%H%M%S%.3fZ"),
        std::process::id()
    );
    let directory = parent.join(run_id);
    // `create_dir` fails on an existing run: reports are immutable per run.
    std::fs::create_dir(&directory).unwrap();
    let write = |name: &str, contents: String| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(name))
            .unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    };
    let mut report = report;
    report["run"]["output"] = json!({
        "directory": directory.display().to_string(),
        "resolvedFrom": origin,
    });
    write(
        "report.json",
        serde_json::to_string_pretty(&report).unwrap() + "\n",
    );
    write("summary.md", summary_markdown(&report));
    println!("TL-615 report: {}", directory.display());
    println!("{}", summary_markdown(&report));
    assert!(
        passed,
        "functional failures (timing is recorded separately): {:?}",
        report["functional"]["failedScenarios"]
    );
}

/// Manual: `cargo test --release -p light-headless-runtime --lib tracking_bench -- --ignored`.
#[test]
#[ignore = "manual release-mode benchmark; writes an immutable report under .artifacts/performance"]
fn manual_position_tracking_benchmark_300_instances() {
    manual_benchmark(300);
}

#[test]
#[ignore = "manual release-mode benchmark; writes an immutable report under .artifacts/performance"]
fn manual_position_tracking_benchmark_1000_instances() {
    manual_benchmark(1000);
}
