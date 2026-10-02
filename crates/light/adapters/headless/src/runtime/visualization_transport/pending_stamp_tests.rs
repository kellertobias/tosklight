//! TL-594 (coordinator follow-up to track B): with the family-adapter gate on, the WebSocket
//! Preload lane is stamped with the accepted Pending publication it shows (episode, and the
//! gate ticket as `source_frame`), and its lease binds exactly that identity. An edit adopts a
//! Preload stream lease only while the session's Pending source serves that same frame.
use super::*;
use crate::runtime::output_scheduler::{
    PendingAttemptTicket, PendingEpisodeIdentity, PendingEpisodeStatus, PendingLaneValues,
    PendingNativeReadout,
};
use crate::runtime::position_readout::{
    CapturedPositionOwner, CapturedPositionReadouts, PendingPositionReadoutSource,
};
use light_application::{ProgrammingDisplayedLane, ProgrammingDisplayedSource};
use light_core::{FixtureId, ProgrammerId};
use light_engine::{PositionCommandedOwnerReadout, PositionCommandedReadout};
use light_wire::v2::output_control::{OutputFrameIdentity, OutputNativeLane, OutputPreloadState};

fn frame(ticket: u64) -> OutputFrameIdentity {
    OutputFrameIdentity {
        generation: 4_242,
        sequence: ticket,
        sampled_at: format!("2026-10-02T10:00:{:02}.000+00:00", ticket % 60),
        tracking: None,
    }
}

fn readout(identity: PendingEpisodeIdentity, ticket: u64) -> PendingNativeReadout {
    PendingNativeReadout::for_tests(
        identity,
        PendingAttemptTicket::new(ticket),
        OutputNativeLane {
            show_id: Some(identity.show_id.0),
            revision: 77,
            frame: Some(frame(ticket)),
            instances: Vec::new(),
            points: Vec::new(),
        },
        PendingLaneValues {
            values: Arc::default(),
            profile: Arc::default(),
            grand_master: 1.0,
            blackout: false,
        },
    )
}

/// The session Pending source double: serves one ticket's captured readouts.
struct TicketSource {
    programmer: ProgrammerId,
    owner: FixtureId,
    show: uuid::Uuid,
    ticket: std::sync::Mutex<u64>,
}

impl PendingPositionReadoutSource for TicketSource {
    fn capture(
        &self,
        programmer: ProgrammerId,
        owners: &[FixtureId],
    ) -> Option<CapturedPositionReadouts> {
        (programmer == self.programmer && owners == [self.owner]).then_some(())?;
        let readout = PositionCommandedOwnerReadout {
            owner: self.owner,
            commands: Some(vec![PositionCommandedReadout {
                destination: self.owner,
                emitter_id: uuid::Uuid::nil(),
                pan_degrees: 30.,
                tilt_degrees: 40.,
            }]),
        };
        Some(CapturedPositionReadouts {
            identity: frame(*self.ticket.lock().unwrap()),
            scope: VisualizationScope {
                show_id: Some(self.show),
            },
            show_revision: 77,
            owners: vec![CapturedPositionOwner {
                owner: self.owner,
                requested: None,
                zoom: None,
                readout,
            }],
        })
    }
}

fn preload_stamp(
    sent: &[VisualizationServerMessage],
) -> (u64, Option<u64>, Option<VisualizationPendingStampView>) {
    sent.iter()
        .find_map(|message| match message {
            VisualizationServerMessage::Snapshot {
                lane: VisualizationLane::Preload,
                source_frame,
                lease,
                pending,
                ..
            }
            | VisualizationServerMessage::Delta {
                lane: VisualizationLane::Preload,
                source_frame,
                lease,
                pending,
                ..
            } => Some((
                *source_frame,
                *lease,
                pending
                    .as_ref()
                    .map(|pending| VisualizationPendingStampView {
                        state: pending.status.state,
                        episode: pending.status.episode,
                        ticket: pending.frame.as_ref().map(|frame| frame.sequence),
                    }),
            )),
            _ => None,
        })
        .expect("a Preload lane message")
}

#[derive(Debug, PartialEq)]
struct VisualizationPendingStampView {
    state: OutputPreloadState,
    episode: Option<uuid::Uuid>,
    ticket: Option<u64>,
}

fn publish_live(state: &AppState) {
    let rendered = state.output.render(Default::default()).unwrap();
    state.output.render_frames_and_publish(
        &crate::runtime::visualization_frame::RenderedSemanticFrame::untraced(
            rendered,
            Default::default(),
        ),
        VisualizationScope {
            show_id: state.active_show.current().map(|show| show.id.0),
        },
    );
}

