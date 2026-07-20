use super::*;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    CueListRuntimeProjection, CueNumber, ManualXFadeDirection, PlaybackCueReference,
    PlaybackRuntimeIdentity, PlaybackRuntimeProjection, PlaybackShowScope,
    PlaybackTargetProjection,
};
use chrono::Utc;
use light_core::{AttributeKey, AttributeValue, CueListId, FixtureId, SessionId, ShowId, UserId};
use light_programmer::{CueRecordingCapturedSource, ProgrammerRegistry};
use light_show::PortableShowRevision;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

#[test]
fn replay_skips_environment_capture_commit_activation_and_active_preload_release() {
    let setup = Setup::normal();
    let ports = TestPorts::changed(setup.show_id, 7);
    let envelope = setup.envelope("cue-replay", ProgrammingCueActivationPolicy::GoToIfNormal);

    let first = setup
        .service
        .handle_cue_recording(envelope.clone(), &ports)
        .unwrap();
    let replay = setup
        .service
        .handle_cue_recording(envelope, &ports)
        .unwrap();

    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(ports.environments.load(Ordering::Relaxed), 1);
    assert_eq!(ports.commits.load(Ordering::Relaxed), 1);
    assert_eq!(ports.activations.load(Ordering::Relaxed), 1);
    assert!(matches!(
        replay.outcome,
        ProgrammingCueRecordOutcome::Changed {
            runtime: Some(_),
            ..
        }
    ));
}

#[test]
fn already_authoritative_take_live_succeeds_without_a_runtime_event_and_replays() {
    let setup = Setup::normal();
    let ports = TestPorts::without_activation_event(setup.show_id, 7);
    let envelope = setup.envelope(
        "cue-no-runtime-event",
        ProgrammingCueActivationPolicy::GoToIfNormal,
    );

    let result = setup
        .service
        .handle_cue_recording(envelope.clone(), &ports)
        .unwrap();
    assert!(matches!(
        result.outcome,
        ProgrammingCueRecordOutcome::Changed { runtime: None, .. }
    ));
    assert_eq!(ports.activations.load(Ordering::Relaxed), 1);

    let replay = setup
        .service
        .handle_cue_recording(envelope, &ports)
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(ports.activations.load(Ordering::Relaxed), 1);
}

#[test]
fn active_preload_fallback_releases_only_after_an_accepted_commit_and_never_activates() {
    let setup = Setup::active_preload();
    let ports = TestPorts::changed(setup.show_id, 7);
    let mut envelope = setup.envelope(
        "preload-record",
        ProgrammingCueActivationPolicy::GoToIfNormal,
    );
    envelope.command.capture_policy = ProgrammingCueCapturePolicy::PendingOrActivePreload;

    let result = setup
        .service
        .handle_cue_recording(envelope.clone(), &ports)
        .unwrap();
    assert_eq!(
        result.captured_source,
        CueRecordingCapturedSource::ActivePreload
    );
    assert_eq!(ports.activations.load(Ordering::Relaxed), 0);
    assert!(
        setup
            .registry
            .get(setup.session)
            .unwrap()
            .preload_active
            .is_empty()
    );

    let replay = setup
        .service
        .handle_cue_recording(envelope, &ports)
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(ports.commits.load(Ordering::Relaxed), 1);
    assert_eq!(ports.environments.load(Ordering::Relaxed), 1);
}

#[test]
fn active_preload_fallback_is_also_released_after_an_accepted_no_change() {
    let setup = Setup::active_preload();
    let ports = TestPorts::no_change(setup.show_id, 7);
    let mut envelope = setup.envelope(
        "preload-no-change",
        ProgrammingCueActivationPolicy::GoToIfNormal,
    );
    envelope.command.capture_policy = ProgrammingCueCapturePolicy::PendingOrActivePreload;

    let result = setup
        .service
        .handle_cue_recording(envelope, &ports)
        .unwrap();

    assert!(matches!(
        result.outcome,
        ProgrammingCueRecordOutcome::NoChange { .. }
    ));
    assert_eq!(
        result.captured_source,
        CueRecordingCapturedSource::ActivePreload
    );
    assert!(
        setup
            .registry
            .get(setup.session)
            .unwrap()
            .preload_active
            .is_empty()
    );
    assert_eq!(ports.activations.load(Ordering::Relaxed), 0);
}

