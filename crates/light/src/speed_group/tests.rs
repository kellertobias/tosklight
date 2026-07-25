use parking_lot::Mutex;
use std::sync::Arc;
use uuid::Uuid;

use crate::{ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, EventBus};

use super::*;

const AUTHORITY: Uuid = Uuid::from_u128(100);

#[test]
fn absolute_and_relative_actions_share_one_revisioned_service() {
    let events = EventBus::default();
    let service = SpeedGroupService::with_authority(events.clone(), AUTHORITY);
    let ports = FakePorts::default();

    let absolute = service
        .handle(
            envelope(
                context(1, 10, "absolute"),
                SpeedGroupCommand::current(SpeedGroupAction::SetBpm {
                    group: group(1),
                    bpm: bpm(128.5),
                }),
            ),
            &ports,
        )
        .unwrap();
    let relative = service
        .handle(
            envelope(
                context(2, 20, "relative"),
                SpeedGroupCommand::exact(
                    AUTHORITY,
                    1,
                    SpeedGroupAction::AdjustBpm {
                        group: group(1),
                        delta: delta(-8.25),
                    },
                ),
            ),
            &ports,
        )
        .unwrap();

    assert_eq!(absolute.revision, 1);
    assert_eq!(absolute.groups[0].manual_bpm, 128.5);
    assert_eq!(relative.revision, 2);
    assert_eq!(relative.groups[0].manual_bpm, 120.25);
    assert_eq!(ports.applies(), 2);
    assert_eq!(ports.clock_reads(), 2);
    assert_eq!(events.latest_sequence(), 2);
}

#[test]
fn concurrent_relative_actions_resolve_under_the_application_lock() {
    let service = Arc::new(SpeedGroupService::with_authority(
        EventBus::default(),
        AUTHORITY,
    ));
    let ports = Arc::new(FakePorts::default());
    let mut workers = Vec::new();
    for request in ["relative-one", "relative-two"] {
        let service = Arc::clone(&service);
        let ports = Arc::clone(&ports);
        workers.push(std::thread::spawn(move || {
            service
                .handle(
                    envelope(
                        context(1, 10, request),
                        SpeedGroupCommand::current(SpeedGroupAction::AdjustBpm {
                            group: group(1),
                            delta: delta(1.0),
                        }),
                    ),
                    ports.as_ref(),
                )
                .unwrap()
        }));
    }
    let mut revisions = workers
        .into_iter()
        .map(|worker| worker.join().unwrap().revision)
        .collect::<Vec<_>>();
    revisions.sort_unstable();

    assert_eq!(revisions, [1, 2]);
    assert_eq!(ports.state().groups[0].manual_bpm, 122.0);
    assert_eq!(ports.applies(), 2);
}

#[test]
fn direct_entry_breaks_reciprocal_link_and_resets_manual_controls_once() {
    let service = SpeedGroupService::with_authority(EventBus::default(), AUTHORITY);
    let ports = FakePorts::default();
    ports.update(|state| {
        state.groups[0].paused = true;
        state.groups[0].speed_master_scale = 0.5;
        state.groups[0].synchronized_with = Some(group(2));
        state.groups[1].synchronized_with = Some(group(1));
        state
            .manual_control_clean
            .retain(|candidate| *candidate != group(1));
    });

    let result = service
        .handle(
            envelope(
                context(1, 10, "unlink"),
                SpeedGroupCommand::current(SpeedGroupAction::SetBpm {
                    group: group(1),
                    bpm: bpm(120.0),
                }),
            ),
            &ports,
        )
        .unwrap();

    assert_eq!(result.groups.len(), 2);
    assert_eq!(result.groups[0].group, group(1));
    assert_eq!(result.groups[1].group, group(2));
    assert!(!result.groups[0].paused);
    assert_eq!(result.groups[0].speed_master_scale, 1.0);
    assert_eq!(result.groups[0].phase_origin_millis, 10_001);
    assert!(
        result
            .groups
            .iter()
            .all(|item| item.synchronized_with.is_none())
    );
}

