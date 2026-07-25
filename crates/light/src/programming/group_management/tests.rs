use super::candidate::{PreparedGroupManagement, prepare_group_management};
use super::*;
use crate::active_show::{ActiveShowUnitOfWork, BackupIdentity, PreparedActiveShowTransaction};
use crate::{ActionErrorKind, ActiveShowPorts};
use light_core::FixtureId;
use light_show::{PortableShowCommit, PortableShowDocument, PortableShowTransaction, ShowStore};
use serde_json::{Value, json};
use std::path::PathBuf;
use uuid::Uuid;

#[test]
fn update_properties_changes_labels_and_retains_every_other_stored_field() {
    let members = [FixtureId::new(), FixtureId::new()];
    let mut body = group_body("house", "Old name", &members);
    body["future_extension"] = json!({"retain": [1, 2, 3]});
    body["programming"] = json!({"dimmer": {"kind": "normalized", "value": 0.5}});
    let rig = TestRig::new([("house", body)]);
    let commit = commit_for(
        &rig,
        "house",
        GroupManagementOperation::UpdateProperties(GroupPropertiesUpdate {
            name: "Front wash".into(),
            color: Some("#ff0000".into()),
            icon: Some("◆".into()),
        }),
    );

    let result = changed(&rig, &commit);

    assert_eq!(result.projection.raw_body["name"], "Front wash");
    assert_eq!(result.projection.raw_body["color"], "#ff0000");
    assert_eq!(result.projection.raw_body["icon"], "◆");
    assert_eq!(
        result.projection.raw_body["future_extension"],
        json!({"retain": [1, 2, 3]})
    );
    assert_eq!(
        result.projection.raw_body["programming"],
        json!({"dimmer": {"kind": "normalized", "value": 0.5}})
    );
    assert_eq!(result.projection.raw_body["fixtures"], json!(members));
    assert_eq!(result.projection.object_revision, 2);
    assert!(result.selection.is_none());
}

#[test]
fn identical_properties_are_a_no_change_without_an_event() {
    let rig = TestRig::new([("house", group_body("house", "Front wash", &[]))]);
    let commit = commit_for(
        &rig,
        "house",
        GroupManagementOperation::UpdateProperties(GroupPropertiesUpdate {
            name: "Front wash".into(),
            color: None,
            icon: None,
        }),
    );

    let prepared = prepare(&rig, &commit).unwrap();
    let PreparedActiveShowTransaction::NoChange(state) = prepared else {
        panic!("an identical property update must not prepare a commit")
    };

    assert!(!state.result.changed);
    assert_eq!(state.result.event_sequence, None);
    assert_eq!(state.result.projection.object_revision, 1);
}

#[test]
fn a_stored_empty_group_stays_stored_and_empty() {
    let rig = TestRig::new([("blackout", group_body("blackout", "Blackout", &[]))]);
    let commit = commit_for(
        &rig,
        "blackout",
        GroupManagementOperation::UpdateProperties(GroupPropertiesUpdate {
            name: "House out".into(),
            color: None,
            icon: None,
        }),
    );

    let result = changed(&rig, &commit);

    assert_eq!(result.projection.raw_body["fixtures"], json!([]));
    assert!(result.projection.raw_body.get("deleted").is_none());
}

#[test]
fn refresh_frozen_recaptures_ordered_membership_and_selects_the_source() {
    let members = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let mut frozen = group_body("frozen", "Frozen", &[members[0]]);
    frozen["frozen_from"] = json!({
        "source_group_id": "source",
        "source_revision": 1,
        "captured_at": "2020-01-01T00:00:00Z"
    });
    let rig = TestRig::new([
        ("source", group_body("source", "Source", &members)),
        ("frozen", frozen),
    ]);
    let commit = commit_for(
        &rig,
        "frozen",
        GroupManagementOperation::RefreshFrozen {
            expected_source: None,
        },
    );

    let result = changed(&rig, &commit);

    assert_eq!(result.projection.raw_body["fixtures"], json!(members));
    let stored = &result.projection.raw_body["frozen_from"];
    assert_eq!(stored["source_group_id"], "source");
    assert_eq!(stored["source_revision"], rig.document.revision().value());
    assert_ne!(stored["captured_at"], "2020-01-01T00:00:00Z");
    let selection = result.selection.as_ref().expect("frozen refresh selects");
    assert_eq!(selection.source_group_id, "source");
    assert_eq!(selection.fixtures, members.to_vec());
}

#[test]
fn refreshing_a_group_that_is_not_frozen_mutates_nothing() {
    let rig = TestRig::new([("plain", group_body("plain", "Plain", &[]))]);
    let commit = commit_for(
        &rig,
        "plain",
        GroupManagementOperation::RefreshFrozen {
            expected_source: None,
        },
    );

    let error = error_for(&rig, &commit);

    assert_eq!(error.kind, ActionErrorKind::Invalid);
}

