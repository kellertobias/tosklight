use super::*;
use crate::{
    ActionError, ActionErrorKind, ProgrammingGroupCommit, ProgrammingGroupCommitResult,
    ProgrammingGroupProjection, ProgrammingGroupRecordOperation, ProgrammingGroupRecordOutcome,
    ProgrammingGroupRecordRequest, ProgrammingGroupRecordingPorts,
    ProgrammingGroupRevisionExpectation,
};
use light_core::{Revision, ShowId};
use light_programmer::{GroupDefinition, SelectionReference};
use light_show::PortableShowRevision;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};

#[derive(Clone)]
struct StoredGroup {
    revision: Revision,
    raw_body: serde_json::Value,
}

struct FakeShow {
    show_id: ShowId,
    revision: Revision,
    groups: HashMap<String, StoredGroup>,
}

struct GroupPorts {
    registry: ProgrammerRegistry,
    show: Mutex<FakeShow>,
    calls: AtomicUsize,
    event_sequence: AtomicU64,
    fail: AtomicBool,
}

impl GroupPorts {
    fn new(show_id: ShowId, registry: ProgrammerRegistry) -> Self {
        Self {
            registry,
            show: Mutex::new(FakeShow {
                show_id,
                revision: 0,
                groups: HashMap::new(),
            }),
            calls: AtomicUsize::new(0),
            event_sequence: AtomicU64::new(0),
            fail: AtomicBool::new(false),
        }
    }

    fn seed(&self, id: &str, group: GroupDefinition) {
        let mut show = self.show.lock();
        show.revision += 1;
        show.groups.insert(
            id.into(),
            StoredGroup {
                revision: 1,
                raw_body: serde_json::to_value(group).unwrap(),
            },
        );
    }

    fn stored(&self, id: &str) -> Option<GroupDefinition> {
        self.show
            .lock()
            .groups
            .get(id)
            .map(|stored| serde_json::from_value(stored.raw_body.clone()).unwrap())
    }
}

impl ProgrammingGroupRecordingPorts for GroupPorts {
    fn authorize_group_recording(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn commit_group(
        &self,
        _context: &ActionContext,
        commit: &ProgrammingGroupCommit,
    ) -> Result<ProgrammingGroupCommitResult, ActionError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.fail.load(Ordering::Relaxed) {
            return Err(ActionError::new(
                ActionErrorKind::Unavailable,
                "simulated Group persistence failure",
            ));
        }
        let mut show = self.show.lock();
        validate_fake_show(&show, commit)?;
        let current = show.groups.get(&commit.group_id).cloned();
        validate_fake_revision(current.as_ref(), commit.expected_object_revision)?;
        let groups = decoded_groups(&show.groups);
        let existing = groups.get(&commit.group_id);
        let updated = commit.updated_group(existing, &groups)?;
        if updated.is_none() && current.is_none() {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                format!("Group {} does not exist", commit.group_id),
            ));
        }
        let raw_body = updated.map(|group| serde_json::to_value(group).unwrap());
        let changed = current.as_ref().map(|stored| &stored.raw_body) != raw_body.as_ref();
        let object_revision =
            current.as_ref().map_or(0, |stored| stored.revision) + u64::from(changed);
        let event_sequence = changed.then(|| {
            commit.finish_actor_selection_gesture(&self.registry);
            show.revision += 1;
            match raw_body.clone() {
                Some(raw_body) => {
                    show.groups.insert(
                        commit.group_id.clone(),
                        StoredGroup {
                            revision: object_revision,
                            raw_body,
                        },
                    );
                }
                None => {
                    show.groups.remove(&commit.group_id);
                }
            }
            self.event_sequence.fetch_add(1, Ordering::Relaxed) + 1
        });
        Ok(ProgrammingGroupCommitResult {
            changed,
            projection: ProgrammingGroupProjection {
                show_id: show.show_id,
                object_id: commit.group_id.clone(),
                object_revision,
                deleted: raw_body.is_none(),
                raw_body: raw_body.map(Arc::new),
            },
            show_revision: PortableShowRevision::from_value(show.revision),
            event_sequence,
        })
    }
}

fn validate_fake_show(show: &FakeShow, commit: &ProgrammingGroupCommit) -> Result<(), ActionError> {
    if show.show_id != commit.show_id {
        return Err(ActionError::new(
            ActionErrorKind::NotFound,
            "show is not active",
        ));
    }
    if let Some(expected) = commit.expected_show_revision
        && expected.value() != show.revision
    {
        return Err(
            ActionError::new(ActionErrorKind::Conflict, "show revision conflict")
                .at_related_revision(show.revision),
        );
    }
    Ok(())
}