#[test]
fn synchronization_copies_source_rate_pause_and_phase_and_detaches_old_peers() {
    let service = SpeedGroupService::with_authority(EventBus::default(), AUTHORITY);
    let ports = FakePorts::default();
    ports.update(|state| {
        state.groups[0].manual_bpm = 144.0;
        state.groups[0].paused = true;
        state.groups[0].phase_origin_millis = 777;
        state.groups[0].synchronized_with = Some(group(3));
        state.groups[2].synchronized_with = Some(group(1));
        state.groups[1].synchronized_with = Some(group(4));
        state.groups[3].synchronized_with = Some(group(2));
    });

    let result = service
        .handle(
            envelope(
                context(1, 10, "sync"),
                SpeedGroupCommand::current(SpeedGroupAction::Synchronize {
                    source: group(1),
                    target: group(2),
                }),
            ),
            &ports,
        )
        .unwrap();

    assert_eq!(
        result
            .groups
            .iter()
            .map(|item| item.group)
            .collect::<Vec<_>>(),
        [group(1), group(2), group(3), group(4),]
    );
    let state = ports.state();
    for (index, peer) in [(0, group(2)), (1, group(1))] {
        assert_eq!(state.groups[index].manual_bpm, 144.0);
        assert!(state.groups[index].paused);
        assert_eq!(state.groups[index].phase_origin_millis, 777);
        assert_eq!(state.groups[index].synchronized_with, Some(peer));
    }
    assert_eq!(state.groups[2].synchronized_with, None);
    assert_eq!(state.groups[3].synchronized_with, None);
}

#[test]
fn canonical_direct_and_link_actions_are_no_change() {
    let events = EventBus::default();
    let service = SpeedGroupService::with_authority(events.clone(), AUTHORITY);
    let ports = FakePorts::default();

    let direct = service
        .handle(
            envelope(
                context(1, 10, "direct-noop"),
                SpeedGroupCommand::current(SpeedGroupAction::SetBpm {
                    group: group(1),
                    bpm: bpm(120.0),
                }),
            ),
            &ports,
        )
        .unwrap();
    ports.update(|state| {
        state.groups[0].synchronized_with = Some(group(2));
        state.groups[1].synchronized_with = Some(group(1));
    });
    let linked = service
        .handle(
            envelope(
                context(1, 10, "sync-noop"),
                SpeedGroupCommand::current(SpeedGroupAction::Synchronize {
                    source: group(1),
                    target: group(2),
                }),
            ),
            &ports,
        )
        .unwrap();

    assert_eq!(direct.outcome, SpeedGroupOutcome::NoChange);
    assert_eq!(linked.outcome, SpeedGroupOutcome::NoChange);
    assert_eq!(ports.applies(), 0);
    assert_eq!(events.latest_sequence(), 0);
}

#[test]
fn replay_precedes_stale_expectation_and_collision_is_rejected() {
    let service = SpeedGroupService::with_authority(EventBus::default(), AUTHORITY);
    let ports = FakePorts::default();
    let action = SpeedGroupAction::SetBpm {
        group: group(1),
        bpm: bpm(130.0),
    };
    let first = service
        .handle(
            envelope(
                context(1, 10, "same"),
                SpeedGroupCommand::exact(AUTHORITY, 0, action),
            ),
            &ports,
        )
        .unwrap();
    ports.fail_non_authorization_calls(true);
    let replay = service
        .handle(
            envelope(
                context(1, 10, "same"),
                SpeedGroupCommand::exact(AUTHORITY, 0, action),
            ),
            &ports,
        )
        .unwrap();
    let collision = service
        .handle(
            envelope(
                context(1, 10, "same"),
                SpeedGroupCommand::exact(
                    AUTHORITY,
                    1,
                    SpeedGroupAction::SetBpm {
                        group: group(1),
                        bpm: bpm(131.0),
                    },
                ),
            ),
            &ports,
        )
        .unwrap_err();
    ports.fail_non_authorization_calls(false);

    assert_eq!(first.event_sequence, replay.event_sequence);
    assert!(replay.replayed);
    assert_eq!(ports.applies(), 1);
    assert_eq!(ports.clock_reads(), 1);
    assert_eq!(collision.kind, ActionErrorKind::Conflict);
}

#[test]
fn another_desk_or_replaced_session_does_not_share_request_replay() {
    let service = SpeedGroupService::with_authority(EventBus::default(), AUTHORITY);
    let ports = FakePorts::default();
    let action = SpeedGroupCommand::current(SpeedGroupAction::SetBpm {
        group: group(1),
        bpm: bpm(130.0),
    });
    service
        .handle(envelope(context(1, 10, "request"), action), &ports)
        .unwrap();
    let other_desk = service
        .handle(envelope(context(2, 10, "request"), action), &ports)
        .unwrap();
    let new_session = service
        .handle(envelope(context(1, 11, "request"), action), &ports)
        .unwrap();

    assert!(!other_desk.replayed);
    assert!(!new_session.replayed);
    assert_eq!(other_desk.outcome, SpeedGroupOutcome::NoChange);
    assert_eq!(new_session.outcome, SpeedGroupOutcome::NoChange);
}