#[test]
fn a_missing_frozen_source_fails_before_any_mutation() {
    let mut frozen = group_body("frozen", "Frozen", &[]);
    frozen["frozen_from"] = json!({
        "source_group_id": "gone",
        "source_revision": 1,
        "captured_at": "2020-01-01T00:00:00Z"
    });
    let rig = TestRig::new([("frozen", frozen)]);
    let commit = commit_for(
        &rig,
        "frozen",
        GroupManagementOperation::RefreshFrozen {
            expected_source: None,
        },
    );

    let error = error_for(&rig, &commit);

    assert_eq!(error.kind, ActionErrorKind::NotFound);
}

#[test]
fn a_stale_declared_source_revision_conflicts_without_mutating() {
    let mut frozen = group_body("frozen", "Frozen", &[]);
    frozen["frozen_from"] = json!({
        "source_group_id": "source",
        "source_revision": 1,
        "captured_at": "2020-01-01T00:00:00Z"
    });
    let rig = TestRig::new([
        ("source", group_body("source", "Source", &[])),
        ("frozen", frozen),
    ]);
    let commit = commit_for(
        &rig,
        "frozen",
        GroupManagementOperation::RefreshFrozen {
            expected_source: Some(GroupSourceExpectation {
                source_group_id: "source".into(),
                expected_source_revision: Some(99),
            }),
        },
    );

    let error = error_for(&rig, &commit);

    assert_eq!(error.kind, ActionErrorKind::Conflict);
}

#[test]
fn detach_derived_freezes_the_resolved_membership_and_keeps_frozen_metadata() {
    let members = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let mut derived = group_body("derived", "Derived", &[]);
    derived["derived_from"] = json!({"source_group_id": "source", "rule": {"type": "odd"}});
    derived["frozen_from"] = json!({
        "source_group_id": "other",
        "source_revision": 4,
        "captured_at": "2020-01-01T00:00:00Z"
    });
    let rig = TestRig::new([
        ("source", group_body("source", "Source", &members)),
        ("derived", derived),
    ]);
    let commit = commit_for(
        &rig,
        "derived",
        GroupManagementOperation::DetachDerived {
            expected_source: None,
        },
    );

    let result = changed(&rig, &commit);

    assert_eq!(result.projection.raw_body["derived_from"], Value::Null);
    assert_eq!(
        result.projection.raw_body["fixtures"],
        json!([members[0], members[2]])
    );
    assert_eq!(
        result.projection.raw_body["frozen_from"]["source_group_id"],
        "other"
    );
    assert!(result.selection.is_none());
}

#[test]
fn detaching_a_group_that_is_not_derived_mutates_nothing() {
    let rig = TestRig::new([("plain", group_body("plain", "Plain", &[]))]);
    let commit = commit_for(
        &rig,
        "plain",
        GroupManagementOperation::DetachDerived {
            expected_source: None,
        },
    );

    let error = error_for(&rig, &commit);

    assert_eq!(error.kind, ActionErrorKind::Invalid);
}

#[test]
fn a_stale_object_revision_conflicts_and_reports_the_current_revision() {
    let rig = TestRig::new([("house", group_body("house", "House", &[]))]);
    let mut commit = commit_for(&rig, "house", rename("Renamed"));
    commit.expected_object_revision = 7;

    let error = error_for(&rig, &commit);

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(1));
}

#[test]
fn a_stale_show_revision_conflicts_before_reading_the_group() {
    let rig = TestRig::new([("house", group_body("house", "House", &[]))]);
    let mut commit = commit_for(&rig, "house", rename("Renamed"));
    commit.expected_show_revision = Some(light_show::PortableShowRevision::from_value(
        rig.document.revision().value() + 40,
    ));

    let error = error_for(&rig, &commit);

    assert_eq!(error.kind, ActionErrorKind::Conflict);
}

#[test]
fn undo_restores_the_exact_previous_stored_body() {
    let member = FixtureId::new();
    let mut original = group_body("house", "Original", &[member]);
    original["future_extension"] = json!({"retain": true});
    let rig = TestRig::new([("house", original.clone())]);
    rig.store
        .put_object("group", "house", &group_body("house", "Renamed", &[]), 1)
        .unwrap();
    let rig = rig.reloaded();
    let commit = commit_for(&rig, "house", GroupManagementOperation::Undo);

    let result = changed(&rig, &commit);

    assert_eq!(result.projection.raw_body.as_ref(), &original);
    assert_eq!(result.projection.object_revision, 3);
    assert!(result.selection.is_none());
}

