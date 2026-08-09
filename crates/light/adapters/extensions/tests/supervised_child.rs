use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use light_extensions_contract::{
    CanonicalControlIntent, DeviceActionDeclaration, DeviceActionRequest, DeviceActionStatus,
    ExtensionCapability, FeedbackChange, FeedbackDelta, FeedbackSnapshot, FeedbackValue,
    HighlightControlAction, PlaybackControl, TelemetryChannelDeclaration, TelemetryQuality,
    TelemetryValue, TelemetryValueKind,
};
use light_extensions_host::{
    BoundControlInput, DeviceActionEnqueueError, ExtensionApplicationPorts, ExtensionHost,
    ExtensionLimits, ExtensionSpec, ExtensionState, HostControlContext, HostHealth, PortError,
    RunningExtension, TelemetryEnvelope, TimecodeEnvelope,
};

#[derive(Default)]
struct FakePorts {
    revision: AtomicU64,
    snapshots: Mutex<Vec<u64>>,
    controls: Mutex<Vec<(HostControlContext, BoundControlInput)>>,
    telemetry: Mutex<Vec<TelemetryEnvelope>>,
    timecode: Mutex<Vec<TimecodeEnvelope>>,
}

impl FakePorts {
    fn at_revision(revision: u64) -> Arc<Self> {
        Arc::new(Self {
            revision: AtomicU64::new(revision),
            ..Self::default()
        })
    }

    fn set_revision(&self, revision: u64) {
        self.revision.store(revision, Ordering::Release);
    }
}

impl ExtensionApplicationPorts for FakePorts {
    fn feedback_snapshot(
        &self,
        _context: &HostControlContext,
        _bindings: &BTreeMap<String, CanonicalControlIntent>,
    ) -> FeedbackSnapshot {
        let revision = self.revision.load(Ordering::Acquire);
        self.snapshots
            .lock()
            .expect("snapshots mutex")
            .push(revision);
        FeedbackSnapshot {
            context: feedback_context(),
            revision,
            controls: BTreeMap::from([("go".into(), FeedbackValue::Boolean(false))]),
        }
    }

    fn apply_control(
        &self,
        context: &HostControlContext,
        input: BoundControlInput,
    ) -> Result<Option<FeedbackDelta>, PortError> {
        self.controls
            .lock()
            .expect("controls mutex")
            .push((context.clone(), input.clone()));
        let base_revision = self.revision.fetch_add(1, Ordering::AcqRel);
        Ok(Some(FeedbackDelta {
            context: feedback_context(),
            base_revision,
            revision: base_revision + 1,
            changes: vec![FeedbackChange {
                control_id: "go".into(),
                value: Some(FeedbackValue::Boolean(input.input.input_id == 1)),
            }],
        }))
    }

    fn publish_telemetry(&self, telemetry: TelemetryEnvelope) -> Result<(), PortError> {
        self.telemetry
            .lock()
            .expect("telemetry mutex")
            .push(telemetry);
        Ok(())
    }

    fn publish_timecode(&self, timecode: TimecodeEnvelope) -> Result<(), PortError> {
        self.timecode.lock().expect("timecode mutex").push(timecode);
        Ok(())
    }
}

fn child_program() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tl-extension-conformance-child"))
}

fn spec(mode: &str) -> ExtensionSpec {
    ExtensionSpec {
        program: child_program(),
        extension_id: "test.surface".into(),
        extension_instance_id: "test-instance".into(),
        extension_version: "0.0.0-test".into(),
        approved_package_digest: "sha256:approved".into(),
        desk_id: "desk-main".into(),
        channel_credential: "private-channel-token".into(),
        requested_capabilities: BTreeSet::from([
            ExtensionCapability::ControlSurface,
            ExtensionCapability::TelemetrySource,
            ExtensionCapability::TimecodeSource,
        ]),
        feedback_features: BTreeSet::from([
            light_extensions_host::FeedbackFeature::Lamp,
            light_extensions_host::FeedbackFeature::Blink,
            light_extensions_host::FeedbackFeature::SemanticColor,
            light_extensions_host::FeedbackFeature::RgbColor,
        ]),
        telemetry_channels: vec![TelemetryChannelDeclaration {
            channel_id: "temperature".into(),
            label: "Temperature".into(),
            quantity: "temperature".into(),
            unit: "degC".into(),
            value_kind: TelemetryValueKind::Number,
            minimum: Some(-40.0),
            maximum: Some(125.0),
            precision: Some(1),
            expected_interval_micros: Some(1_000),
            quality_flags: BTreeSet::from([TelemetryQuality::Good, TelemetryQuality::Stale]),
        }],
        maximum_telemetry_rate_hz: 100,
        device_actions: vec![DeviceActionDeclaration {
            action_id: "identify".into(),
            label: "Identify device".into(),
            required_permission: "device.identify".into(),
            parameters: BTreeMap::from([("seconds".into(), TelemetryValueKind::Integer)]),
            result_values: BTreeMap::from([("accepted".into(), TelemetryValueKind::Boolean)]),
        }],
        control_bindings: BTreeMap::from([(
            "go".into(),
            CanonicalControlIntent::PlaybackCurrent {
                slot: 1,
                control: PlaybackControl::ButtonOne,
            },
        )]),
        settings: BTreeMap::new(),
        environment: BTreeMap::from([("TL_EXTENSION_TEST_MODE".into(), mode.into())]),
    }
}

