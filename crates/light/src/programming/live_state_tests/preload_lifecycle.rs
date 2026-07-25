use super::*;
use crate::{ActionErrorKind, ProgrammingPreloadLifecycleState};
use chrono::Utc;
use light_core::{AttributeKey, AttributeValue, ShowId};
use light_programmer::{
    PreloadPlaybackQueueAction, PreloadPlaybackQueueSurface, PreloadProgrammerValueMutation,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct LifecyclePorts {
    registry: ProgrammerRegistry,
    capture_programmer: AtomicBool,
    reconcile_selection: Mutex<Option<Vec<FixtureId>>>,
    reconciliations: AtomicUsize,
    persisted: Mutex<Vec<&'static str>>,
    commits: AtomicUsize,
}

impl ProgrammingPreloadLifecyclePorts for LifecyclePorts {
    fn authorize_preload_lifecycle(
        &self,
        _context: &ActionContext,
    ) -> Result<(), crate::ActionError> {
        Ok(())
    }

    fn capture_programmer_on_preload(&self, _context: &ActionContext) -> bool {
        self.capture_programmer.load(Ordering::SeqCst)
    }

    fn commit_preload(
        &self,
        context: &ActionContext,
        request: &ProgrammingPreloadLifecycleRequest,
    ) -> Result<ProgrammingPreloadCommitResult, crate::ActionError> {
        self.commits.fetch_add(1, Ordering::SeqCst);
        let session = SessionId(context.session_id.unwrap());
        let actions = self.registry.preload_playback_actions(session).unwrap();
        self.registry.activate_preload(session);
        self.registry.take_preload_playback_actions(session);
        let ProgrammingPreloadLifecycleAction::Go { show_id, .. } = request.action else {
            panic!("the commit port accepts only GO")
        };
        Ok(ProgrammingPreloadCommitResult {
            show_id,
            show_revision: 7,
            playback_event_sequence_before: 11,
            playback_event_sequence_after: 12,
            committed_at: Utc::now(),
            programmer_fade_millis: 400,
            executed_playback_actions: actions.len(),
            executed: actions
                .iter()
                .map(|action| ProgrammingPreloadExecutedPlaybackAction {
                    playback_number: action.playback_number,
                    page: action.page,
                    action: ProgrammingPreloadPlaybackAction::Go,
                    surface: ProgrammingPreloadPlaybackSurface::Physical,
                })
                .collect(),
            runtime_changes: Vec::new(),
            warnings: Vec::new(),
        })
    }

    fn reconcile_preload_capture(&self, context: &ActionContext) {
        self.reconciliations.fetch_add(1, Ordering::SeqCst);
        let Some(selection) = self.reconcile_selection.lock().clone() else {
            return;
        };
        self.registry
            .select(SessionId(context.session_id.unwrap()), selection);
    }

    fn persist_preload_lifecycle(
        &self,
        _context: &ActionContext,
        operation: &'static str,
    ) -> Option<String> {
        self.persisted.lock().push(operation);
        None
    }
}

struct LifecycleSetup {
    registry: ProgrammerRegistry,
    service: ProgrammingService,
    events: EventBus,
    user: UserId,
    session: SessionId,
    context: ActionContext,
    ports: LifecyclePorts,
    show_id: ShowId,
}

impl LifecycleSetup {
    fn new() -> Self {
        let registry = ProgrammerRegistry::default();
        let user = UserId::new();
        let session = SessionId::new();
        let desk = Uuid::new_v4();
        registry.start(session, user);
        registry.attach_command_context(session, SessionId(desk));
        let events = EventBus::new(64);
        let service = ProgrammingService::new(
            registry.clone(),
            events.clone(),
            Arc::new(HighlightRegistry::default()),
        );
        Self {
            registry: registry.clone(),
            service,
            events,
            user,
            session,
            context: ActionContext::operator(desk, user.0, session.0, ActionSource::Http),
            ports: LifecyclePorts {
                registry,
                capture_programmer: AtomicBool::new(true),
                reconcile_selection: Mutex::new(None),
                reconciliations: AtomicUsize::new(0),
                persisted: Mutex::new(Vec::new()),
                commits: AtomicUsize::new(0),
            },
            show_id: ShowId::new(),
        }
    }

    fn exact_action(
        &self,
        request_id: &str,
        action: ProgrammingPreloadLifecycleAction,
    ) -> ActionEnvelope<ProgrammingPreloadLifecycleRequest> {
        self.action_with_selection(request_id, self.selection_revision(), action)
    }

    fn action_with_selection(
        &self,
        request_id: &str,
        expected_selection_revision: u64,
        action: ProgrammingPreloadLifecycleAction,
    ) -> ActionEnvelope<ProgrammingPreloadLifecycleRequest> {
        let exact = ProgrammingPreloadRevisionExpectation::Exact;
        ActionEnvelope {
            context: self.context.clone().with_request_id(request_id),
            command: ProgrammingPreloadLifecycleRequest {
                expected_capture_mode_revision: exact(
                    self.registry.capture_mode_revision(self.user),
                ),
                expected_values_revision: exact(self.registry.preload_values_revision(self.user)),
                expected_queue_revision: exact(
                    self.registry.preload_playback_queue_revision(self.user),
                ),
                expected_selection_revision: exact(expected_selection_revision),
                action,
            },
        }
    }

    fn go(&self) -> ProgrammingPreloadLifecycleAction {
        ProgrammingPreloadLifecycleAction::Go {
            show_id: self.show_id,
            expected_show_revision: ProgrammingPreloadRevisionExpectation::Exact(7),
            expected_playback_event_sequence: ProgrammingPreloadRevisionExpectation::Exact(11),
        }
    }

    fn handle(
        &self,
        action: ActionEnvelope<ProgrammingPreloadLifecycleRequest>,
    ) -> Result<ProgrammingPreloadLifecycleResult, crate::ActionError> {
        self.service.handle_preload_lifecycle(action, &self.ports)
    }

    fn selection_revision(&self) -> u64 {
        self.registry.selection(self.session).unwrap().revision
    }

    fn arm_and_set_pending(&self, fixture: FixtureId) {
        self.registry.arm_preload(self.session, true);
        assert!(self.registry.apply_preload_values(
            self.session,
            &[PreloadProgrammerValueMutation::SetFixture {
                fixture_id: fixture,
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(0.5),
                timing: Default::default(),
            }],
        ));
    }

    fn queue(&self, playback_number: u16) {
        assert!(self.registry.queue_preload_playback_action(
            self.session,
            playback_number,
            None,
            PreloadPlaybackQueueAction::Go,
            PreloadPlaybackQueueSurface::Physical,
        ));
    }
}

#[test]
fn enter_reconciles_selection_once_and_replay_and_no_change_stay_sparse() {
    let setup = LifecycleSetup::new();
    let reconciled = FixtureId::new();
    *setup.ports.reconcile_selection.lock() = Some(vec![reconciled]);
    let action = setup.exact_action("enter-1", ProgrammingPreloadLifecycleAction::Enter);
    let first = setup.handle(action.clone()).unwrap();

    assert_eq!(first.state, ProgrammingPreloadLifecycleState::Changed);
    assert!(!first.active);
    assert!(first.capture_mode.blind);
    assert_eq!(first.capture_mode_event_sequence, Some(2));
    assert_eq!(first.interaction_event_sequence, Some(1));
    assert_eq!(first.selection_revision, 1);
    assert!(first.values_projection.is_none());
    assert!(first.queue_projection.is_none());
    assert_eq!(
        setup.registry.selection(setup.session).unwrap().selected,
        [reconciled]
    );
    assert_eq!(setup.ports.reconciliations.load(Ordering::SeqCst), 1);
    assert_eq!(*setup.ports.persisted.lock(), ["preload.enter"]);
    let cursor = setup.events.latest_sequence();

    super::super::preload_values_projection::reset_projection_read_count();
    super::super::preload_playback_queue_projection::reset_projection_read_count();
    let replay = setup.handle(action).unwrap();
    assert!(replay.replayed);
    assert_eq!(
        replay.capture_mode_event_sequence,
        first.capture_mode_event_sequence
    );
    assert_eq!(replay.selection_revision, 1);
    assert_eq!(setup.events.latest_sequence(), cursor);
    assert_eq!(setup.ports.reconciliations.load(Ordering::SeqCst), 1);
    assert_eq!(setup.ports.persisted.lock().len(), 1);
    assert_eq!(
        super::super::preload_values_projection::projection_read_count(),
        0
    );
    assert_eq!(
        super::super::preload_playback_queue_projection::projection_read_count(),
        0
    );

    let no_change = setup
        .handle(setup.exact_action("enter-2", ProgrammingPreloadLifecycleAction::Enter))
        .unwrap();
    assert_eq!(no_change.state, ProgrammingPreloadLifecycleState::NoChange);
    assert!(no_change.values_projection.is_none());
    assert!(no_change.queue_projection.is_none());
    assert_eq!(setup.events.latest_sequence(), cursor);
    assert_eq!(setup.ports.persisted.lock().len(), 1);
}

#[test]
fn clear_pending_publishes_one_values_and_queue_projection_and_preserves_active_values() {
    let setup = LifecycleSetup::new();
    let active_fixture = FixtureId::new();
    setup.arm_and_set_pending(active_fixture);
    assert!(setup.registry.activate_preload(setup.session));
    setup.arm_and_set_pending(FixtureId::new());
    setup.queue(3);
    let clear = setup
        .handle(setup.exact_action("clear-1", ProgrammingPreloadLifecycleAction::ClearPending))
        .unwrap();

    assert_eq!(clear.state, ProgrammingPreloadLifecycleState::Changed);
    assert!(clear.active);
    assert_eq!(clear.values_revision, 1);
    assert!(
        clear
            .values_projection
            .as_ref()
            .unwrap()
            .fixture_values
            .is_empty()
    );
    assert_eq!(clear.queue_revision, 1);
    assert!(clear.queue_projection.as_ref().unwrap().actions.is_empty());
    assert!(clear.values_event_sequence.is_some());
    assert!(clear.queue_event_sequence.is_some());
    assert_eq!(*setup.ports.persisted.lock(), ["preload.clear"]);

    super::super::preload_values_projection::reset_projection_read_count();
    super::super::preload_playback_queue_projection::reset_projection_read_count();
    let no_change = setup
        .handle(setup.exact_action("clear-2", ProgrammingPreloadLifecycleAction::ClearPending))
        .unwrap();
    assert_eq!(no_change.state, ProgrammingPreloadLifecycleState::NoChange);
    assert!(no_change.active);
    assert!(no_change.values_projection.is_none());
    assert!(no_change.queue_projection.is_none());
    assert_eq!(setup.ports.persisted.lock().len(), 1);
    assert_eq!(
        super::super::preload_values_projection::projection_read_count(),
        0
    );
    assert_eq!(
        super::super::preload_playback_queue_projection::projection_read_count(),
        0
    );
}

#[test]
fn go_checks_selection_commits_once_and_replay_repeats_no_effect_or_event() {
    let setup = LifecycleSetup::new();
    setup.arm_and_set_pending(FixtureId::new());
    setup.queue(8);
    let stale = setup.action_with_selection("go-stale", 99, setup.go());
    let error = setup.handle(stale).unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(0));
    assert_eq!(setup.ports.commits.load(Ordering::SeqCst), 0);

    let action = setup.exact_action("go-1", setup.go());
    let first = setup.handle(action.clone()).unwrap();
    assert_eq!(first.state, ProgrammingPreloadLifecycleState::Changed);
    assert!(first.active);
    assert!(!first.capture_mode.blind);
    assert!(first.capture_mode_event_sequence.is_some());
    assert!(first.values_event_sequence.is_some());
    assert!(first.queue_event_sequence.is_some());
    assert_eq!(first.commit.as_ref().unwrap().executed_playback_actions, 1);
    assert_eq!(
        first.commit.as_ref().unwrap().executed[0].playback_number,
        8
    );
    assert_eq!(setup.ports.commits.load(Ordering::SeqCst), 1);
    let cursor = setup.events.latest_sequence();

    let replay = setup.handle(action).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.commit, first.commit);
    assert_eq!(setup.ports.commits.load(Ordering::SeqCst), 1);
    assert_eq!(setup.events.latest_sequence(), cursor);
}

