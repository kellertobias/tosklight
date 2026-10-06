//! TL-596: typed semantic programming for the established capacity workloads.
//!
//! The legacy builders sample scalar `color.*`/`pan`/`tilt` lanes in the benchmark and hand the
//! engine finished batches. The semantic variants keep the same fixtures, Dynamic count and
//! lane count per Dynamic, but author typed Programming lanes (semantic Color recipe
//! components and Angle Pan/Tilt next to the ordinary Intensity lane), install them in a real
//! `DynamicRuntime`, give every animated target a static semantic base, and start each Dynamic
//! through Programmer `DynamicOn` values exactly as `DynamicsService::start` expands them. The
//! frame is then produced by the production Live transaction (`LiveOutputBench`).
use light_core::{
    AttributeValue, FixtureId, SessionId,
    programming::{
        ColorIntent, ColorProgram, PROGRAMMING_CONTRACT_VERSION, PositionIntent, ProgrammingOwner,
        VirtualColorAuthoringV1, VirtualColorRecipe,
    },
};
use light_dynamics::{
    DynamicDefinition, DynamicDefinitionSnapshot, DynamicInstanceOverrides, DynamicLane,
    DynamicReference, DynamicRuntime, DynamicSemanticValue, Rational,
};
use light_engine::Engine;
use light_headless_runtime::output_benchmark::LiveOutputBench;
use light_programmer::{DynamicProgrammerValueMutation, ProgrammerRegistry};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

/// One started Dynamic: its definition, ordered targets and authored instance link.
pub(super) struct DynamicStart {
    pub definition: DynamicDefinition,
    pub targets: Vec<FixtureId>,
    pub link: Uuid,
}

/// The typed lanes that replace the legacy `color.red/green/blue` and `pan`/`tilt` lanes.
pub(super) const SEMANTIC_LANE_ATTRIBUTES: &[&str] = &[
    "intensity",
    "color.recipe.red",
    "color.recipe.green",
    "color.recipe.blue",
    "position.angles.pan",
    "position.angles.tilt",
];

/// Replace every lane after the first (the legacy Intensity lane) with the five typed lanes,
/// shaped like the variant's own mode. Lane ids stay deterministic per definition.
pub(super) fn typed_lanes(
    definition: &mut DynamicDefinition,
    variant: usize,
) -> Result<(), String> {
    let intensity = definition.lanes[0].clone();
    let mut lanes = vec![intensity];
    let components = [
        (recipe_address("red"), [0.1, 0.9]),
        (recipe_address("green"), [0.2, 0.8]),
        (recipe_address("blue"), [0.0, 0.7]),
        (angle_address("pan"), [-40.0, 40.0]),
        (angle_address("tilt"), [-25.0, 25.0]),
    ];
    for (index, (address, [low, high])) in components.into_iter().enumerate() {
        let id = Uuid::new_v5(
            &definition.id,
            format!("tl596-typed-lane-{index}").as_bytes(),
        );
        let lane = json!({
            "id": id,
            "speed_multiplier": { "numerator": 1, "denominator": 1 },
            "width": 1,
            "random_group_id": null,
            "phase": null,
            "programming": { "address": address, "configuration": configuration(variant, low, high) },
        });
        lanes.push(
            serde_json::from_value::<DynamicLane>(lane)
                .map_err(|error| format!("typed benchmark lane: {error}"))?,
        );
    }
    definition.lanes = lanes;
    Ok(())
}

fn recipe_address(component: &str) -> Value {
    json!({
        "representation": { "kind": "semantic_color", "basis": "recipe" },
        "component": { "kind": "color", "component": component },
    })
}

fn angle_address(axis: &str) -> Value {
    json!({ "representation": { "kind": "angles" }, "component": { "kind": axis } })
}

fn scalar(value: f32) -> Value {
    json!({ "kind": "value", "value": { "kind": "scalar", "value": value } })
}

fn pwm() -> Value {
    json!({
        "attack": 0.1, "on": 0.35, "decay": 0.15, "off": 0.4,
        "attack_interpolation": "ease_in", "decay_interpolation": "ease_out",
    })
}

