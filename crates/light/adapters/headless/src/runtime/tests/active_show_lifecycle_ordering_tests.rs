use super::*;
use light_application::{ActionContext, ActionError, ActionSource, ActiveShowPorts};
use std::sync::mpsc;

#[derive(Clone, Copy, Debug)]
enum Site {
    ActiveShow,
    Patch,
    Update,
    Topology,
    Import,
    CueTransfer,
    CueDeletion,
}

const SITES: [Site; 7] = [
    Site::ActiveShow,
    Site::Patch,
    Site::Update,
    Site::Topology,
    Site::Import,
    Site::CueTransfer,
    Site::CueDeletion,
];

/// Hold the same exclusive activation permit used by Patch while a real HTTP action arrives.
/// The action must wait without occupying Programmer, so the activation owner can enter it.
pub(super) async fn request_waiting_for_activation_keeps_programmer_available(
    state: &AppState,
    request: impl std::future::Future<Output = Response> + Send + 'static,
) -> Response {
    let activation = state.active_show.acquire().await;
    let request = tokio::spawn(request);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !request.is_finished(),
        "valid action did not wait for activation"
    );
    let probe_state = state.clone();
    let mut probe = tokio::task::spawn_blocking(move || {
        probe_state.programming.programmers().serialized(|| ())
    });
    let available = match tokio::time::timeout(Duration::from_millis(200), &mut probe).await {
        Ok(result) => {
            result.unwrap();
            true
        }
        Err(_) => false,
    };
    // Release even after a failed ordering check, so old behavior cannot wedge the test process.
    drop(activation);
    if !available {
        tokio::time::timeout(Duration::from_secs(2), probe)
            .await
            .unwrap()
            .unwrap();
    }
    let response = tokio::time::timeout(Duration::from_secs(2), request)
        .await
        .unwrap()
        .unwrap();
    assert!(
        available,
        "HTTP action occupied Programmer while waiting for Patch activation"
    );
    response
}

fn session(state: &AppState) -> Session {
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "show-lifecycle-ordering".into(),
        connected: true,
        desk: state.installation.desk().unwrap(),
    };
    state.programming.start(session.id);
    attach_session_command_context(state, &session);
    state.sessions.insert_session(session.clone());
    session
}

fn context(session: &Session) -> ActionContext {
    ActionContext::operator(session.desk.id, session.id.0, ActionSource::Http)
}

fn owner() -> ProgrammingInstallOwner {
    ProgrammingInstallOwner {
        gesture: ProgrammingOwnerGesturePolicy::Preserve,
        highlight: ProgrammingOwnerHighlightPolicy::DeferToOuterInteraction,
    }
}

/// The callback is exactly where ActiveShowService takes its application operation mutex.
/// Calling this boundary directly keeps the test independent of filesystem commit latency.
fn lifecycle<T>(
    state: &AppState,
    session: &Session,
    site: Site,
    inherited: bool,
    operation: impl FnOnce() -> Result<T, ActionError>,
) -> Result<T, ActionError> {
    let context = context(session);
    let show = light_core::ShowId(Uuid::from_u128(548));
    match site {
        Site::ActiveShow => {
            let ports = if inherited {
                ServerActiveShowPorts::show_objects_with_programming_owner(state.clone(), owner())
            } else {
                ServerActiveShowPorts::show_objects(state.clone())
            };
            ports.run_active_show_lifecycle(&context, show, operation)
        }
        Site::Patch => {
            let ports = if inherited {
                ServerShowPatchPorts::with_activation_held(state.clone())
            } else {
                ServerShowPatchPorts::new(state.clone())
            };
            ports.run_active_show_lifecycle(&context, show, operation)
        }
        Site::Update => {
            ServerProgrammingUpdatePorts::new(state.clone(), session.clone(), inherited, false)
                .run_active_show_lifecycle(&context, show, operation)
        }
        Site::Topology => {
            let ports = if inherited {
                ServerPlaybackTopologyPorts::within_active_show(
                    state.clone(),
                    session.clone(),
                    show,
                )
            } else {
                ServerPlaybackTopologyPorts::new(state.clone(), session.clone(), show)
            };
            ports.run_active_show_lifecycle(&context, show, operation)
        }
        Site::Import => {
            let ports = if inherited {
                ServerSelectiveImportPorts::with_programming_owner(state.clone(), owner())
            } else {
                ServerSelectiveImportPorts::new(state.clone())
            };
            ports.run_active_show_lifecycle(&context, show, operation)
        }
        Site::CueTransfer => command_http::ServerProgrammingCueTransferPorts::new(
            state.clone(),
            session.clone(),
            inherited,
        )
        .run_active_show_lifecycle(&context, show, operation),
        Site::CueDeletion => command_http::ServerProgrammingCueDeletionPorts::new(
            state.clone(),
            session.clone(),
            inherited,
        )
        .run_active_show_lifecycle(&context, show, operation),
    }
}