#[test]
fn release_checks_selection_then_clears_active_values_without_pending_projection() {
    let setup = LifecycleSetup::new();
    setup.arm_and_set_pending(FixtureId::new());
    assert!(setup.registry.activate_preload(setup.session));
    setup.registry.arm_preload(setup.session, true);

    let stale = setup.action_with_selection(
        "release-stale",
        99,
        ProgrammingPreloadLifecycleAction::Release,
    );
    let error = setup.handle(stale).unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert!(setup.registry.has_active_preload(setup.session).unwrap());
    assert!(setup.registry.capture_mode(setup.session).unwrap().blind);

    let released = setup
        .handle(setup.exact_action("release-1", ProgrammingPreloadLifecycleAction::Release))
        .unwrap();
    assert_eq!(released.state, ProgrammingPreloadLifecycleState::Changed);
    assert!(!released.active);
    assert!(!released.capture_mode.blind);
    assert!(released.values_projection.is_none());
    assert!(released.queue_projection.is_none());
    assert_eq!(released.selection_revision, setup.selection_revision());
    assert_eq!(*setup.ports.persisted.lock(), ["preload.release"]);

    let cursor = setup.events.latest_sequence();
    let no_change = setup
        .handle(setup.exact_action("release-2", ProgrammingPreloadLifecycleAction::Release))
        .unwrap();
    assert_eq!(no_change.state, ProgrammingPreloadLifecycleState::NoChange);
    assert!(no_change.values_projection.is_none());
    assert!(no_change.queue_projection.is_none());
    assert_eq!(setup.events.latest_sequence(), cursor);
    assert_eq!(setup.ports.persisted.lock().len(), 1);
}

