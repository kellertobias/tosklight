//! TL-639 round 2: `--digest-lifecycle` drives the operator lifecycle through a digest run, so
//! two builds are also compared while Programmer values fade, a FixAT replaces a family, fixtures
//! are Frozen (some with installation Pan/Tilt inversion) and released, a Preload GO fades its
//! committed values in, and the Grand Master, Blackout and control loss act on the output. The
//! events are applied before the frame of their tick through the same Programmer and patch entry
//! points the desk uses; each event's outcome is part of the digest.
use crate::light_benchmark::scenario::BenchmarkScenario;
use light_core::{
    ApplicationClock, AttributeKey, AttributeValue, FixtureId,
    programming::{
        ColorIntent, ColorProgram, PositionIntent, ProgrammingOwner, VirtualColorAuthoringV1,
        VirtualColorRecipe,
    },
};
use light_dynamics::{DynamicSemanticValue, DynamicValueTiming, ProgrammingFamilyFixAt};
use light_fixture::{FixtureFreezeState, FreezeFamily, FrozenFixtureTarget};
use light_programmer::{
    DynamicProgrammerValueMutation, PreloadProgrammerValueMutation, PreloadProgrammerValueTiming,
};
use std::{collections::HashMap, sync::Arc};

/// Ticks at which each event is applied (before that tick's frame).
const FADE_TICK: u64 = 2;
const FIX_AT_TICK: u64 = 5;
const FREEZE_TICK: u64 = 8;
const PRELOAD_TICK: u64 = 11;
const UNFREEZE_TICK: u64 = 17;
const GRAND_MASTER_TICKS: std::ops::Range<u64> = 20..23;
const BLACKOUT_TICK: u64 = 23;
const CONTROL_LOSS_TICKS: std::ops::Range<u64> = 24..27;
const FADE_MILLIS: u64 = 150;
const PRELOAD_FADE_MILLIS: u64 = 200;