#[test]
fn every_adapter_waits_for_programmer_before_entering_show_operation() {
    let (state, directory) = test_state();
    let session = session(&state);
    for site in SITES {
        let (started_tx, started_rx) = mpsc::channel();
        let (entered_tx, entered_rx) = mpsc::channel();
        let worker_state = state.clone();
        let worker_session = session.clone();
        let worker = state.programming.programmers().serialized(|| {
            let worker = std::thread::spawn(move || {
                // The base adapter's caller owns activation; the other standalone adapters
                // retain their own existing acquisition policy.
                let _activation = matches!(site, Site::ActiveShow)
                    .then(|| worker_state.active_show.try_acquire().unwrap());
                started_tx.send(()).unwrap();
                lifecycle(&worker_state, &worker_session, site, false, || {
                    entered_tx.send(()).unwrap();
                    Ok(())
                })
                .unwrap();
            });
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(
                entered_rx.recv_timeout(Duration::from_millis(50)),
                Err(mpsc::RecvTimeoutError::Timeout),
                "{site:?} entered the show operation while another thread owned Programmer",
            );
            worker
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.join().unwrap();
    }
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn inherited_lifecycles_reuse_programmer_and_preserve_outer_desk_ownership() {
    let (state, directory) = test_state();
    let session = session(&state);
    let (done_tx, done_rx) = mpsc::channel();
    let worker_state = state.clone();
    let worker_session = session.clone();
    let worker = std::thread::spawn(move || {
        let _activation = worker_state.active_show.try_acquire().unwrap();
        let context = context(&worker_session);
        let result = run_programming_interaction(
            &worker_state,
            &worker_session,
            &context,
            "lifecycle_ordering_test",
            ProgrammingLockPolicy::AllowLockedReconciliation,
            || {
                for site in SITES {
                    lifecycle(&worker_state, &worker_session, site, true, || {
                        let refresh = worker_state
                            .programming
                            .run_selection_refresh_within_interaction(&context, || 548);
                        assert_eq!(refresh.output, 548);
                        assert!(refresh.events.is_empty());
                        Ok(())
                    })
                    .unwrap();
                }
            },
        );
        done_tx.send(result.is_ok()).unwrap();
    });
    assert!(
        done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("nested lifecycle re-locked the desk"),
    );
    worker.join().unwrap();
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn standalone_lifecycle_allows_selection_callback_to_acquire_desk() {
    let (state, directory) = test_state();
    let session = session(&state);
    let (done_tx, done_rx) = mpsc::channel();
    let worker_state = state.clone();
    let worker_session = session.clone();
    let worker = std::thread::spawn(move || {
        let _activation = worker_state.active_show.try_acquire().unwrap();
        lifecycle(
            &worker_state,
            &worker_session,
            Site::ActiveShow,
            false,
            || {
                let refresh = worker_state
                    .programming
                    .run_selection_refresh(&context(&worker_session), || 548);
                assert_eq!(refresh.output, 548);
                assert!(refresh.events.is_empty());
                Ok(())
            },
        )
        .unwrap();
        done_tx.send(()).unwrap();
    });
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("lifecycle retained a desk gate through selection refresh");
    worker.join().unwrap();
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn standalone_update_rejects_activation_before_entering_programmer_boundary() {
    let (state, directory) = test_state();
    let session = session(&state);
    let _activation = state.active_show.acquire_blocking();
    let result: Result<(), ActionError> = lifecycle(&state, &session, Site::Update, false, || {
        panic!("rejected activation entered the show operation")
    });
    assert!(matches!(
        result,
        Err(ActionError {
            kind: light_application::ActionErrorKind::Busy,
            ..
        })
    ));
    drop(_activation);
    let _ = std::fs::remove_dir_all(directory);
}

fn osc_subscriber(state: &AppState, session: &Session, settings: Option<bool>) {
    let source = "127.0.0.1:9054".parse().unwrap();
    state.integrations.register_osc_subscriber(
        "show-lifecycle-ordering".into(),
        OscSubscriber {
            capability: light_core::SurfaceCapability::Programming,
            path: "main".into(),
            target: source,
            command_source: source,
            session_id: session.id,
            last_seen: Instant::now(),
            shifted: settings == Some(false),
            shift_held: settings == Some(true),
            update_record_started: (settings == Some(true))
                .then(|| Instant::now() - Duration::from_secs(3)),
            update_first_release: None,
            last_highlight_action: None,
        },
    );
}

#[test]
fn osc_update_arm_and_settings_wait_before_holding_desk_during_programmer_contention() {
    let (state, directory) = test_state();
    let session = session(&state);
    for settings in [false, true] {
        state
            .programming
            .set_command_line(session.id, "UPDATE".into());
        osc_subscriber(&state, &session, Some(settings));
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker_state = state.clone();
        let (worker, probe, desk_available) = state.programming.programmers().serialized(|| {
            let worker = std::thread::spawn(move || {
                started_tx.send(()).unwrap();
                let handled = handle_programmer_osc(
                    &worker_state,
                    "/light/main/programmer/record",
                    &[OscArgument::Bool(!settings)],
                    Some("127.0.0.1:9054"),
                );
                done_tx.send(handled).unwrap();
            });
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(
                done_rx.recv_timeout(Duration::from_millis(50)),
                Err(mpsc::RecvTimeoutError::Timeout),
            );
            // A waiting OSC mutation must leave the desk available to the Programmer owner.
            // Probe from a separate thread so a regression releases the held Programmer gate
            // on timeout, allowing both workers to finish instead of wedging the test process.
            let probe_state = state.clone();
            let desk_id = session.desk.id;
            let (desk_tx, desk_rx) = mpsc::channel();
            let probe = std::thread::spawn(move || {
                probe_state.programming.run_desk_operation(desk_id, || {
                    desk_tx.send(()).unwrap();
                });
            });
            let desk_available = desk_rx.recv_timeout(Duration::from_secs(2));
            (worker, probe, desk_available)
        });
        assert!(done_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        worker.join().unwrap();
        probe.join().unwrap();
        assert_eq!(
            desk_available,
            Ok(()),
            "OSC settings={settings} held desk while waiting for Programmer"
        );
        assert_eq!(
            state.programming.get(session.id).unwrap().command_line,
            if settings { "" } else { "UPDATE" },
        );
    }
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn osc_record_dispatch_releases_desk_before_entering_programming_service() {
    let (state, directory) = test_state();
    let session = session(&state);
    osc_subscriber(&state, &session, None);
    let worker_state = state.clone();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        for pressed in [true, false] {
            assert!(handle_programmer_osc(
                &worker_state,
                "/light/main/programmer/record",
                &[OscArgument::Bool(pressed)],
                Some("127.0.0.1:9054"),
            ));
        }
        done_tx.send(()).unwrap();
    });
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("routed Record re-entered its held desk gate");
    worker.join().unwrap();
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "RECORD "
    );
    let _ = std::fs::remove_dir_all(directory);
}
