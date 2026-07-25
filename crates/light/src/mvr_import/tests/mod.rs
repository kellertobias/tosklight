mod support;

use super::*;
use crate::{ActionErrorKind, ApplicationEvent, EventFilter, EventReplay, ShowEvent};
use light_core::FixtureId;
use light_fixture::PortablePatchedFixtureRecord;
use std::sync::atomic::Ordering;
use support::*;
use uuid::Uuid;

#[test]
fn resolved_and_unresolved_fixtures_commit_once_and_install_the_exact_runtime() {
    let rig = Rig::new();
    let resolved = mvr_fixture(Uuid::from_u128(1), "Resolved", 1, 1);
    let mut unresolved = mvr_fixture(Uuid::from_u128(2), "Unresolved", 1, 20);
    unresolved.gdtf_spec = "Unavailable.gdtf".into();

    let result = rig
        .service
        .apply(
            rig.envelope(vec![resolved, unresolved], vec![fixture_definition(1)]),
            &rig.ports,
        )
        .unwrap();

    assert!(result.changed);
    assert_eq!(result.show_revision.value(), 1);
    assert_eq!(result.patch_revision.value(), 1);
    assert_eq!(result.imported_fixtures, 1);
    assert_eq!(result.unresolved_fixtures, 1);
    assert_eq!(result.event_sequence, Some(1));
    assert_eq!(result.change.fixtures.len(), 1);
    assert_eq!(result.change.fixtures[0].fixture_revision, 1);
    assert_eq!(result.change.profile_revisions.len(), 1);
    assert!(result.warnings[0].contains("Unavailable.gdtf"));

    let document = rig.document();
    assert_eq!(document.revision().value(), 1);
    assert_eq!(document.patch_revision().value(), 1);
    assert_eq!(document.objects_of_kind("patched_fixture").count(), 1);
    assert_eq!(document.objects_of_kind("mvr_fixture").count(), 1);
    assert_eq!(
        document.objects_of_kind("unresolved_mvr_fixture").count(),
        1
    );
    let stored_fixture = document.objects_of_kind("patched_fixture").next().unwrap();
    let record = PortablePatchedFixtureRecord::decode(stored_fixture.body().clone()).unwrap();
    let stored_profile = record.selected_profile_reference().unwrap().unwrap();
    assert_eq!(result.change.fixtures[0].profile, stored_profile);
    let profile = document
        .fixture_profile_revision(stored_profile.profile_id, stored_profile.profile_revision)
        .unwrap();
    assert_eq!(
        result.change.profile_revisions[0].content_digest,
        profile.digest().as_str()
    );
    let installed = rig.ports.installed.lock();
    let installed = installed.as_ref().unwrap();
    assert_eq!(installed.revision, document.revision().value());
    assert_eq!(installed.fixtures.len(), 1);
    assert_eq!(count(&rig.ports.counters.backups), 1);
    assert_eq!(count(&rig.ports.counters.commits), 1);
    assert_eq!(count(&rig.ports.counters.runtime_prepares), 1);
    assert_eq!(count(&rig.ports.counters.runtime_installs), 1);
    assert_eq!(count(&rig.ports.counters.reconciles), 1);

    let EventReplay::Events(events) = rig.events.replay(0, &EventFilter::default()) else {
        panic!("MVR commit event should be retained");
    };
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].payload,
        ApplicationEvent::Show(ShowEvent::PatchChanged(_))
    ));
}

#[test]
fn stale_prepared_import_cannot_overwrite_a_newer_show_revision() {
    let rig = Rig::new();
    let prepared = rig
        .service
        .prepare(
            rig.envelope(
                vec![mvr_fixture(Uuid::from_u128(10), "Prepared", 1, 1)],
                vec![fixture_definition(1)],
            ),
            &rig.ports,
        )
        .unwrap();
    let store = rig.ports.store();
    let document = store.portable_document().unwrap();
    let mut competing = document.transaction();
    competing.put("future_object", "newer", serde_json::json!({"kept":true}));
    store.apply_portable_transaction(competing).unwrap();

    let error = rig.service.commit(prepared, &rig.ports).unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(1));
    let document = rig.document();
    assert!(document.object("future_object", "newer").is_some());
    assert_eq!(document.objects_of_kind("patched_fixture").count(), 0);
    assert_eq!(count(&rig.ports.counters.backups), 0);
    assert_eq!(count(&rig.ports.counters.commits), 0);
    assert_eq!(count(&rig.ports.counters.runtime_prepares), 0);
    assert_eq!(count(&rig.ports.counters.runtime_installs), 0);
    assert_eq!(rig.events.latest_sequence(), 0);
}