fn limits() -> ExtensionLimits {
    ExtensionLimits {
        // Process creation can be noticeably slower when the Rust test harness runs cases in
        // parallel. The deadline remains short and explicit without making cold CI startup flaky.
        handshake_timeout: Duration::from_secs(5),
        shutdown_timeout: Duration::from_millis(80),
        first_restart_backoff: Duration::from_millis(20),
        maximum_restart_backoff: Duration::from_millis(50),
        maximum_failures: 1,
        command_queue: 8,
        inbound_queue: 32,
        wire_queue: 4,
        stderr_queue: 8,
        log_bytes: 512,
        log_lines: 8,
        telemetry_history_samples: 2,
    }
}

fn start(mode: &str, ports: Arc<FakePorts>, limits: ExtensionLimits) -> RunningExtension {
    ExtensionHost::new(spec(mode), limits, ports).start()
}

fn wait_for(
    extension: &RunningExtension,
    timeout: Duration,
    predicate: impl Fn(&HostHealth) -> bool,
) -> HostHealth {
    // macOS can spend tens of seconds validating a freshly linked test child before its first
    // instruction runs. Keep the protocol's own handshake deadline exact, while allowing the
    // outer test observer enough time to see that terminal or running state under a cold launch.
    let observation_timeout = timeout.max(Duration::from_secs(30));
    let deadline = Instant::now() + observation_timeout;
    loop {
        let health = extension.health();
        if predicate(&health) {
            return health;
        }
        assert!(
            Instant::now() < deadline,
            "condition was not reached; last health: {health:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn handshake_has_a_deadline_and_requires_exact_authenticated_identity() {
    for mode in [
        "silent",
        "bad_version",
        "bad_identity",
        "bad_instance",
        "bad_digest",
        "bad_auth",
        "bad_capabilities",
    ] {
        let ports = FakePorts::at_revision(7);
        let mut case_limits = limits();
        case_limits.handshake_timeout = Duration::from_millis(750);
        let mut extension = start(mode, Arc::clone(&ports), case_limits);
        let health = wait_for(&extension, Duration::from_secs(3), |health| {
            matches!(health.state, ExtensionState::Terminal { .. })
        });
        assert_eq!(health.launches, 1, "{mode}: {health:?}");
        assert_eq!(health.protocol_errors, 1, "{mode}: {health:?}");
        assert!(
            ports.controls.lock().expect("controls mutex").is_empty(),
            "{mode} reached the application"
        );
        extension.stop();
    }
}

#[test]
fn full_snapshot_precedes_deltas_and_typed_control_round_trips() {
    let ports = FakePorts::at_revision(7);
    let mut extension = start("normal", Arc::clone(&ports), limits());
    wait_for(&extension, Duration::from_secs(2), |_| {
        ports.controls.lock().expect("controls mutex").len() == 2
    });
    let controls = ports.controls.lock().expect("controls mutex").clone();
    assert_eq!(controls[0].0.extension_id, "test.surface");
    assert_eq!(controls[0].0.extension_instance_id, "test-instance");
    assert_eq!(controls[0].0.desk_id, "desk-main");
    assert_eq!(controls[0].0.source, "native_extension");
    assert_eq!(controls[0].1.input.input_id, 1);
    assert_eq!(controls[1].1.input.input_id, 2);
    assert_eq!(
        controls[0].1.intent,
        CanonicalControlIntent::PlaybackCurrent {
            slot: 1,
            control: PlaybackControl::ButtonOne,
        }
    );
    assert_eq!(
        ports.snapshots.lock().expect("snapshots mutex").as_slice(),
        &[7],
        "one authoritative full snapshot was obtained before inputs"
    );
    let health = wait_for(&extension, Duration::from_secs(2), |health| {
        health
            .extension_health
            .as_ref()
            .and_then(|report| report.counters.get("delta_revision"))
            .is_some()
    });
    assert!(matches!(health.state, ExtensionState::Running));
    extension.stop();
}

#[test]
fn canonical_highlight_action_survives_the_supervised_binding() {
    let ports = FakePorts::at_revision(7);
    let mut highlight_spec = spec("normal");
    highlight_spec.control_bindings.insert(
        "go".into(),
        CanonicalControlIntent::Highlight {
            action: HighlightControlAction::Next,
        },
    );
    let mut extension = ExtensionHost::new(highlight_spec, limits(), Arc::clone(&ports)).start();
    wait_for(&extension, Duration::from_secs(5), |_| {
        ports.controls.lock().expect("controls mutex").len() == 2
    });
    let controls = ports.controls.lock().expect("controls mutex").clone();
    assert!(controls.iter().all(|(_, input)| {
        input.intent
            == CanonicalControlIntent::Highlight {
                action: HighlightControlAction::Next,
            }
    }));
    extension.stop();
}

#[test]
fn button_press_and_release_order_is_enforced_per_connection() {
    for (mode, expected) in [
        ("duplicate_press", "pressed while already held"),
        (
            "release_without_press",
            "released without a preceding press",
        ),
    ] {
        let ports = FakePorts::at_revision(7);
        let mut extension = start(mode, Arc::clone(&ports), limits());
        let health = wait_for(&extension, Duration::from_secs(2), |health| {
            matches!(health.state, ExtensionState::Terminal { .. })
        });
        assert_eq!(health.protocol_errors, 1, "{mode}: {health:?}");
        assert!(
            health
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains(expected)),
            "{mode}: {health:?}"
        );
        extension.stop();
    }
}

#[test]
fn typed_declared_telemetry_stream_reaches_the_fake_host() {
    let ports = FakePorts::at_revision(7);
    let mut extension = start("normal", Arc::clone(&ports), limits());
    wait_for(&extension, Duration::from_secs(2), |_| {
        ports.telemetry.lock().expect("telemetry mutex").len() == 2
    });
    let telemetry = ports.telemetry.lock().expect("telemetry mutex").clone();
    assert_eq!(telemetry[0].extension_id, "test.surface");
    assert_eq!(telemetry[0].sample.channel_id, "temperature");
    assert_eq!(telemetry[0].sample.quality, TelemetryQuality::Good);
    assert_eq!(telemetry[0].sample.value, TelemetryValue::Number(21.0));
    assert_eq!(telemetry[1].sample.sample_id, 2);
    extension.stop();

    let rejected_ports = FakePorts::at_revision(7);
    let mut rejected = start(
        "undeclared_telemetry",
        Arc::clone(&rejected_ports),
        limits(),
    );
    let health = wait_for(&rejected, Duration::from_secs(2), |health| {
        matches!(health.state, ExtensionState::Terminal { .. })
    });
    assert_eq!(health.protocol_errors, 1);
    assert!(
        rejected_ports
            .telemetry
            .lock()
            .expect("telemetry mutex")
            .is_empty()
    );
    rejected.stop();
}

#[test]
fn telemetry_health_is_bounded_and_reports_loss_invalid_stale_and_excess_rate() {
    let ports = FakePorts::at_revision(7);
    let mut extension = start("telemetry_health", Arc::clone(&ports), limits());
    let health = wait_for(&extension, Duration::from_secs(2), |health| {
        health
            .telemetry
            .get("temperature")
            .is_some_and(|channel| channel.accepted_samples == 4)
    });
    let channel = &health.telemetry["temperature"];
    assert_eq!(channel.lost_samples, 1);
    assert_eq!(channel.invalid_samples, 2);
    assert_eq!(channel.stale_samples, 1);
    assert!(channel.excess_rate_samples >= 1);
    assert_eq!(channel.history.len(), 2);
    assert_eq!(channel.history[0].sample.sample_id, 4);
    assert_eq!(channel.history[1].sample.sample_id, 6);
    assert!(channel.latest.as_ref().unwrap().received_at_micros > 0);
    assert_eq!(ports.telemetry.lock().expect("telemetry mutex").len(), 4);
    std::thread::sleep(Duration::from_millis(5));
    assert!(extension.health().telemetry["temperature"].stale);
    extension.stop();
}

#[test]
fn typed_timecode_validates_frames_and_monotonic_sequence_before_publication() {
    let ports = FakePorts::at_revision(7);
    let mut extension = start("timecode", Arc::clone(&ports), limits());
    wait_for(&extension, Duration::from_secs(2), |_| {
        ports.timecode.lock().expect("timecode mutex").len() == 1
    });
    let sample = ports.timecode.lock().expect("timecode mutex")[0].clone();
    assert_eq!(sample.extension_id, "test.surface");
    assert!(sample.received_at_micros > 0);
    assert_eq!(sample.sample.frames, 12);
    extension.stop();

    for mode in ["invalid_timecode", "duplicate_timecode"] {
        let ports = FakePorts::at_revision(7);
        let mut extension = start(mode, Arc::clone(&ports), limits());
        let health = wait_for(&extension, Duration::from_secs(2), |health| {
            matches!(health.state, ExtensionState::Terminal { .. })
        });
        assert_eq!(health.protocol_errors, 1, "{mode}: {health:?}");
        assert!(ports.timecode.lock().expect("timecode mutex").len() <= 1);
        extension.stop();
    }
}

#[test]
fn declared_and_permitted_device_action_round_trips_on_the_bounded_command_channel() {
    let ports = FakePorts::at_revision(7);
    let mut extension = start("device_action", ports, limits());
    wait_for(&extension, Duration::from_secs(6), |health| {
        matches!(health.state, ExtensionState::Running)
    });
    assert_eq!(
        extension.device_action(DeviceActionRequest {
            request_id: 1,
            action_id: "not-permitted".into(),
            parameters: BTreeMap::new(),
        }),
        Err(DeviceActionEnqueueError::UndeclaredOrDenied)
    );
    extension
        .device_action(DeviceActionRequest {
            request_id: 2,
            action_id: "identify".into(),
            parameters: BTreeMap::from([("seconds".into(), TelemetryValue::Integer(2))]),
        })
        .unwrap();
    let health = wait_for(&extension, Duration::from_secs(2), |health| {
        health.device_action_results.len() == 1
    });
    let result = &health.device_action_results[0];
    assert_eq!(result.request_id, 2);
    assert_eq!(result.status, DeviceActionStatus::Completed);
    assert_eq!(result.values["accepted"], TelemetryValue::Boolean(true));
    extension.stop();

    let ports = FakePorts::at_revision(7);
    let mut rejected = start("bad_device_action_result", ports, limits());
    wait_for(&rejected, Duration::from_secs(6), |health| {
        matches!(health.state, ExtensionState::Running)
    });
    rejected
        .device_action(DeviceActionRequest {
            request_id: 3,
            action_id: "identify".into(),
            parameters: BTreeMap::from([("seconds".into(), TelemetryValue::Integer(2))]),
        })
        .unwrap();
    let health = wait_for(&rejected, Duration::from_secs(2), |health| {
        matches!(health.state, ExtensionState::Terminal { .. })
    });
    assert_eq!(health.protocol_errors, 1);
    assert!(health.device_action_results.is_empty());
    rejected.stop();
}

#[test]
fn malformed_and_oversized_frames_fault_only_the_child_session() {
    for mode in ["malformed_after_config", "oversized_after_config"] {
        let ports = FakePorts::at_revision(7);
        let heartbeat = Arc::new(AtomicU64::new(0));
        let heartbeat_thread = {
            let heartbeat = Arc::clone(&heartbeat);
            std::thread::spawn(move || {
                for _ in 0..20 {
                    heartbeat.fetch_add(1, Ordering::Relaxed);
                    std::thread::sleep(Duration::from_millis(2));
                }
            })
        };
        let mut extension = start(mode, Arc::clone(&ports), limits());
        let health = wait_for(&extension, Duration::from_secs(2), |health| {
            matches!(health.state, ExtensionState::Terminal { .. })
        });
        heartbeat_thread.join().expect("host heartbeat thread");
        assert_eq!(health.protocol_errors, 1, "{mode}: {health:?}");
        assert_eq!(heartbeat.load(Ordering::Relaxed), 20);
        assert!(ports.controls.lock().expect("controls mutex").is_empty());
        extension.stop();
    }
}

#[test]
fn crash_restarts_with_backoff_and_recovers_from_a_new_snapshot() {
    let ports = FakePorts::at_revision(7);
    let mut retrying_limits = limits();
    retrying_limits.maximum_failures = 3;
    let mut extension = start("crash_once", Arc::clone(&ports), retrying_limits);
    let restarting = wait_for(&extension, Duration::from_secs(2), |health| {
        matches!(health.state, ExtensionState::Restarting { failures: 1, .. })
    });
    assert_eq!(restarting.crashes, 1);
    ports.set_revision(12);
    let health = wait_for(&extension, Duration::from_secs(3), |health| {
        health.launches >= 2
            && matches!(health.state, ExtensionState::Running)
            && ports.controls.lock().expect("controls mutex").len() == 2
    });
    assert_eq!(health.launches, 2);
    assert_eq!(
        ports.snapshots.lock().expect("snapshots mutex").as_slice(),
        &[7, 12],
        "restart requests a fresh full snapshot rather than replaying guessed deltas"
    );
    extension.stop();
}

#[test]
fn repeated_crashes_reach_terminal_health_until_explicit_restart() {
    let ports = FakePorts::at_revision(7);
    let mut retrying_limits = limits();
    retrying_limits.maximum_failures = 2;
    let mut extension = start("always_crash", ports, retrying_limits);
    let terminal = wait_for(&extension, Duration::from_secs(2), |health| {
        matches!(health.state, ExtensionState::Terminal { failures: 2 })
    });
    assert_eq!(terminal.launches, 2);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        extension.health().launches,
        2,
        "terminal state does not spin"
    );
    extension.restart().expect("operator restart request");
    let relaunched = wait_for(&extension, Duration::from_secs(2), |health| {
        health.launches > 2
    });
    assert!(relaunched.launches > 2);
    extension.stop();
}