#[test]
fn stale_revision_and_replaced_authority_report_current_revision() {
    let service = SpeedGroupService::with_authority(EventBus::default(), AUTHORITY);
    let ports = FakePorts::default();
    let action = SpeedGroupAction::SetBpm {
        group: group(1),
        bpm: bpm(130.0),
    };
    service
        .handle(
            envelope(context(1, 10, "first"), SpeedGroupCommand::current(action)),
            &ports,
        )
        .unwrap();
    let stale = service
        .handle(
            envelope(
                context(1, 10, "stale"),
                SpeedGroupCommand::exact(AUTHORITY, 0, action),
            ),
            &ports,
        )
        .unwrap_err();
    let replaced = service
        .handle(
            envelope(
                context(1, 10, "replaced"),
                SpeedGroupCommand::exact(Uuid::from_u128(999), 1, action),
            ),
            &ports,
        )
        .unwrap_err();

    assert_eq!(stale.current_revision, Some(1));
    assert_eq!(replaced.current_revision, Some(1));
    assert_eq!(ports.clock_reads(), 1);
}

#[test]
fn persistence_warning_is_retained_in_the_single_changed_outcome() {
    let events = EventBus::default();
    let service = SpeedGroupService::with_authority(events.clone(), AUTHORITY);
    let ports = FakePorts::default();
    ports.persistence_pending();

    let result = service
        .handle(
            envelope(
                context(1, 10, "pending"),
                SpeedGroupCommand::current(SpeedGroupAction::SetBpm {
                    group: group(1),
                    bpm: bpm(121.0),
                }),
            ),
            &ports,
        )
        .unwrap();

    assert_eq!(result.durability, SpeedGroupDurability::PersistencePending);
    assert_eq!(result.warning.as_deref(), Some("persistence pending"));
    assert_eq!(events.latest_sequence(), 1);
    assert_eq!(ports.applies(), 1);
}

#[test]
fn malformed_adapter_state_never_publishes_an_authoritative_event() {
    let events = EventBus::default();
    let service = SpeedGroupService::with_authority(events.clone(), AUTHORITY);
    let ports = FakePorts::default();
    ports.malform_next();

    let error = service
        .handle(
            envelope(
                context(1, 10, "malformed"),
                SpeedGroupCommand::current(SpeedGroupAction::SetBpm {
                    group: group(1),
                    bpm: bpm(140.0),
                }),
            ),
            &ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Internal);
    assert_eq!(events.latest_sequence(), 0);
    let snapshot = service
        .snapshot(&context(1, 10, "snapshot"), &ports)
        .unwrap();
    assert_eq!(snapshot.projection.revision, 0);
}

fn group(value: u8) -> SpeedGroupId {
    SpeedGroupId::new(value).unwrap()
}

fn bpm(value: f64) -> SpeedBpm {
    SpeedBpm::new(value).unwrap()
}

fn delta(value: f64) -> SpeedBpmDelta {
    SpeedBpmDelta::new(value).unwrap()
}

fn context(desk: u128, session: u128, request: &str) -> ActionContext {
    ActionContext::operator(
        Uuid::from_u128(desk),
        Uuid::from_u128(5),
        Uuid::from_u128(session),
        ActionSource::Http,
    )
    .with_request_id(request)
}

fn envelope(
    context: ActionContext,
    command: SpeedGroupCommand,
) -> ActionEnvelope<SpeedGroupCommand> {
    ActionEnvelope { context, command }
}

struct FakePorts {
    state: Mutex<SpeedGroupPortState>,
    applies: Mutex<usize>,
    clock_reads: Mutex<usize>,
    application: Mutex<SpeedGroupApplication>,
    malform: Mutex<bool>,
    fail_non_authorization_calls: Mutex<bool>,
}