#[test]
fn failed_commit_preserves_active_preload_and_is_not_cached() {
    let setup = Setup::active_preload();
    let ports = TestPorts::failed(setup.show_id);
    let mut envelope = setup.envelope("preload-fail", ProgrammingCueActivationPolicy::GoToIfNormal);
    envelope.command.capture_policy = ProgrammingCueCapturePolicy::PendingOrActivePreload;

    for _ in 0..2 {
        let error = setup
            .service
            .handle_cue_recording(envelope.clone(), &ports)
            .unwrap_err();
        assert_eq!(error.kind, ActionErrorKind::Conflict);
        assert!(
            !setup
                .registry
                .get(setup.session)
                .unwrap()
                .preload_active
                .is_empty()
        );
    }
    assert_eq!(ports.commits.load(Ordering::Relaxed), 2);
    assert_eq!(ports.activations.load(Ordering::Relaxed), 0);
}

#[test]
fn no_change_and_pending_preload_never_activate() {
    let normal = Setup::normal();
    let no_change = TestPorts::no_change(normal.show_id, 7);
    let result = normal
        .service
        .handle_cue_recording(
            normal.envelope("no-change", ProgrammingCueActivationPolicy::GoToIfNormal),
            &no_change,
        )
        .unwrap();
    assert!(matches!(
        result.outcome,
        ProgrammingCueRecordOutcome::NoChange { .. }
    ));
    assert_eq!(no_change.activations.load(Ordering::Relaxed), 0);

    let pending = Setup::pending_preload();
    let ports = TestPorts::changed(pending.show_id, 7);
    let result = pending
        .service
        .handle_cue_recording(
            pending.envelope("pending", ProgrammingCueActivationPolicy::GoToIfNormal),
            &ports,
        )
        .unwrap();
    assert_eq!(
        result.captured_source,
        CueRecordingCapturedSource::PendingPreload
    );
    assert_eq!(ports.activations.load(Ordering::Relaxed), 0);
}

#[test]
fn cue_timing_must_remain_exact_across_the_javascript_wire_boundary() {
    let setup = Setup::normal();
    let ports = TestPorts::changed(setup.show_id, 7);
    let mut envelope = setup.envelope("unsafe-timing", ProgrammingCueActivationPolicy::Hold);
    envelope.command.timing.fade_millis = Some(9_007_199_254_740_992);

    let error = setup
        .service
        .handle_cue_recording(envelope, &ports)
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(ports.environments.load(Ordering::Relaxed), 0);
}

#[test]
fn cue_name_uses_the_same_printable_byte_contract_as_the_client() {
    let setup = Setup::normal();
    let ports = TestPorts::changed(setup.show_id, 7);
    let mut empty = setup.envelope("empty-name", ProgrammingCueActivationPolicy::Hold);
    empty.command.name = Some("  ".into());
    assert_eq!(
        setup
            .service
            .handle_cue_recording(empty, &ports)
            .unwrap_err()
            .kind,
        ActionErrorKind::Invalid
    );

    let mut exact = setup.envelope("utf8-name", ProgrammingCueActivationPolicy::Hold);
    exact.command.name = Some("Ä".repeat(128));
    setup.service.handle_cue_recording(exact, &ports).unwrap();
    assert_eq!(ports.environments.load(Ordering::Relaxed), 1);
}

#[test]
fn deleting_a_cue_never_attempts_to_activate_the_removed_cue() {
    let setup = Setup::normal();
    let ports = TestPorts::deleted(setup.show_id, 7);

    let result = setup
        .service
        .handle_cue_recording(
            setup.envelope("delete-cue", ProgrammingCueActivationPolicy::GoToIfNormal),
            &ports,
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        ProgrammingCueRecordOutcome::Changed {
            recorded_cue: ProgrammingRecordedCue { deleted: true, .. },
            runtime: None,
            ..
        }
    ));
    assert_eq!(ports.activations.load(Ordering::Relaxed), 0);
}

#[test]
fn replay_scope_includes_user_desk_and_session_and_foreign_ownership_is_rejected() {
    let setup = Setup::normal();
    let ports = TestPorts::changed(setup.show_id, 7);
    let first = setup.envelope("shared-id", ProgrammingCueActivationPolicy::Hold);
    setup
        .service
        .handle_cue_recording(first.clone(), &ports)
        .unwrap();

    let mut peer_desk = first.clone();
    peer_desk.context.desk_id = Uuid::new_v4();
    setup
        .service
        .handle_cue_recording(peer_desk, &ports)
        .unwrap();
    assert_eq!(ports.commits.load(Ordering::Relaxed), 2);

    let peer_session = SessionId::new();
    setup.registry.start(peer_session, setup.user);
    let mut peer_session_action = first.clone();
    peer_session_action.context.session_id = Some(peer_session.0);
    peer_session_action.context.desk_id = Uuid::new_v4();
    setup
        .service
        .handle_cue_recording(peer_session_action, &ports)
        .unwrap();
    assert_eq!(ports.commits.load(Ordering::Relaxed), 3);

    let other_user = UserId::new();
    let other_session = SessionId::new();
    setup.registry.start(other_session, other_user);
    setup.registry.set(
        other_session,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.7),
    );
    let mut other_user_action = first.clone();
    other_user_action.context.user_id = Some(other_user.0);
    other_user_action.context.session_id = Some(other_session.0);
    other_user_action.context.desk_id = Uuid::new_v4();
    setup
        .service
        .handle_cue_recording(other_user_action, &ports)
        .unwrap();
    assert_eq!(ports.commits.load(Ordering::Relaxed), 4);

    let mut foreign = first;
    foreign.context.user_id = Some(UserId::new().0);
    foreign.context.request_id = Some("foreign".into());
    let error = setup
        .service
        .handle_cue_recording(foreign, &ports)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Forbidden);
    assert_eq!(ports.environments.load(Ordering::Relaxed), 4);
}