/// Apply the event of `tick`, if any, and describe its outcome for the digest.
pub fn apply(
    scenario: &BenchmarkScenario,
    tick: u64,
    last_values: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
) -> Result<Option<String>, String> {
    let programmers = &scenario.programmers;
    let Some(state) = programmers.active().into_iter().next() else {
        return Ok(None);
    };
    let session = state.session_id;
    let mut families = state
        .values
        .iter()
        .filter(|value| {
            value.attribute == ProgrammingOwner::Color.key()
                || value.attribute == ProgrammingOwner::Position.key()
        })
        .map(|value| (value.fixture_id, value.attribute.clone()))
        .collect::<Vec<_>>();
    families.sort_by_key(|(fixture, attribute)| (fixture.0, attribute.0.clone()));
    families.dedup();
    let mut fixtures = scenario
        .engine
        .snapshot()
        .fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    fixtures.sort_by_key(|fixture| fixture.0);
    let every = |items: &[(FixtureId, AttributeKey)], step: usize, offset: usize| {
        items
            .iter()
            .enumerate()
            .filter(move |(index, _)| index % step == offset)
            .map(|(_, item)| item.clone())
            .collect::<Vec<_>>()
    };
    match tick {
        FADE_TICK => {
            let mut assignments = every(&families, 8, 0)
                .into_iter()
                .map(|(fixture, attribute)| (fixture, attribute.clone(), alternate(&attribute)))
                .collect::<Vec<_>>();
            assignments.extend(fixtures.iter().step_by(8).map(|fixture| {
                (
                    *fixture,
                    AttributeKey::intensity(),
                    AttributeValue::Normalized(0.3),
                )
            }));
            let count = assignments.len();
            programmers.set_many_faded_with_timing(session, assignments, Some(FADE_MILLIS), None);
            Ok(Some(format!("fade {count}")))
        }
        FIX_AT_TICK => {
            let mutations = every(&families, 16, 3)
                .into_iter()
                .map(|(fixture, attribute)| {
                    let owner = if attribute == ProgrammingOwner::Color.key() {
                        ProgrammingOwner::Color
                    } else {
                        ProgrammingOwner::Position
                    };
                    let mask =
                        ProgrammingFamilyFixAt::from_family(owner, None, alternate(&attribute))
                            .map_err(|error| format!("digest FixAT: {error}"))?;
                    Ok(DynamicProgrammerValueMutation::Set {
                        fixture_id: fixture,
                        attribute,
                        value: DynamicSemanticValue::ProgrammingFixAt {
                            mask,
                            timing: DynamicValueTiming::default(),
                        },
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let applied = !mutations.is_empty()
                && programmers.apply_dynamic_values(session, &mutations, None);
            Ok(Some(format!("fixat {} {applied}", mutations.len())))
        }
        FREEZE_TICK => {
            let mut frozen = 0;
            replace_freeze(scenario, |fixture_id, index| {
                let target = match index % 32 {
                    1 => FrozenFixtureTarget {
                        full: true,
                        ..Default::default()
                    },
                    9 => FrozenFixtureTarget {
                        families: vec![FreezeFamily::Intensity, FreezeFamily::Color],
                        values: last_values
                            .iter()
                            .filter(|((fixture, attribute), _)| {
                                *fixture == fixture_id
                                    && (attribute.is_intensity()
                                        || attribute.0.starts_with("color"))
                            })
                            .map(|((_, attribute), value)| (attribute.clone(), value.clone()))
                            .collect(),
                        ..Default::default()
                    },
                    _ => return None,
                };
                frozen += 1;
                Some(target)
            })?;
            Ok(Some(format!("freeze {frozen}")))
        }
        PRELOAD_TICK => {
            let armed = programmers.arm_preload(session, true);
            let mutations = every(&families, 8, 4)
                .into_iter()
                .map(
                    |(fixture, attribute)| PreloadProgrammerValueMutation::SetFixture {
                        fixture_id: fixture,
                        value: alternate(&attribute),
                        attribute,
                        timing: PreloadProgrammerValueTiming::default(),
                    },
                )
                .chain(fixtures.iter().skip(4).step_by(8).map(|fixture| {
                    PreloadProgrammerValueMutation::SetFixture {
                        fixture_id: *fixture,
                        attribute: AttributeKey::intensity(),
                        value: AttributeValue::Normalized(0.6),
                        timing: PreloadProgrammerValueTiming::default(),
                    }
                }))
                .collect::<Vec<_>>();
            let stored = programmers.apply_preload_values(session, &mutations);
            let went = programmers.activate_preload_at_with_fade(
                session,
                scenario.clock.now(),
                PRELOAD_FADE_MILLIS,
            );
            programmers.set_modes(session, Some(false), None, None, None);
            Ok(Some(format!(
                "preload {} {armed} {stored} {went}",
                mutations.len()
            )))
        }
        UNFREEZE_TICK => {
            replace_freeze(scenario, |_, _| None)?;
            Ok(Some("unfreeze".into()))
        }
        _ => Ok(None),
    }
}

/// The render options of `tick`: Grand Master at half, then Blackout, then control loss.
pub fn render_options(tick: u64) -> light_engine::RenderOptions {
    let mut options = light_engine::RenderOptions::default();
    if GRAND_MASTER_TICKS.contains(&tick) {
        options.grand_master = 0.5;
    }
    options.blackout = tick == BLACKOUT_TICK;
    if CONTROL_LOSS_TICKS.contains(&tick) {
        options.control_loss_progress = Some((tick - CONTROL_LOSS_TICKS.start + 1) as f32 / 4.0);
    }
    options
}

/// A different value of the same family: a cool recipe Color, or an offset Angle.
fn alternate(attribute: &AttributeKey) -> AttributeValue {
    if *attribute == ProgrammingOwner::Color.key() {
        let recipe = VirtualColorRecipe {
            version: 1,
            rgb: [0.2, 0.5, 1.0],
            amber: 0.0,
            approximate: false,
        };
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent {
                base_xyz: VirtualColorAuthoringV1::recipe_xyz(&recipe)
                    .expect("valid digest recipe"),
                recipe,
                ..ColorIntent::default()
            },
        }))
    } else {
        AttributeValue::Position(Arc::new(PositionIntent::angles(20.0, 60.0)))
    }
}

/// Replace every root fixture's Freeze state by `target(fixture, sorted index)` through a patch
/// snapshot replacement, as a Freeze action does.
fn replace_freeze(
    scenario: &BenchmarkScenario,
    mut target: impl FnMut(FixtureId, usize) -> Option<FrozenFixtureTarget>,
) -> Result<(), String> {
    let mut snapshot = scenario.engine.snapshot().as_ref().clone();
    let mut order = snapshot
        .fixtures
        .iter()
        .enumerate()
        .map(|(index, fixture)| (fixture.fixture_id.0, index))
        .collect::<Vec<_>>();
    order.sort_unstable();
    let mut fixtures = snapshot.fixtures.to_vec();
    for (sorted, (_, index)) in order.into_iter().enumerate() {
        let fixture = &mut fixtures[index];
        // Installation inversion on some fixtures from the Freeze onwards (kept after release).
        if sorted % 32 == 17 {
            fixture.invert_pan = true;
            fixture.invert_tilt = true;
        }
        let mut state = FixtureFreezeState::default();
        if let Some(frozen) = target(fixture.fixture_id, sorted) {
            state.targets.insert(fixture.fixture_id, frozen);
        }
        fixture.freeze = state;
    }
    snapshot.fixtures = fixtures.into();
    snapshot.revision += 1;
    match scenario.live.as_ref() {
        Some(live) => live.bench.replace_snapshot(snapshot),
        None => scenario
            .engine
            .replace_snapshot(snapshot)
            .map_err(|error| error.to_string()),
    }
    .map_err(|error| format!("digest Freeze: {error}"))
}