impl Default for FakePorts {
    fn default() -> Self {
        Self {
            state: Mutex::new(SpeedGroupPortState {
                groups: (1..=SPEED_GROUP_COUNT as u8)
                    .map(|one_based| SpeedGroupProjection {
                        group: group(one_based),
                        manual_bpm: 120.0,
                        paused: false,
                        speed_master_scale: 1.0,
                        synchronized_with: None,
                        phase_origin_millis: 0,
                    })
                    .collect(),
                manual_control_clean: (1..=SPEED_GROUP_COUNT as u8).map(group).collect(),
            }),
            applies: Mutex::new(0),
            clock_reads: Mutex::new(0),
            application: Mutex::new(SpeedGroupApplication::durable()),
            malform: Mutex::new(false),
            fail_non_authorization_calls: Mutex::new(false),
        }
    }
}

impl FakePorts {
    fn applies(&self) -> usize {
        *self.applies.lock()
    }

    fn clock_reads(&self) -> usize {
        *self.clock_reads.lock()
    }

    fn state(&self) -> SpeedGroupPortState {
        self.state.lock().clone()
    }

    fn update(&self, update: impl FnOnce(&mut SpeedGroupPortState)) {
        update(&mut self.state.lock());
    }

    fn persistence_pending(&self) {
        *self.application.lock() = SpeedGroupApplication {
            durability: SpeedGroupDurability::PersistencePending,
            warning: Some("persistence pending".into()),
        };
    }

    fn malform_next(&self) {
        *self.malform.lock() = true;
    }

    fn fail_non_authorization_calls(&self, fail: bool) {
        *self.fail_non_authorization_calls.lock() = fail;
    }
}

impl SpeedGroupPorts for FakePorts {
    fn state(&self, _context: &ActionContext) -> Result<SpeedGroupPortState, ActionError> {
        assert!(!*self.fail_non_authorization_calls.lock());
        Ok(self.state())
    }

    fn application_millis(&self, _context: &ActionContext) -> Result<u64, ActionError> {
        assert!(!*self.fail_non_authorization_calls.lock());
        let mut reads = self.clock_reads.lock();
        *reads += 1;
        Ok(10_000 + *reads as u64)
    }

    fn apply(
        &self,
        _context: &ActionContext,
        action: SpeedGroupResolvedAction,
    ) -> Result<SpeedGroupApplication, ActionError> {
        assert!(!*self.fail_non_authorization_calls.lock());
        *self.applies.lock() += 1;
        let mut state = self.state.lock();
        match action {
            SpeedGroupResolvedAction::SetManualBpm {
                group,
                bpm,
                applied_at_millis,
            } => apply_manual(&mut state, group, bpm, applied_at_millis),
            SpeedGroupResolvedAction::Synchronize {
                source,
                target,
                applied_at_millis: _,
            } => apply_sync(&mut state, source, target),
        }
        if *self.malform.lock() {
            state.groups[0].manual_bpm += 1.0;
        }
        Ok(self.application.lock().clone())
    }
}

fn apply_manual(state: &mut SpeedGroupPortState, group: SpeedGroupId, bpm: f64, now: u64) {
    if let Some(peer) = reciprocal(state, group) {
        state.groups[peer.index()].synchronized_with = None;
    }
    let projection = &mut state.groups[group.index()];
    projection.manual_bpm = bpm;
    projection.paused = false;
    projection.speed_master_scale = 1.0;
    projection.synchronized_with = None;
    projection.phase_origin_millis = now;
    mark_state_clean(state, group);
}

fn apply_sync(state: &mut SpeedGroupPortState, source: SpeedGroupId, target: SpeedGroupId) {
    for group in [source, target] {
        if let Some(peer) = reciprocal(state, group) {
            state.groups[peer.index()].synchronized_with = None;
        }
        state.groups[group.index()].synchronized_with = None;
    }
    let source_projection = state.groups[source.index()];
    for (group, peer) in [(source, target), (target, source)] {
        let projection = &mut state.groups[group.index()];
        projection.manual_bpm = source_projection.manual_bpm;
        projection.paused = source_projection.paused;
        projection.speed_master_scale = 1.0;
        projection.synchronized_with = Some(peer);
        projection.phase_origin_millis = source_projection.phase_origin_millis;
        mark_state_clean(state, group);
    }
}

fn reciprocal(state: &SpeedGroupPortState, group: SpeedGroupId) -> Option<SpeedGroupId> {
    let peer = state.groups[group.index()].synchronized_with?;
    (state.groups[peer.index()].synchronized_with == Some(group)).then_some(peer)
}

fn mark_state_clean(state: &mut SpeedGroupPortState, group: SpeedGroupId) {
    if !state.manual_control_clean.contains(&group) {
        state.manual_control_clean.push(group);
        state.manual_control_clean.sort_unstable();
    }
}