#[test]
fn within_interaction_bridge_does_not_reenter_programming_gates() {
    let setup = Setup::normal();
    let ports = TestPorts::changed(setup.show_id, 7);
    let result = setup
        .service
        .record_cue_within_interaction(
            setup.envelope("nested", ProgrammingCueActivationPolicy::Hold),
            &ports,
        )
        .unwrap();
    assert!(!result.replayed);
    assert_eq!(ports.commits.load(Ordering::Relaxed), 1);
}

struct Setup {
    registry: ProgrammerRegistry,
    service: ProgrammingService,
    session: SessionId,
    user: UserId,
    desk: Uuid,
    show_id: ShowId,
}

impl Setup {
    fn normal() -> Self {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let user = UserId::new();
        registry.start(session, user);
        registry.set(
            session,
            FixtureId::new(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        Self::from_registry(registry, session, user)
    }

    fn pending_preload() -> Self {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let user = UserId::new();
        registry.start(session, user);
        registry.arm_preload(session, true);
        registry.set(
            session,
            FixtureId::new(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.6),
        );
        Self::from_registry(registry, session, user)
    }

    fn active_preload() -> Self {
        let setup = Self::pending_preload();
        setup.registry.activate_preload(setup.session);
        setup
    }

    fn from_registry(registry: ProgrammerRegistry, session: SessionId, user: UserId) -> Self {
        Self {
            service: ProgrammingService::new(
                registry.clone(),
                crate::EventBus::default(),
                Arc::new(light_programmer::HighlightRegistry::default()),
            ),
            registry,
            session,
            user,
            desk: Uuid::new_v4(),
            show_id: ShowId::new(),
        }
    }

    fn envelope(
        &self,
        request_id: &str,
        activation_policy: ProgrammingCueActivationPolicy,
    ) -> ActionEnvelope<ProgrammingCueRecordRequest> {
        ActionEnvelope {
            context: ActionContext::operator(
                self.desk,
                self.user.0,
                self.session.0,
                ActionSource::UserInterface,
            )
            .with_request_id(request_id),
            command: ProgrammingCueRecordRequest {
                show_id: self.show_id,
                target: ProgrammingCueRecordTarget::Pool { playback_number: 7 },
                operation: ProgrammingCueRecordOperation::Overwrite,
                cue_number: Some(CueNumber::new(1.0)),
                timing: ProgrammingCueRecordTiming::default(),
                cue_only: false,
                name: None,
                capture_policy: ProgrammingCueCapturePolicy::CurrentCapture,
                activation_policy,
                expected_show_revision: ProgrammingCueShowRevisionExpectation::Current,
            },
        }
    }
}

struct TestPorts {
    show_id: ShowId,
    playback: u16,
    changed: bool,
    fail: bool,
    deleted: bool,
    activation_event: bool,
    cue_id: Uuid,
    environments: AtomicUsize,
    commits: AtomicUsize,
    activations: AtomicUsize,
    last_context: Mutex<Option<ActionContext>>,
}

impl TestPorts {
    fn changed(show_id: ShowId, playback: u16) -> Self {
        Self::new(show_id, playback, true, false, false, true)
    }

    fn no_change(show_id: ShowId, playback: u16) -> Self {
        Self::new(show_id, playback, false, false, false, true)
    }

    fn failed(show_id: ShowId) -> Self {
        Self::new(show_id, 7, false, true, false, true)
    }

    fn deleted(show_id: ShowId, playback: u16) -> Self {
        Self::new(show_id, playback, true, false, true, true)
    }

    fn without_activation_event(show_id: ShowId, playback: u16) -> Self {
        Self::new(show_id, playback, true, false, false, false)
    }

    fn new(
        show_id: ShowId,
        playback: u16,
        changed: bool,
        fail: bool,
        deleted: bool,
        activation_event: bool,
    ) -> Self {
        Self {
            show_id,
            playback,
            changed,
            fail,
            deleted,
            activation_event,
            cue_id: Uuid::new_v4(),
            environments: AtomicUsize::new(0),
            commits: AtomicUsize::new(0),
            activations: AtomicUsize::new(0),
            last_context: Mutex::new(None),
        }
    }

    fn completion(&self) -> ProgrammingCueCommitResult {
        let cue_list_id = CueListId::new();
        let mut cue = light_playback::Cue::new(if self.deleted { 0.5 } else { 1.0 });
        if !self.deleted {
            cue.id = self.cue_id;
        }
        let cue_list = light_playback::CueList {
            id: cue_list_id,
            name: "Test".into(),
            priority: 0,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![cue],
        };
        let playback =
            light_playback::PlaybackDefinition::new_cue_list(self.playback, "Test", cue_list_id);
        ProgrammingCueCommitResult {
            changed: self.changed,
            projections: ProgrammingCueProjections {
                show_id: self.show_id,
                cue_list: ProgrammingCueObjectProjection {
                    kind: crate::ActiveShowObjectKind::CueList,
                    object_id: cue_list_id.0.to_string(),
                    object_revision: if self.changed { 2 } else { 1 },
                    raw_body: Arc::new(serde_json::to_value(cue_list).unwrap()),
                },
                playback: Some(ProgrammingCueObjectProjection {
                    kind: crate::ActiveShowObjectKind::Playback,
                    object_id: self.playback.to_string(),
                    object_revision: 1,
                    raw_body: Arc::new(serde_json::to_value(playback).unwrap()),
                }),
                page: None,
            },
            recorded_cue: ProgrammingRecordedCue {
                id: self.cue_id,
                number: CueNumber::new(1.0),
                deleted: self.deleted,
            },
            show_revision: PortableShowRevision::from_value(if self.changed { 2 } else { 1 }),
            event_sequence: self.changed.then_some(41),
            concrete_playback_number: Some(self.playback),
        }
    }
}

impl ProgrammingCueRecordingPorts for TestPorts {
    fn authorize_cue_recording(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn cue_recording_environment(
        &self,
        context: &ActionContext,
        _request: &ProgrammingCueRecordRequest,
    ) -> Result<ProgrammingCueRecordingEnvironment, ActionError> {
        self.environments.fetch_add(1, Ordering::Relaxed);
        *self.last_context.lock() = Some(context.clone());
        Ok(ProgrammingCueRecordingEnvironment {
            target: ProgrammingCueResolvedTarget::Playback {
                playback_number: self.playback,
                page_slot: None,
            },
            active_cue: None,
        })
    }

    fn commit_cue(
        &self,
        _context: &ActionContext,
        _commit: &ProgrammingCueCommit,
    ) -> Result<ProgrammingCueCommitResult, ActionError> {
        self.commits.fetch_add(1, Ordering::Relaxed);
        if self.fail {
            Err(ActionError::new(ActionErrorKind::Conflict, "stale show"))
        } else {
            Ok(self.completion())
        }
    }

    fn activate_recorded_cue(
        &self,
        _context: &ActionContext,
        playback_number: u16,
        _cue_number: CueNumber,
    ) -> Option<ProgrammingCueActivationCompletion> {
        self.activations.fetch_add(1, Ordering::Relaxed);
        Some(ProgrammingCueActivationCompletion {
            projection: runtime_projection(self.show_id, playback_number, self.cue_id),
            event_sequence: self.activation_event.then_some(42),
        })
    }
}

fn runtime_projection(show_id: ShowId, playback: u16, cue_id: Uuid) -> PlaybackRuntimeProjection {
    PlaybackRuntimeProjection {
        scope: PlaybackShowScope {
            show_id: show_id.0,
            show_revision: 2,
        },
        requested: PlaybackRuntimeIdentity::Playback(playback),
        playback_number: Some(playback),
        target: PlaybackTargetProjection::CueList {
            cue_list_id: CueListId::new(),
            runtime: Some(Box::new(CueListRuntimeProjection {
                cue_index: 0,
                previous_index: None,
                current: Some(PlaybackCueReference {
                    id: cue_id,
                    number: 1.0,
                }),
                loaded: None,
                normal_next: None,
                effective_next: None,
                effective_next_is_loaded: false,
                paused: false,
                activated_at: Utc::now(),
                master: 1.0,
                fader_position: 1.0,
                fader_pickup_required: false,
                flash: false,
                temporary: false,
                temporary_active: false,
                temporary_master: 1.0,
                swap_active: false,
                enabled: true,
                transition_timing_bypassed: false,
                manual_xfade_position: 0.0,
                manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
                manual_xfade_progress: 0.0,
            })),
        },
    }
}
