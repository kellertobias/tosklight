//! TL-594 C4: the Normal lane message and the claimed readouts of one publication share one
//! accepted source and one session lease; Preload is never leased with a Live stamp.
use super::*;
use crate::runtime::output_readouts::DisplayedSource;
use light_wire::v2::output_readouts::{OutputReadoutUnavailable, VisualizationReadoutClaim};

fn session(state: &AppState) -> Session {
    let session = Session {
        id: light_core::SessionId::new(),
        token: "visualization-readouts".into(),
        connected: true,
        desk: state.installation.desk().unwrap(),
        capability: Default::default(),
    };
    state.programming.start(session.id);
    session
}

fn publish_empty_frame(state: &AppState) {
    let rendered = state
        .output
        .engine()
        .render_with_contribution_batches(Default::default(), &[])
        .unwrap();
    state.output.render_frames_and_publish(
        &crate::runtime::visualization_frame::RenderedSemanticFrame::untraced(
            rendered,
            Default::default(),
        ),
        VisualizationScope { show_id: None },
    );
}

fn messages(batch: Vec<VisualizationOutgoingMessage>) -> Vec<VisualizationServerMessage> {
    batch
        .into_iter()
        .filter_map(|message| match message {
            VisualizationOutgoingMessage::Server(message) => Some(message),
            VisualizationOutgoingMessage::Raw(_) => None,
        })
        .collect()
}

#[tokio::test]
async fn lane_and_readouts_share_the_source_and_lease_of_one_delivery() {
    let (state, directory) = crate::runtime::tests::test_state();
    let session = session(&state);
    let owner = light_core::FixtureId::new();
    publish_empty_frame(&state);
    let source = state.output.latest_visualization_frame().unwrap();
    let mut claims = SubscriptionClaims::new(state.output.clone(), session.id.0);
    claims.subscribe([VisualizationLane::Normal, VisualizationLane::Preload]);
    claims.readouts =
        crate::runtime::visualization_readouts::claimed_owners(Some(VisualizationReadoutClaim {
            fixture_ids: vec![owner.0, owner.0],
        }));
    let outgoing = LatestOutgoing::new(state.output.clone());
    let mut publication = ClientPublicationState::new();
    assert!(
        publication
            .publish(Arc::clone(&source), &claims, &state, &session, &outgoing)
            .await
    );
    let sent = messages(outgoing.next().await.unwrap());
    let mut lane_lease = None;
    let mut lane_sequence = 0;
    let mut readouts = None;
    for message in &sent {
        match message {
            VisualizationServerMessage::Snapshot {
                lane,
                lease,
                source_frame,
                sequence,
                ..
            } => match lane {
                VisualizationLane::Normal => {
                    assert_eq!(*source_frame, source.sequence);
                    lane_lease = *lease;
                    lane_sequence = *sequence;
                }
                VisualizationLane::Preload => {
                    assert_eq!(*lease, None, "Preload never carries a Live lease");
                }
            },
            VisualizationServerMessage::Readouts {
                source_frame,
                readouts: snapshot,
                sequence,
            } => {
                assert_eq!(*source_frame, source.sequence);
                assert_eq!(*sequence, lane_sequence + 1, "the stream stays consecutive");
                readouts = Some(snapshot.clone());
            }
            _ => {}
        }
    }
    let lane_lease = lane_lease.expect("the Normal lane delivery is leased");
    let readouts = readouts.expect("the claim is answered in the same publication");
    assert_eq!(readouts.lease, Some(lane_lease), "one delivery, one lease");
    assert_eq!(readouts.frame.unwrap().sequence, source.sequence);
    assert_eq!(readouts.unavailable, None::<OutputReadoutUnavailable>);
    assert_eq!(
        readouts.owners.len(),
        2,
        "claim order and duplicates are kept"
    );
    assert!(
        !readouts.owners[0].position.available,
        "an unpatched owner is unavailable"
    );
    let Some(DisplayedSource::Live(leased)) = state.sessions.displayed_sources().leases().resolve(
        session.id,
        VisualizationLane::Normal,
        lane_lease,
        std::time::Instant::now(),
    ) else {
        panic!("the lane lease resolves to the delivered Live frame");
    };
    assert!(Arc::ptr_eq(&leased, &source));

    publish_empty_frame(&state);
    claims.readouts = crate::runtime::visualization_readouts::claimed_owners(None);
    let next = state.output.latest_visualization_frame().unwrap();
    assert!(
        publication
            .publish(next, &claims, &state, &session, &outgoing)
            .await
    );
    let sent = messages(outgoing.next().await.unwrap());
    assert!(
        !sent
            .iter()
            .any(|message| matches!(message, VisualizationServerMessage::Readouts { .. })),
        "a Subscribe without a claim clears it"
    );
    let next_lease = sent.iter().find_map(|message| match message {
        VisualizationServerMessage::Delta {
            lane: VisualizationLane::Normal,
            lease,
            ..
        }
        | VisualizationServerMessage::Snapshot {
            lane: VisualizationLane::Normal,
            lease,
            ..
        } => *lease,
        _ => None,
    });
    assert!(
        next_lease.is_some_and(|lease| lease != lane_lease),
        "a lease per delivery"
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[path = "pending_stamp_tests.rs"]
mod pending_stamp;
