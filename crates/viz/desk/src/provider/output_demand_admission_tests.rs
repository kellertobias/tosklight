//! Exercise tagged asynchronous replies through the real provider inbox and native decode.
use super::*;
use crate::provider::output_demand::{OutputRead, OutputRequest};
fn send_tagged(desk: &Desk, request: OutputRequest, output: OutputDmxSnapshot, legacy: bool) {
    desk.outbox
        .send_output(OutputRead {
            request,
            output: Box::new(output),
            preload: legacy.then(|| {
                Box::new(crate::wire::PreloadProjection {
                    fixture_values: vec![crate::wire::PreloadFixtureValue {
                        fixture_id: desk.rig.fixture,
                        attribute: "intensity".into(),
                        value: crate::wire::PreloadAttributeValue::Normalized(0.125),
                    }],
                })
            }),
        })
        .unwrap();
}

fn tagged(
    desk: &mut Desk,
    request: OutputRequest,
    output: OutputDmxSnapshot,
    legacy: bool,
) -> usize {
    send_tagged(desk, request, output, legacy);
    desk.provider
        .poll()
        .iter()
        .filter(|e| matches!(e, ProviderEvent::Values(_)))
        .count()
}

#[test]
fn queued_output_does_not_cross_a_lane_change_or_same_show_delta() {
    let mut desk = Desk::new(false);
    let live = desk.provider.output_demand.snapshot();
    let initial = desk.output(Some(stamp(1, 2)), A);
    assert_eq!(tagged(&mut desk, live, initial, false), 1);

    send_tagged(&desk, live, desk.output(Some(stamp(2, 2)), B), false);
    desk.provider.follow_preload(true);
    assert!(
        !desk
            .provider
            .poll()
            .iter()
            .any(|event| matches!(event, ProviderEvent::Values(_)))
    );
    assert_eq!(desk.point_height(), A.point_z);

    let preload = desk.provider.output_demand.snapshot();
    let old = desk.output(Some(stamp(3, 2)), B);
    // The marker may already have been consumed when the delta reaches the render thread.
    desk.provider.queue_desk_output(old.clone());
    send_tagged(&desk, preload, old, false);
    let mut plan = desk.rig.plan();
    plan.scene.revision = 2;
    desk.send(Message::delta(
        plan,
        Vec::new(),
        ProviderDiagnostics::default(),
    ));
    let events = desk.provider.poll();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ProviderEvent::SceneDelta(_)))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, ProviderEvent::Values(_)))
    );
    assert_eq!(desk.point_height(), A.point_z);

    let fresh = desk.output(Some(stamp(4, 2)), B);
    assert_eq!(tagged(&mut desk, preload, fresh, false), 1);
    assert_eq!(desk.point_height(), B.point_z);
}
#[test]
fn obsolete_native_and_legacy_responses_never_mutate_after_double_flip() {
    let mut desk = Desk::new(false);
    let live = desk.provider.output_demand.snapshot();
    let output = desk.output(Some(stamp(5, 2)), A);
    assert_eq!(tagged(&mut desk, live, output, false), 1);
    let picture = desk.picture();
    desk.provider.follow_preload(true);
    let preload = desk.provider.output_demand.snapshot();
    desk.provider.follow_preload(false);
    for old in [live, preload] {
        let output = desk.output(Some(stamp(100, 2)), B);
        assert_eq!(tagged(&mut desk, old, output, true), 0);
        assert_eq!(desk.picture(), picture);
        assert!(desk.provider.preload_projection.fixture_values.is_empty());
    }
    let current = desk.provider.output_demand.snapshot();
    let old = desk.output(Some(stamp(4, 2)), B);
    assert_eq!(
        tagged(&mut desk, current, old, false),
        0,
        "mode flips do not reset accepted-source ordering"
    );
    let fresh = desk.output(Some(stamp(6, 2)), B);
    assert_eq!(tagged(&mut desk, current, fresh, false), 1);
    assert_ne!(desk.picture(), picture);
}
#[test]
fn same_live_frame_can_apply_new_preload_and_return_to_live() {
    let mut desk = Desk::new(false);
    let live = desk.provider.output_demand.snapshot();
    let output = desk.output(Some(stamp(5, 2)), A);
    assert_eq!(tagged(&mut desk, live, output.clone(), false), 1);
    let live_picture = desk.point_height();
    desk.provider.follow_preload(true);
    let request = desk.provider.output_demand.snapshot();
    let mut with_preload = output.clone();
    let mut lane = with_preload.native.clone().unwrap();
    lane.points[0].offset_metres[2] = -9.0;
    with_preload.preload = Some(lane);
    assert_eq!(tagged(&mut desk, request, with_preload, false), 1);
    assert_eq!(desk.point_height(), -9.0);
    desk.provider.follow_preload(false);
    let request = desk.provider.output_demand.snapshot();
    assert_eq!(tagged(&mut desk, request, output, false), 1);
    assert_eq!(desk.point_height(), live_picture);
}
#[test]
fn queued_old_mode_data_is_cleared_before_new_mode_can_use_it() {
    let mut desk = Desk::new(false);
    let output = desk.output(Some(stamp(5, 2)), A);
    desk.provider.queue_desk_output(output);
    desk.provider
        .preload_projection
        .fixture_values
        .push(crate::wire::PreloadFixtureValue {
            fixture_id: desk.rig.fixture,
            attribute: "intensity".into(),
            value: crate::wire::PreloadAttributeValue::Normalized(1.0),
        });
    desk.provider.follow_preload(true);
    assert!(desk.provider.desk_output.is_none());
    assert!(desk.provider.preload_projection.fixture_values.is_empty());
}

#[test]
fn initially_followed_legacy_overlay_applies_and_clears_on_live_switch() {
    let mut desk = Desk::new(true);
    desk.provider.follow_preload(true);
    let request = desk.provider.output_demand.snapshot();
    let mut output = desk.output(Some(stamp(5, 2)), A);
    output.native_protocol = 0;
    output.native = None;
    assert_eq!(tagged(&mut desk, request, output.clone(), true), 1);
    assert!((desk.provider.values.emitters[0].intensity - 0.125).abs() < 0.0001);
    desk.provider.follow_preload(false);
    let request = desk.provider.output_demand.snapshot();
    assert_eq!(tagged(&mut desk, request, output, false), 1);
    assert!((desk.provider.values.emitters[0].intensity - 0.125).abs() > 0.001);
    assert!(desk.provider.preload_projection.fixture_values.is_empty());
}