fn rename(name: &str) -> GroupManagementOperation {
    GroupManagementOperation::UpdateProperties(GroupPropertiesUpdate {
        name: name.to_owned(),
        color: None,
        icon: None,
    })
}

fn group_body(id: &str, name: &str, fixtures: &[FixtureId]) -> Value {
    json!({
        "id": id,
        "name": name,
        "color": null,
        "icon": null,
        "fixtures": fixtures,
        "derived_from": null,
        "frozen_from": null,
        "programming": {},
        "master": 1.0,
        "playback_fader": null
    })
}

fn commit_for(
    rig: &TestRig,
    group_id: &str,
    operation: GroupManagementOperation,
) -> GroupManagementCommit {
    GroupManagementCommit::new(&GroupManagementRequest {
        show_id: rig.document.id(),
        group_id: group_id.to_owned(),
        operation,
        expected_object_revision: rig
            .document
            .object("group", group_id)
            .map_or(0, light_show::PortableShowObject::revision),
        expected_show_revision: None,
    })
}

fn prepare(
    rig: &TestRig,
    commit: &GroupManagementCommit,
) -> Result<PreparedActiveShowTransaction<PreparedGroupManagement>, ActionError> {
    let ports = TestPorts {
        document: rig.document.clone(),
        undo: rig
            .store
            .prepare_object_undo("group", &commit.group_id, commit.expected_object_revision)
            .ok(),
    };
    let unit = TestUnit {
        document: rig.document.clone(),
    };
    prepare_group_management(&ports, &unit, commit)
}

fn error_for(rig: &TestRig, commit: &GroupManagementCommit) -> ActionError {
    match prepare(rig, commit) {
        Ok(_) => panic!("expected the Group action to fail before mutating"),
        Err(error) => error,
    }
}

fn changed(rig: &TestRig, commit: &GroupManagementCommit) -> GroupManagementCommitResult {
    match prepare(rig, commit).unwrap() {
        PreparedActiveShowTransaction::PreparedCommit { state, .. } => state.result,
        PreparedActiveShowTransaction::NoChange(_) => panic!("expected a prepared Group mutation"),
    }
}

struct TestRig {
    path: PathBuf,
    store: ShowStore,
    document: PortableShowDocument,
}

impl TestRig {
    fn new<const N: usize>(objects: [(&str, Value); N]) -> Self {
        let path =
            std::env::temp_dir().join(format!("light-group-management-{}.sqlite", Uuid::new_v4()));
        let (store, _) = ShowStore::create(&path, "Group management test").unwrap();
        for (id, body) in objects {
            store.put_object("group", id, &body, 0).unwrap();
        }
        let document = store.portable_document().unwrap();
        Self {
            path,
            store,
            document,
        }
    }

    fn reloaded(mut self) -> Self {
        self.document = self.store.portable_document().unwrap();
        self
    }
}

impl Drop for TestRig {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.path.display(), suffix));
        }
    }
}

struct TestUnit {
    document: PortableShowDocument,
}

impl ActiveShowUnitOfWork for TestUnit {
    fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    fn backup(&mut self, _identity: &BackupIdentity) -> Result<(), ActionError> {
        unreachable!("candidate preparation never commits")
    }

    fn commit(
        &mut self,
        _transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        unreachable!("candidate preparation never commits")
    }
}

/// The adapter-owned history read is captured up front because a SQLite connection is not `Sync`.
struct TestPorts {
    document: PortableShowDocument,
    undo: Option<light_show::PortableShowObjectUndo>,
}

impl ActiveShowPorts for TestPorts {
    type UnitOfWork = TestUnit;
    type PreparedRuntime = ();

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        _show_id: light_core::ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        Ok(TestUnit {
            document: self.document.clone(),
        })
    }

    fn prepare_object_undo(
        &self,
        _unit: &Self::UnitOfWork,
        _kind: &str,
        object_id: &str,
        _expected_object_revision: light_core::Revision,
    ) -> Result<light_show::PortableShowObjectUndo, ActionError> {
        self.undo.clone().ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::NotFound,
                format!("Group {object_id} has no undo history"),
            )
        })
    }

    fn prepare_runtime(
        &self,
        _snapshot: light_engine::EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        unreachable!("candidate preparation never installs runtime")
    }

    fn install_runtime(&self, _context: &ActionContext, _prepared: Self::PreparedRuntime) {
        unreachable!("candidate preparation never installs runtime")
    }
}

impl GroupManagementActiveShowPorts for TestPorts {
    fn apply_frozen_group_selection(
        &self,
        _context: &ActionContext,
        _selection: &GroupManagementSelection,
    ) {
    }
}