#[tokio::test]
async fn the_gated_preload_lane_is_stamped_and_leased_with_its_pending_publication() {
    let (state, directory) = crate::runtime::tests::test_state_with_family_adapters(
        Default::default(),
        None,
        light_core::programming::PROGRAMMING_CONTRACT_VERSION,
    );
    let show = light_core::ShowId(uuid::Uuid::new_v4());
    state
        .active_show
        .replace_current(Some(light_show::ShowEntry {
            is_base_show: false,
            id: show,
            name: "Pending stamps".into(),
            path: directory
                .join("shows/stamps.toskshow")
                .display()
                .to_string(),
            revision: 0,
            updated_at: String::new(),
            created_at: None,
            last_loaded_at: None,
            revision_copy: None,
        }));
    let session = session(&state);
    let programmer = state.programming.programmers().programmer_id().unwrap();
    let pending = state.programming.pending_episodes();
    assert!(pending.start_if_gated(&state));
    let executor = pending.executor().unwrap();
    assert!(
        executor.wait_for(std::time::Duration::from_secs(30), |progress, status| {
            progress.steps > 0 && *status == PendingEpisodeStatus::Idle
        })
    );
    let mut claims = SubscriptionClaims::new(state.output.clone(), session.id.0);
    claims.subscribe([VisualizationLane::Preload]);
    let outgoing = LatestOutgoing::new(state.output.clone());
    let mut publication = ClientPublicationState::new();
    let deliver = async |publication: &mut ClientPublicationState| {
        publish_live(&state);
        let source = state.output.latest_visualization_frame().unwrap();
        assert!(
            publication
                .publish(source, &claims, &state, &session, &outgoing)
                .await
        );
        messages(outgoing.next().await.unwrap())
    };

    // Gated, nothing published: the passive status, no lease, never a Live stamp.
    let (_, lease, stamp) = preload_stamp(&deliver(&mut publication).await);
    assert_eq!(lease, None);
    assert_eq!(stamp.unwrap().ticket, None);

    let identity = PendingEpisodeIdentity {
        show_id: show,
        activation: uuid::Uuid::new_v4(),
        programmer,
        episode: uuid::Uuid::new_v4(),
    };
    let episode = executor.begin_test_episode(identity);
    let owner = FixtureId::new();
    let source = Arc::new(TicketSource {
        programmer,
        owner,
        show: show.0,
        ticket: std::sync::Mutex::new(7),
    });
    state
        .output
        .pending_position_readouts()
        .install(source.clone());
    let mut leases = Vec::new();
    for ticket in [7, 8] {
        executor.publish_test_readout(&episode, readout(identity, ticket));
        let (source_frame, lease, stamp) = preload_stamp(&deliver(&mut publication).await);
        assert_eq!(
            source_frame, ticket,
            "the gate ticket, not a hub projection sequence"
        );
        assert_eq!(
            stamp,
            Some(VisualizationPendingStampView {
                state: OutputPreloadState::Published,
                episode: Some(identity.episode),
                ticket: Some(ticket),
            })
        );
        let lease = lease.expect("a published Pending lane delivery is leased");
        let Some(crate::runtime::output_readouts::DisplayedSource::PendingFrame {
            frame: leased,
            episode,
            ..
        }) = state.sessions.displayed_sources().leases().resolve(
            session.id,
            VisualizationLane::Preload,
            lease,
            std::time::Instant::now(),
        )
        else {
            panic!("the Preload lease binds the Pending frame");
        };
        assert_eq!((leased, episode), (frame(ticket), identity.episode));
        leases.push(lease);
    }

    let adopt = |lease: u64| {
        crate::runtime::output_readouts::displayed_position_readouts(
            &state,
            session.id,
            true,
            ProgrammingDisplayedSource {
                lane: ProgrammingDisplayedLane::Preload,
                lease,
            },
            None,
            &[owner],
        )
    };
    *source.ticket.lock().unwrap() = 8;
    assert_eq!(
        adopt(leases[1]).unwrap().identity,
        frame(8),
        "exactly the shown ticket"
    );
    assert!(
        adopt(leases[0]).is_none(),
        "an older ticket holds; the source has moved on"
    );
    assert!(
        crate::runtime::output_readouts::displayed_position_readouts(
            &state,
            session.id,
            false,
            ProgrammingDisplayedSource {
                lane: ProgrammingDisplayedLane::Preload,
                lease: leases[1],
            },
            None,
            &[owner],
        )
        .is_none(),
        "a Normal edit never adopts a Pending lease"
    );
    let next = PendingEpisodeIdentity {
        episode: uuid::Uuid::new_v4(),
        ..identity
    };
    let replacement = executor.begin_test_episode(next);
    executor.publish_test_readout(&replacement, readout(next, 8));
    assert!(
        adopt(leases[1]).is_none(),
        "a new episode retires the old lease"
    );
    state.output.pending_position_readouts().clear();
    pending.stop(&state);
    let _ = std::fs::remove_dir_all(directory);
}