fn validate_fake_revision(
    current: Option<&StoredGroup>,
    expected: ProgrammingGroupRevisionExpectation,
) -> Result<(), ActionError> {
    let actual = current.map_or(0, |stored| stored.revision);
    if expected == ProgrammingGroupRevisionExpectation::Current
        || expected == ProgrammingGroupRevisionExpectation::Exact(actual)
    {
        Ok(())
    } else {
        Err(
            ActionError::new(ActionErrorKind::Conflict, "Group revision conflict")
                .at_revision(actual),
        )
    }
}

fn decoded_groups(groups: &HashMap<String, StoredGroup>) -> HashMap<String, GroupDefinition> {
    groups
        .iter()
        .map(|(id, stored)| {
            let mut group =
                serde_json::from_value::<GroupDefinition>(stored.raw_body.clone()).unwrap();
            group.id.clone_from(id);
            (id.clone(), group)
        })
        .collect()
}

struct GroupSetup {
    registry: ProgrammerRegistry,
    events: EventBus,
    service: ProgrammingService,
    ports: Arc<GroupPorts>,
    show_id: ShowId,
    user_id: UserId,
    session_id: SessionId,
    context: ActionContext,
}

impl GroupSetup {
    fn new() -> Self {
        let registry = ProgrammerRegistry::default();
        let user_id = UserId::new();
        let session_id = SessionId::new();
        let desk_id = Uuid::new_v4();
        let show_id = ShowId::new();
        registry.start(session_id, user_id);
        registry.attach_command_context(session_id, SessionId(desk_id));
        let events = EventBus::default();
        Self {
            service: ProgrammingService::new(
                registry.clone(),
                events.clone(),
                Arc::new(HighlightRegistry::default()),
            ),
            ports: Arc::new(GroupPorts::new(show_id, registry.clone())),
            registry,
            events,
            show_id,
            user_id,
            session_id,
            context: ActionContext::operator(desk_id, user_id.0, session_id.0, ActionSource::Http),
        }
    }

    fn request(
        &self,
        group_id: &str,
        operation: ProgrammingGroupRecordOperation,
        expected: ProgrammingGroupRevisionExpectation,
    ) -> ProgrammingGroupRecordRequest {
        ProgrammingGroupRecordRequest {
            show_id: self.show_id,
            group_id: group_id.into(),
            operation,
            expected_object_revision: expected,
            expected_show_revision: None,
        }
    }

    fn action(
        &self,
        request_id: &str,
        request: ProgrammingGroupRecordRequest,
    ) -> ActionEnvelope<ProgrammingGroupRecordRequest> {
        ActionEnvelope {
            context: self.context.clone().with_request_id(request_id),
            command: request,
        }
    }

    fn open_fixture_gesture(&self, fixture: FixtureId) {
        assert!(self.registry.apply_selection_gesture(
            self.session_id,
            vec![SelectionReference::Fixture {
                fixture_id: fixture,
            }],
            &HashMap::new(),
        ));
    }
}

#[test]
fn changed_record_captures_order_and_closes_the_actor_gesture() {
    let setup = GroupSetup::new();
    let first = FixtureId::new();
    let second = FixtureId::new();
    setup.open_fixture_gesture(first);
    assert!(setup.registry.apply_selection_gesture(
        setup.session_id,
        vec![SelectionReference::Fixture { fixture_id: second }],
        &HashMap::new(),
    ));
    let request = setup.request(
        "opaque A.01",
        ProgrammingGroupRecordOperation::Overwrite,
        ProgrammingGroupRevisionExpectation::Exact(0),
    );

    let result = setup
        .service
        .handle_group_recording(
            setup.action("record-ordered", request),
            setup.ports.as_ref(),
        )
        .unwrap();

    assert_eq!(result.applied, 2);
    assert!(matches!(
        result.outcome,
        ProgrammingGroupRecordOutcome::Changed { .. }
    ));
    assert_eq!(
        setup.ports.stored("opaque A.01").unwrap().fixtures,
        vec![first, second]
    );
    assert!(
        !setup
            .registry
            .selection(setup.session_id)
            .unwrap()
            .gesture_open
    );
}