#[test]
fn every_captured_revision_conflict_is_primary_and_has_no_related_authority() {
    let setup = LifecycleSetup::new();
    setup.arm_and_set_pending(FixtureId::new());
    setup.queue(4);
    let cases = [
        ("enter-capture", ProgrammingPreloadLifecycleAction::Enter, 0),
        (
            "enter-selection",
            ProgrammingPreloadLifecycleAction::Enter,
            3,
        ),
        (
            "clear-values",
            ProgrammingPreloadLifecycleAction::ClearPending,
            1,
        ),
        (
            "clear-queue",
            ProgrammingPreloadLifecycleAction::ClearPending,
            2,
        ),
        ("go-capture", setup.go(), 0),
        ("go-values", setup.go(), 1),
        ("go-queue", setup.go(), 2),
        ("go-selection", setup.go(), 3),
        (
            "release-capture",
            ProgrammingPreloadLifecycleAction::Release,
            0,
        ),
        (
            "release-values",
            ProgrammingPreloadLifecycleAction::Release,
            1,
        ),
        (
            "release-queue",
            ProgrammingPreloadLifecycleAction::Release,
            2,
        ),
        (
            "release-selection",
            ProgrammingPreloadLifecycleAction::Release,
            3,
        ),
    ];

    for (request_id, action, stale_field) in cases {
        let mut envelope = setup.exact_action(request_id, action);
        let stale = ProgrammingPreloadRevisionExpectation::Exact(99);
        match stale_field {
            0 => envelope.command.expected_capture_mode_revision = stale,
            1 => envelope.command.expected_values_revision = stale,
            2 => envelope.command.expected_queue_revision = stale,
            3 => envelope.command.expected_selection_revision = stale,
            _ => unreachable!(),
        }
        let error = setup.handle(envelope).unwrap_err();
        assert_eq!(error.kind, ActionErrorKind::Conflict, "{request_id}");
        assert_eq!(error.current_revision, Some(0), "{request_id}");
        assert_eq!(error.current_related_revision, None, "{request_id}");
    }
    assert_eq!(setup.ports.commits.load(Ordering::SeqCst), 0);
    assert!(setup.ports.persisted.lock().is_empty());
    assert_eq!(setup.events.latest_sequence(), 0);
}