/// The legacy variants are Keyframes (Current to Preset), PWM Max/Min on a Speed Group,
/// Middle/Amplitude around Current and seeded Random. Typed lanes mirror the first three; the
/// Random variant's typed lanes use a sinus Max/Min (its Random group stays on Intensity).
fn configuration(variant: usize, low: f32, high: f32) -> Value {
    match variant % 4 {
        0 => json!({ "mode": "keyframes", "configuration": {
            "points": [
                { "position": 0, "source": { "kind": "current" }, "interpolation": "linear" },
                { "position": 0.5, "source": scalar(high), "interpolation": "ease_in_out" },
            ],
            "size": 1,
        }}),
        1 => json!({ "mode": "max_min", "configuration": {
            "minimum": scalar(low), "maximum": scalar(high), "function": "pwm", "size": 1,
            "pwm": pwm(),
        }}),
        2 => json!({ "mode": "middle_amplitude", "configuration": {
            "middle": { "kind": "current" },
            "amplitude": { "kind": "scalar", "value": (high - low) / 2.0 },
            "function": "sinus", "size": 1, "pwm": pwm(), "invert_waveform": false,
        }}),
        _ => json!({ "mode": "max_min", "configuration": {
            "minimum": scalar(low), "maximum": scalar(high), "function": "sinus", "size": 1,
            "pwm": pwm(),
        }}),
    }
}

/// A static semantic base for every animated target: a warm recipe Color and a centred Angle.
/// Typed component lanes animate around whichever base wins composition.
pub(super) fn set_semantic_bases(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    targets: &[FixtureId],
) -> Result<(), String> {
    let recipe = VirtualColorRecipe {
        version: 1,
        rgb: [1.0, 0.6, 0.3],
        amber: 0.0,
        approximate: false,
    };
    let color = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent {
            base_xyz: VirtualColorAuthoringV1::recipe_xyz(&recipe)
                .map_err(|error| format!("benchmark Color base: {error}"))?,
            recipe,
            ..ColorIntent::default()
        },
    }));
    let position = AttributeValue::Position(Arc::new(PositionIntent::angles(0.0, 45.0)));
    programmers.set_many(
        session,
        targets.iter().flat_map(|target| {
            [
                (*target, ProgrammingOwner::Color.key(), color.clone()),
                (*target, ProgrammingOwner::Position.key(), position.clone()),
            ]
        }),
    );
    Ok(())
}

/// Start every Dynamic the way `DynamicsService::start` does: one `DynamicOn` per target and
/// lane, keyed by the lane's output owner and sharing the authored instance link.
pub(super) fn start_dynamics(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    starts: &[DynamicStart],
) -> Result<(), String> {
    for start in starts {
        let reference = DynamicReference {
            dynamic_id: Some(start.definition.id),
            last_known_pool_number: start.definition.pool_number,
            embedded_fallback: DynamicDefinitionSnapshot {
                definition: Arc::new(start.definition.clone()),
            },
        };
        let mutations = start
            .targets
            .iter()
            .flat_map(|fixture_id| {
                start
                    .definition
                    .lanes
                    .iter()
                    .map(|lane| DynamicProgrammerValueMutation::Set {
                        fixture_id: *fixture_id,
                        attribute: lane.output_owner(),
                        value: DynamicSemanticValue::DynamicOn {
                            instance_link: start.link,
                            dynamic: reference.clone(),
                            lane_id: lane.id,
                            overrides: DynamicInstanceOverrides {
                                size: 1.0,
                                speed_multiplier: Rational::ONE,
                                phase_offset_degrees: 0.0,
                            },
                            timing: Default::default(),
                        },
                    })
            })
            .collect::<Vec<_>>();
        if !programmers.apply_dynamic_values(session, &mutations, None) {
            return Err(format!(
                "Dynamic {} start produced no Programmer change",
                start.definition.name
            ));
        }
    }
    Ok(())
}