#[test]
fn direct_no_change_closes_and_publishes_one_final_interaction() {
    let setup = GroupSetup::new();
    let fixture = FixtureId::new();
    setup.ports.seed(
        "same",
        GroupDefinition {
            id: "same".into(),
            name: "Group same".into(),
            fixtures: vec![fixture],
            ..Default::default()
        },
    );
    setup.open_fixture_gesture(fixture);
    let request = setup.request(
        "same",
        ProgrammingGroupRecordOperation::Overwrite,
        ProgrammingGroupRevisionExpectation::Exact(1),
    );

    let result = setup
        .service
        .handle_group_recording(
            setup.action("record-no-change", request),
            setup.ports.as_ref(),
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        ProgrammingGroupRecordOutcome::NoChange { .. }
    ));
    assert!(
        !setup
            .registry
            .selection(setup.session_id)
            .unwrap()
            .gesture_open
    );
    let EventReplay::Events(events) = setup.events.replay(
        0,
        &EventFilter::for_desk(setup.context.desk_id)
            .with_object(EventObject::programming_selection(setup.context.desk_id)),
    ) else {
        panic!("the final selection interaction should be retained")
    };
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].correlation_id,
        Some(result.context.correlation_id)
    );
}

#[test]
fn nested_no_change_defers_publication_to_the_outer_interaction() {
    let setup = GroupSetup::new();
    let fixture = FixtureId::new();
    setup.ports.seed(
        "same",
        GroupDefinition {
            id: "same".into(),
            name: "Group same".into(),
            fixtures: vec![fixture],
            ..Default::default()
        },
    );
    setup.open_fixture_gesture(fixture);
    let request = setup.request(
        "same",
        ProgrammingGroupRecordOperation::Overwrite,
        ProgrammingGroupRevisionExpectation::Current,
    );

    setup
        .service
        .record_group_within_interaction(
            setup.action("nested-no-change", request),
            setup.ports.as_ref(),
        )
        .unwrap();

    assert!(
        !setup
            .registry
            .selection(setup.session_id)
            .unwrap()
            .gesture_open
    );
    assert_eq!(setup.events.latest_sequence(), 0);
}

#[test]
fn replay_and_failure_do_not_repeat_or_close_a_new_gesture() {
    let setup = GroupSetup::new();
    let first = FixtureId::new();
    setup.registry.select(setup.session_id, [first]);
    let request = setup.request(
        "replay",
        ProgrammingGroupRecordOperation::Overwrite,
        ProgrammingGroupRevisionExpectation::Exact(0),
    );
    let action = setup.action("stable-id", request);
    setup
        .service
        .handle_group_recording(action.clone(), setup.ports.as_ref())
        .unwrap();
    setup.open_fixture_gesture(FixtureId::new());

    let replay = setup
        .service
        .handle_group_recording(action, setup.ports.as_ref())
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(setup.ports.calls.load(Ordering::Relaxed), 1);
    assert!(
        setup
            .registry
            .selection(setup.session_id)
            .unwrap()
            .gesture_open
    );

    setup.ports.fail.store(true, Ordering::Relaxed);
    let failed = setup.service.handle_group_recording(
        setup.action(
            "failure",
            setup.request(
                "failure",
                ProgrammingGroupRecordOperation::Overwrite,
                ProgrammingGroupRevisionExpectation::Current,
            ),
        ),
        setup.ports.as_ref(),
    );
    assert_eq!(failed.unwrap_err().kind, ActionErrorKind::Unavailable);
    assert!(
        setup
            .registry
            .selection(setup.session_id)
            .unwrap()
            .gesture_open
    );
}

#[test]
fn explicit_delete_preserves_gesture_but_empty_subtract_finishes_it() {
    let setup = GroupSetup::new();
    setup.ports.seed(
        "delete",
        GroupDefinition {
            id: "delete".into(),
            name: "Delete".into(),
            ..Default::default()
        },
    );
    setup.open_fixture_gesture(FixtureId::new());
    let delete = setup.request(
        "delete",
        ProgrammingGroupRecordOperation::Delete,
        ProgrammingGroupRevisionExpectation::Exact(1),
    );
    setup
        .service
        .handle_group_recording(setup.action("delete", delete), setup.ports.as_ref())
        .unwrap();
    assert!(
        setup
            .registry
            .selection(setup.session_id)
            .unwrap()
            .gesture_open
    );

    setup.ports.seed(
        "subtract",
        GroupDefinition {
            id: "subtract".into(),
            name: "Subtract".into(),
            ..Default::default()
        },
    );
    setup.registry.select(setup.session_id, []);
    let removed = FixtureId::new();
    setup.open_fixture_gesture(removed);
    assert!(setup.registry.apply_selection_gesture(
        setup.session_id,
        vec![SelectionReference::RemoveFixture {
            fixture_id: removed,
        }],
        &HashMap::new(),
    ));
    assert!(
        setup
            .registry
            .selection(setup.session_id)
            .unwrap()
            .selected
            .is_empty()
    );
    assert!(
        setup
            .registry
            .selection(setup.session_id)
            .unwrap()
            .gesture_open
    );
    let subtract = setup.request(
        "subtract",
        ProgrammingGroupRecordOperation::Subtract,
        ProgrammingGroupRevisionExpectation::Exact(1),
    );
    let result = setup
        .service
        .handle_group_recording(
            setup.action("subtract-delete", subtract),
            setup.ports.as_ref(),
        )
        .unwrap();
    assert_eq!(result.applied, 1);
    assert!(
        !setup
            .registry
            .selection(setup.session_id)
            .unwrap()
            .gesture_open
    );
    assert!(setup.ports.stored("subtract").is_none());
}