#[test]
fn ownership_is_exact_and_same_user_peer_desk_observes_shared_capture_authority() {
    let setup = LifecycleSetup::new();
    setup
        .handle(setup.exact_action("owner-enter", ProgrammingPreloadLifecycleAction::Enter))
        .unwrap();
    let peer_session = SessionId::new();
    let peer_desk = Uuid::new_v4();
    setup.registry.start(peer_session, setup.user);
    setup
        .registry
        .attach_command_context(peer_session, SessionId(peer_desk));
    let peer = ActionEnvelope {
        context: ActionContext::operator(
            peer_desk,
            setup.user.0,
            peer_session.0,
            ActionSource::Http,
        )
        .with_request_id("peer-enter"),
        command: ProgrammingPreloadLifecycleRequest {
            expected_capture_mode_revision: ProgrammingPreloadRevisionExpectation::Exact(1),
            expected_values_revision: ProgrammingPreloadRevisionExpectation::Exact(0),
            expected_queue_revision: ProgrammingPreloadRevisionExpectation::Exact(0),
            expected_selection_revision: ProgrammingPreloadRevisionExpectation::Exact(0),
            action: ProgrammingPreloadLifecycleAction::Enter,
        },
    };
    let peer_result = setup.handle(peer).unwrap();
    assert_eq!(
        peer_result.state,
        ProgrammingPreloadLifecycleState::NoChange
    );
    assert!(peer_result.capture_mode.blind);

    let forged = ActionEnvelope {
        context: ActionContext::operator(
            setup.context.desk_id,
            UserId::new().0,
            setup.session.0,
            ActionSource::Http,
        )
        .with_request_id("foreign-enter"),
        command: ProgrammingPreloadLifecycleRequest {
            expected_capture_mode_revision: ProgrammingPreloadRevisionExpectation::Exact(1),
            expected_values_revision: ProgrammingPreloadRevisionExpectation::Exact(0),
            expected_queue_revision: ProgrammingPreloadRevisionExpectation::Exact(0),
            expected_selection_revision: ProgrammingPreloadRevisionExpectation::Exact(0),
            action: ProgrammingPreloadLifecycleAction::Enter,
        },
    };
    assert_eq!(
        setup.handle(forged).unwrap_err().kind,
        ActionErrorKind::Forbidden
    );
}