#[test]
fn runtime_preparation_failure_leaves_persistence_and_live_runtime_unchanged() {
    let rig = Rig::new();
    let prepared = rig
        .service
        .prepare(
            rig.envelope(
                vec![mvr_fixture(Uuid::from_u128(20), "Rejected", 1, 1)],
                vec![fixture_definition(1)],
            ),
            &rig.ports,
        )
        .unwrap();
    rig.ports.fail_runtime.store(true, Ordering::Relaxed);

    let error = rig.service.commit(prepared, &rig.ports).unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(rig.document().revision().value(), 0);
    assert_eq!(rig.document().objects_of_kind("patched_fixture").count(), 0);
    assert_eq!(count(&rig.ports.counters.runtime_prepares), 1);
    assert_eq!(count(&rig.ports.counters.backups), 0);
    assert_eq!(count(&rig.ports.counters.commits), 0);
    assert_eq!(count(&rig.ports.counters.runtime_installs), 0);
    assert!(rig.ports.installed.lock().is_none());
    assert_eq!(rig.events.latest_sequence(), 0);
}

#[test]
fn backup_failure_leaves_persistence_and_live_runtime_unchanged() {
    let rig = Rig::new();
    rig.ports.fail_backup.store(true, Ordering::Relaxed);

    let error = rig
        .service
        .apply(
            rig.envelope(
                vec![mvr_fixture(Uuid::from_u128(21), "Rejected", 1, 1)],
                vec![fixture_definition(1)],
            ),
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Unavailable);
    assert_eq!(rig.document().revision().value(), 0);
    assert_eq!(count(&rig.ports.counters.runtime_prepares), 1);
    assert_eq!(count(&rig.ports.counters.backups), 1);
    assert_eq!(count(&rig.ports.counters.commits), 0);
    assert_eq!(count(&rig.ports.counters.runtime_installs), 0);
    assert!(rig.ports.installed.lock().is_none());
    assert_eq!(rig.events.latest_sequence(), 0);
}

#[test]
fn replace_and_reimport_preserve_mvr_identity_and_move_in_black_settings() {
    let rig = Rig::new();
    let definition = fixture_definition(1);
    let retained_id = FixtureId(Uuid::from_u128(301));
    let replaced_id = FixtureId(Uuid::from_u128(302));
    let retained_source = Uuid::from_u128(303);
    let store = rig.ports.store();
    store
        .put_object(
            "patched_fixture",
            &retained_id.0.to_string(),
            &serde_json::to_value(stored_fixture(
                retained_id,
                definition.clone(),
                10,
                (false, 750),
            ))
            .unwrap(),
            0,
        )
        .unwrap();
    store
        .put_object(
            "mvr_fixture",
            &retained_source.to_string(),
            &serde_json::json!({"fixture_id":retained_id.0.to_string()}),
            0,
        )
        .unwrap();
    store
        .put_object(
            "patched_fixture",
            &replaced_id.0.to_string(),
            &serde_json::to_value(stored_fixture(
                replaced_id,
                definition.clone(),
                20,
                (true, 0),
            ))
            .unwrap(),
            0,
        )
        .unwrap();
    let mut envelope = rig.envelope(
        vec![
            mvr_fixture(retained_source, "Retained", 1, 10),
            mvr_fixture(Uuid::from_u128(304), "Replacement", 1, 20),
        ],
        vec![definition],
    );
    envelope
        .command
        .resolutions
        .insert(Uuid::from_u128(304), MvrImportResolution::Replace);

    let result = rig.service.apply(envelope, &rig.ports).unwrap();

    assert_eq!(result.imported_fixtures, 2);
    assert_eq!(result.change.removed_fixture_ids, vec![replaced_id]);
    let retained = result
        .change
        .fixtures
        .iter()
        .find(|fixture| fixture.patch.fixture_id == retained_id)
        .unwrap();
    assert!(!retained.patch.move_in_black_enabled);
    assert_eq!(retained.patch.move_in_black_delay_millis, 750);
    let document = rig.document();
    assert!(
        document
            .object("patched_fixture", &retained_id.0.to_string())
            .is_some()
    );
    assert!(
        document
            .object("patched_fixture", &replaced_id.0.to_string())
            .is_none()
    );
    assert_eq!(count(&rig.ports.counters.commits), 1);
}

#[test]
fn all_skipped_import_is_a_read_only_noop_without_backup_runtime_or_event() {
    let rig = Rig::new();
    let source_id = Uuid::from_u128(401);
    let mut envelope = rig.envelope(
        vec![mvr_fixture(source_id, "Skipped", 1, 1)],
        vec![fixture_definition(1)],
    );
    envelope
        .command
        .resolutions
        .insert(source_id, MvrImportResolution::Skip);

    let result = rig.service.apply(envelope, &rig.ports).unwrap();

    assert!(!result.changed);
    assert_eq!(result.event_sequence, None);
    assert_eq!(result.show_revision.value(), 0);
    assert_eq!(count(&rig.ports.counters.backups), 0);
    assert_eq!(count(&rig.ports.counters.commits), 0);
    assert_eq!(count(&rig.ports.counters.runtime_prepares), 0);
    assert_eq!(count(&rig.ports.counters.runtime_installs), 0);
    assert_eq!(rig.events.latest_sequence(), 0);
}