#[test]
fn same_user_desks_capture_independent_authoritative_selections() {
    let setup = GroupSetup::new();
    let peer_session = SessionId::new();
    let peer_desk = Uuid::new_v4();
    setup.registry.start(peer_session, setup.user_id);
    setup
        .registry
        .attach_command_context(peer_session, SessionId(peer_desk));
    let actor_fixture = FixtureId::new();
    let peer_fixture = FixtureId::new();
    setup.registry.select(setup.session_id, [actor_fixture]);
    setup.registry.select(peer_session, [peer_fixture]);
    let actor = setup.request(
        "actor",
        ProgrammingGroupRecordOperation::Overwrite,
        ProgrammingGroupRevisionExpectation::Exact(0),
    );
    setup
        .service
        .handle_group_recording(setup.action("shared-id", actor), setup.ports.as_ref())
        .unwrap();

    let peer_context = ActionContext::operator(
        peer_desk,
        setup.user_id.0,
        peer_session.0,
        ActionSource::Http,
    )
    .with_request_id("shared-id");
    setup
        .service
        .handle_group_recording(
            ActionEnvelope {
                context: peer_context,
                command: setup.request(
                    "peer",
                    ProgrammingGroupRecordOperation::Overwrite,
                    ProgrammingGroupRevisionExpectation::Exact(0),
                ),
            },
            setup.ports.as_ref(),
        )
        .unwrap();

    assert_eq!(
        setup.ports.stored("actor").unwrap().fixtures,
        [actor_fixture]
    );
    assert_eq!(setup.ports.stored("peer").unwrap().fixtures, [peer_fixture]);
}

#[test]
fn foreign_user_revision_conflict_and_request_reuse_are_rejected() {
    let setup = GroupSetup::new();
    setup.registry.select(setup.session_id, [FixtureId::new()]);
    let mut foreign = setup.action(
        "foreign",
        setup.request(
            "foreign",
            ProgrammingGroupRecordOperation::Overwrite,
            ProgrammingGroupRevisionExpectation::Current,
        ),
    );
    foreign.context.user_id = Some(UserId::new().0);
    assert_eq!(
        setup
            .service
            .handle_group_recording(foreign, setup.ports.as_ref())
            .unwrap_err()
            .kind,
        ActionErrorKind::Forbidden
    );

    setup.ports.seed(
        "stale",
        GroupDefinition {
            id: "stale".into(),
            ..Default::default()
        },
    );
    let stale = setup.request(
        "stale",
        ProgrammingGroupRecordOperation::Overwrite,
        ProgrammingGroupRevisionExpectation::Exact(0),
    );
    let error = setup
        .service
        .handle_group_recording(setup.action("stale", stale), setup.ports.as_ref())
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(1));

    let first = setup.request(
        "one",
        ProgrammingGroupRecordOperation::Overwrite,
        ProgrammingGroupRevisionExpectation::Exact(0),
    );
    setup
        .service
        .handle_group_recording(setup.action("reuse", first), setup.ports.as_ref())
        .unwrap();
    let reused = setup.request(
        "two",
        ProgrammingGroupRecordOperation::Overwrite,
        ProgrammingGroupRevisionExpectation::Exact(0),
    );
    assert_eq!(
        setup
            .service
            .handle_group_recording(setup.action("reuse", reused), setup.ports.as_ref())
            .unwrap_err()
            .kind,
        ActionErrorKind::Conflict
    );
}

#[test]
fn whitespace_only_id_is_rejected_without_calling_the_port() {
    let setup = GroupSetup::new();
    let error = setup
        .service
        .handle_group_recording(
            setup.action(
                "blank",
                setup.request(
                    "   ",
                    ProgrammingGroupRecordOperation::Overwrite,
                    ProgrammingGroupRevisionExpectation::Current,
                ),
            ),
            setup.ports.as_ref(),
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(setup.ports.calls.load(Ordering::Relaxed), 0);
}