#[test]
fn queues_and_logs_remain_bounded_and_overflow_faults_instead_of_guessing() {
    let ports = FakePorts::at_revision(7);
    let mut tiny = limits();
    tiny.inbound_queue = 2;
    tiny.stderr_queue = 2;
    tiny.log_bytes = 160;
    tiny.log_lines = 2;
    let mut flooded = start("flood", ports, tiny);
    let health = wait_for(&flooded, Duration::from_secs(3), |health| {
        matches!(health.state, ExtensionState::Terminal { .. })
    });
    assert!(health.inbound_drops > 0, "{health:?}");
    assert_eq!(health.protocol_errors, 1, "{health:?}");
    assert!(health.log_bytes <= 160, "{health:?}");
    assert!(health.log_lines.len() <= 2, "{health:?}");
    assert!(health.log_drops > 0, "{health:?}");
    flooded.stop();

    let repair_ports = FakePorts::at_revision(7);
    let mut outbound_limits = limits();
    outbound_limits.command_queue = 2;
    outbound_limits.wire_queue = 1;
    let mut stalled = start(
        "stall_after_config",
        Arc::clone(&repair_ports),
        outbound_limits,
    );
    wait_for(&stalled, Duration::from_secs(2), |health| {
        matches!(health.state, ExtensionState::Running)
    });
    repair_ports.set_revision(50);
    let mut rejected = false;
    for revision in 8..1_000_u64 {
        let result = stalled.feedback_delta(FeedbackDelta {
            context: feedback_context(),
            base_revision: revision - 1,
            revision,
            changes: vec![FeedbackChange {
                control_id: "large".into(),
                value: Some(FeedbackValue::Text("x".repeat(128 * 1024))),
            }],
        });
        rejected |= result.is_err();
        if rejected {
            break;
        }
    }
    assert!(rejected, "bounded queue must reject pressure synchronously");
    let repaired = wait_for(&stalled, Duration::from_secs(4), |health| {
        health.outbound_drops > 0
            && health
                .extension_health
                .as_ref()
                .and_then(|report| report.counters.get("snapshot_revision"))
                == Some(&50)
    });
    assert!(matches!(repaired.state, ExtensionState::Running));
    stalled.stop();
}

fn feedback_context() -> light_extensions_contract::FeedbackContext {
    light_extensions_contract::FeedbackContext {
        desk_id: "desk-main".into(),
        show_id: Some("show-main".into()),
        show_generation: 1,
    }
}

#[test]
fn shutdown_is_idempotent_and_kills_a_hung_child_after_the_deadline() {
    let ports = FakePorts::at_revision(7);
    let mut graceful = start("normal", Arc::clone(&ports), limits());
    wait_for(&graceful, Duration::from_secs(2), |health| {
        matches!(health.state, ExtensionState::Running)
    });
    graceful.stop();
    graceful.stop();
    assert!(matches!(graceful.health().state, ExtensionState::Stopped));

    let mut hung = start("hang_shutdown", ports, limits());
    wait_for(&hung, Duration::from_secs(2), |health| {
        matches!(health.state, ExtensionState::Running)
    });
    let started = Instant::now();
    hung.stop();
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(matches!(hung.health().state, ExtensionState::Stopped));
}
