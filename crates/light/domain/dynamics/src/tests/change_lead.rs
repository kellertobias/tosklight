//! TL-659: a Dynamic start is owed to the first output frame sampled at or after the instant it
//! should start outputting: its activation, its activation delay, or its boundary.
use super::*;

fn started(definition: DynamicDefinition, now_millis: u64, delay_millis: u64) -> DynamicRuntime {
    let definition_id = definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    runtime
        .start(DynamicStartRequest {
            activation_delay_millis: delay_millis,
            ..start_request(
                definition_id,
                controller(7, 1, false),
                FixtureId::new(),
                now_millis,
                false,
            )
        })
        .unwrap();
    runtime
}

#[test]
fn a_start_now_dynamic_is_claimed_by_the_first_frame_after_it() {
    let mut runtime = started(definition(lane()), 1_000, 0);

    assert_eq!(runtime.claim_change_lead_start(999), None);
    assert_eq!(runtime.claim_change_lead_start(1_020), Some(1_000_000));
    assert_eq!(runtime.claim_change_lead_start(1_040), None, "claimed once");
}

#[test]
fn a_delayed_start_is_due_after_its_activation_delay() {
    let mut runtime = started(definition(lane()), 1_000, 500);

    assert_eq!(runtime.claim_change_lead_start(1_499), None);
    assert_eq!(runtime.claim_change_lead_start(1_500), Some(1_500_000));
}

#[test]
fn a_boundary_start_is_due_at_the_boundary_sampling_chooses() {
    let mut definition = definition(lane());
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational {
            numerator: 4,
            denominator: 1,
        },
    };
    definition.default_activation = ActivationPolicy::NextBoundary;
    definition.activation_boundary = ActivationBoundary::Bar;
    let mut runtime = started(definition, 1_250, 0);
    let transport = DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: 1_500,
        beat_phase: 0.5,
        phase_advancing: true,
    };

    assert_eq!(
        runtime.claim_change_lead_start(1_500),
        None,
        "nothing is due before the boundary is known"
    );
    runtime.sample_all(1_500, 10, &[transport; 5], &Sources { current: 0.0 });
    assert_eq!(runtime.claim_change_lead_start(3_999), None);
    assert_eq!(runtime.claim_change_lead_start(4_000), Some(4_000_000));
}