/// TL-639: set by `--digest-ticks` before any scenario is built, so instance identities (and
/// with them Random lanes and equal-priority order) repeat between processes.
pub(super) static DERIVED_INSTANCE_IDS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// The production Live transaction over `engine`, with every definition installed.
pub(super) fn live_bench(
    engine: Arc<Engine>,
    definitions: impl IntoIterator<Item = DynamicDefinition>,
    rate_hz: u16,
    publish: bool,
) -> Result<LiveOutputBench, String> {
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    if DERIVED_INSTANCE_IDS.load(std::sync::atomic::Ordering::Relaxed) {
        runtime.derive_instance_ids_from(Uuid::from_u128(0x7106_3900));
    }
    runtime
        .install_definitions(definitions)
        .map_err(|error| format!("install benchmark Dynamics: {error}"))?;
    let bench = LiveOutputBench::new(engine, runtime, rate_hz, publish)?;
    if !bench.family_engaged() {
        return Err("the Live family adapters are not engaged for this engine".into());
    }
    Ok(bench)
}

/// What a capacity builder needs to produce its semantic variant.
#[derive(Clone, Copy, Debug)]
pub(super) struct SemanticBuild {
    pub rate_hz: u16,
    pub publish: bool,
    /// TL-641 `--start-latency`: leave the Dynamics unstarted for the probe to start.
    pub defer_starts: bool,
}

/// The legacy headless-stress Dynamics with typed lanes, each started on its own partition.
pub(super) fn stress_starts(
    targets: &[FixtureId],
    instance_count: usize,
) -> Result<Vec<DynamicStart>, String> {
    super::scenario::production_definitions(targets, instance_count)?
        .into_iter()
        .enumerate()
        .map(|(index, (mut definition, partition))| {
            typed_lanes(&mut definition, index)?;
            definition.target_binding = light_dynamics::DynamicTargetBinding::Targetless;
            Ok(DynamicStart {
                link: Uuid::new_v5(&definition.id, b"tosklight:tl596:activation"),
                definition,
                targets: partition,
            })
        })
        .collect()
}

/// The sustained-show Intensity Dynamics. The Programmer holds one Dynamic per target and
/// attribute, so the legacy overlap (variant 1 on every target, the others on every fourth) is
/// realized as a four-way partition of the same targets.
pub(super) fn intensity_starts(targets: &[FixtureId]) -> Vec<DynamicStart> {
    super::scenario::intensity_definitions(targets)
        .into_iter()
        .enumerate()
        .map(|(index, mut definition)| {
            definition.target_binding = light_dynamics::DynamicTargetBinding::Targetless;
            DynamicStart {
                link: Uuid::new_v5(&definition.id, b"tosklight:tl596:activation"),
                targets: targets
                    .iter()
                    .enumerate()
                    .filter(|(target, _)| target % 4 == index)
                    .map(|(_, target)| *target)
                    .collect(),
                definition,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_dynamics::DynamicLaneBody;

    #[test]
    fn typed_stress_dynamics_keep_the_legacy_shape_and_install_at_contract_one() {
        let targets = (0..40)
            .map(|index| FixtureId(Uuid::from_u128(0x5960_0000 + index)))
            .collect::<Vec<_>>();
        let starts = stress_starts(&targets, 20).unwrap();
        assert_eq!(starts.len(), 20);
        assert!(starts.iter().all(|start| start.targets.len() == 2));
        for start in &starts {
            let owners = start
                .definition
                .lanes
                .iter()
                .map(|lane| lane.output_owner().0.to_string())
                .collect::<Vec<_>>();
            assert_eq!(
                owners,
                [
                    "intensity",
                    "color",
                    "color",
                    "color",
                    "position",
                    "position"
                ]
            );
            assert!(matches!(
                start.definition.lanes[0].body,
                DynamicLaneBody::LegacyScalar(_)
            ));
        }
        let mut runtime =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        runtime
            .install_definitions(starts.into_iter().map(|start| start.definition))
            .unwrap();
    }

    #[test]
    fn sustained_intensity_dynamics_partition_every_target_once() {
        let targets = (0..10)
            .map(|index| FixtureId(Uuid::from_u128(0x5961_0000 + index)))
            .collect::<Vec<_>>();
        let starts = intensity_starts(&targets);
        let mut covered = starts
            .iter()
            .flat_map(|start| start.targets.iter().copied())
            .collect::<Vec<_>>();
        covered.sort_by_key(|target| target.0);
        assert_eq!(covered, targets);
        let mut runtime =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        runtime
            .install_definitions(starts.into_iter().map(|start| start.definition))
            .unwrap();
    }
}
