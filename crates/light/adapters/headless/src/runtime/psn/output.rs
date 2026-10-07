//! Install one receiver capture; the engine's captured patch supplies Point origins.
use super::service::PsnTrackingSnapshot;
use std::sync::Arc;

pub(in crate::runtime) fn publish(state: &super::super::AppState, snapshot: PsnTrackingSnapshot) {
    let points = snapshot
        .bindings
        .iter()
        .map(|point| {
            let sample = point.identity.sample;
            light_engine::TrackedPointInput {
                binding_id: point.binding_id,
                fixture_id: light_core::FixtureId(point.point_fixture_id),
                position_metres: point.position_metres,
                position_received_at_millis: point.position_received_at_millis,
                sample: light_engine::TrackedSampleIdentity {
                    source_id: point.identity.source.to_string().into(),
                    source_generation: point.identity.source_generation,
                    source_epoch: sample.id.source_epoch,
                    sequence: sample.id.sequence,
                    sender_timestamp_micros: sample.sender_timestamp_micros,
                    accepted_at_millis: sample.accepted_at_millis,
                },
            }
        })
        .collect::<Arc<[_]>>();
    state
        .output
        .engine()
        .set_tracking_frame(Arc::new(light_engine::TrackedInputFrame {
            show_id: snapshot.show_id,
            configuration_generation: snapshot.configuration_generation,
            point_generation: snapshot.point_generation,
            source_generation: snapshot.source_generation,
            accepted_sequence: snapshot.accepted_sequence,
            sampled_at_millis: snapshot.sampled_at_millis,
            legacy_overrides: Arc::default(),
            points,
        }));
}
