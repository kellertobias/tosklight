mod support;

use crate::ActionErrorKind;
use light_show::FixtureProfileRevision;
use serde_json::json;
use support::{CounterSnapshot, FailurePoint, TestRig, envelope, patch_batch, profile_with_modes};
use uuid::Uuid;

#[test]
fn large_shared_profile_batch_reads_and_mutates_each_boundary_once() {
    let (profile, reference) = profile_with_modes(2_000);
    let rig = TestRig::new(profile, FailurePoint::None);
    let request = envelope(
        patch_batch(rig.ports.show_id(), reference, 100),
        "large-batch",
        0,
    );

    let result = rig.service.handle(request, &rig.ports).unwrap();

    assert!(!result.replayed);
    assert_eq!(result.change.show_revision.value(), 1);
    assert_eq!(result.change.patch_revision.value(), 1);
    assert!(result.changed);
    assert_eq!(result.event_sequence, Some(1));
    assert_eq!(result.change.fixtures.len(), 100);
    assert_eq!(result.change.profile_revisions.len(), 1);
    assert_eq!(rig.service.events().latest_sequence(), 1);
    assert_eq!(rig.counters(), successful_counts(100));
    rig.assert_portable_patch(100, 1);
}

#[test]
fn stale_patch_revision_stops_before_profile_resolution_or_side_effects() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::None);
    let request = envelope(patch_batch(rig.ports.show_id(), reference, 1), "stale", 1);

    let error = rig.service.handle(request, &rig.ports).unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.message, "stale patch revision");
    assert_eq!(error.current_revision, Some(0));
    assert_eq!(rig.counters(), begun_only_counts());
    assert_eq!(rig.service.events().latest_sequence(), 0);
    rig.assert_empty_show();
}

#[test]
fn unrelated_show_mutation_does_not_stale_the_patch_revision() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::None);
    rig.seed_unrelated_group();
    let before = rig.portable_document();
    assert_eq!(before.revision().value(), 1);
    assert_eq!(before.patch_revision().value(), 0);

    let result = rig
        .service
        .handle(
            envelope(
                patch_batch(rig.ports.show_id(), reference, 1),
                "patch-after-group",
                0,
            ),
            &rig.ports,
        )
        .unwrap();

    assert_eq!(result.change.show_revision.value(), 2);
    assert_eq!(result.change.patch_revision.value(), 1);
    assert_eq!(rig.counters(), successful_counts(1));
    rig.assert_portable_patch(1, 1);
    assert!(
        rig.portable_document()
            .object("group", "unrelated")
            .is_some()
    );
}

#[test]
fn invalid_complete_patch_stops_before_backup_and_commit() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::None);
    let mut command = patch_batch(rig.ports.show_id(), reference, 2);
    command.fixtures[1].patch.universe = Some(1);
    command.fixtures[1].patch.address = Some(1);
    command.fixtures[1].patch.split_patches[0].universe = Some(1);
    command.fixtures[1].patch.split_patches[0].address = Some(1);

    let error = rig
        .service
        .handle(envelope(command, "overlap", 0), &rig.ports)
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert!(error.message.contains("patch overlap"));
    assert_eq!(
        rig.counters(),
        CounterSnapshot {
            active_show_begins: 1,
            library_reads: 1,
            runtime_prepares: 1,
            ..CounterSnapshot::default()
        }
    );
    assert_eq!(rig.service.events().latest_sequence(), 0);
    rig.assert_empty_show();
}

#[test]
fn backup_failure_prevents_commit_and_every_downstream_effect() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::Backup);
    let request = envelope(
        patch_batch(rig.ports.show_id(), reference, 1),
        "backup-failure",
        0,
    );

    let error = rig.service.handle(request, &rig.ports).unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Unavailable);
    assert_eq!(
        rig.counters(),
        CounterSnapshot {
            active_show_begins: 1,
            library_reads: 1,
            runtime_prepares: 1,
            backups: 1,
            ..CounterSnapshot::default()
        }
    );
    assert_eq!(rig.service.events().latest_sequence(), 0);
    rig.assert_empty_show();
}

#[test]
fn commit_failure_prevents_install_reconcile_and_event_publication() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::Commit);
    let request = envelope(
        patch_batch(rig.ports.show_id(), reference, 1),
        "commit-failure",
        0,
    );

    let error = rig.service.handle(request, &rig.ports).unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Internal);
    assert_eq!(
        rig.counters(),
        CounterSnapshot {
            active_show_begins: 1,
            library_reads: 1,
            runtime_prepares: 1,
            backups: 1,
            commits: 1,
            ..CounterSnapshot::default()
        }
    );
    assert_eq!(rig.service.events().latest_sequence(), 0);
    rig.assert_empty_show();
}

#[test]
fn exact_retry_reuses_committed_revisions_and_event_without_side_effects() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::None);
    let request = envelope(patch_batch(rig.ports.show_id(), reference, 1), "retry", 0);

    let first = rig.service.handle(request.clone(), &rig.ports).unwrap();
    let retry = rig.service.handle(request, &rig.ports).unwrap();

    let mut expected_retry = first.clone();
    expected_retry.replayed = true;
    assert_eq!(retry, expected_retry);
    assert_eq!(retry.change.show_revision, first.change.show_revision);
    assert_eq!(retry.change.patch_revision, first.change.patch_revision);
    assert_eq!(retry.event_sequence, first.event_sequence);
    assert_eq!(rig.counters(), successful_counts(1));
    assert_eq!(rig.service.events().latest_sequence(), 1);
    rig.assert_portable_patch(1, 1);
}

#[test]
fn reused_request_id_with_different_command_is_a_conflict() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::None);
    let command = patch_batch(rig.ports.show_id(), reference, 1);
    let first = envelope(command.clone(), "collision", 0);
    rig.service.handle(first, &rig.ports).unwrap();
    let mut changed = command;
    changed.fixtures[0].patch.name = "Different operation".into();

    let error = rig
        .service
        .handle(envelope(changed, "collision", 0), &rig.ports)
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert!(error.message.contains("request_id was already used"));
    assert_eq!(rig.counters(), successful_counts(1));
    assert_eq!(rig.service.events().latest_sequence(), 1);
    rig.assert_portable_patch(1, 1);
}

#[test]
fn removal_uses_the_same_atomic_compile_and_event_path() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::None);
    let add = patch_batch(rig.ports.show_id(), reference, 1);
    let fixture_id = add.fixtures[0].patch.fixture_id;
    rig.service
        .handle(envelope(add, "add-before-remove", 0), &rig.ports)
        .unwrap();
    let remove = crate::PatchFixturesCommand {
        show_id: rig.ports.show_id(),
        fixtures: Vec::new(),
        remove_fixture_ids: vec![fixture_id],
    };

    let result = rig
        .service
        .handle(envelope(remove, "remove", 1), &rig.ports)
        .unwrap();

    assert!(result.change.fixtures.is_empty());
    assert_eq!(result.change.removed_fixture_ids, vec![fixture_id]);
    assert_eq!(result.change.show_revision.value(), 2);
    assert_eq!(result.change.patch_revision.value(), 2);
    assert!(result.changed);
    assert_eq!(result.event_sequence, Some(2));
    assert_eq!(rig.service.events().latest_sequence(), 2);
    assert_eq!(
        rig.counters(),
        CounterSnapshot {
            active_show_begins: 2,
            library_reads: 1,
            catalog_reads: 0,
            runtime_prepares: 2,
            backups: 2,
            commits: 2,
            written_fixtures: 1,
            written_profiles: 1,
            runtime_installs: 2,
            reconciliations: 2,
        }
    );
    rig.assert_portable_patch(0, 1);
}

#[test]
fn removing_an_already_absent_fixture_is_an_idempotent_noop() {
    let (profile, _) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::None);
    let remove = crate::PatchFixturesCommand {
        show_id: rig.ports.show_id(),
        fixtures: Vec::new(),
        remove_fixture_ids: vec![light_core::FixtureId::new()],
    };

    let result = rig
        .service
        .handle(envelope(remove, "remove-absent", 0), &rig.ports)
        .unwrap();

    assert!(!result.changed);
    assert!(result.change.fixtures.is_empty());
    assert!(result.change.removed_fixture_ids.is_empty());
    assert_eq!(result.change.show_revision.value(), 0);
    assert_eq!(result.change.patch_revision.value(), 0);
    assert_eq!(result.event_sequence, None);
    assert_eq!(rig.counters(), begun_only_counts());
    assert_eq!(rig.service.events().latest_sequence(), 0);
    rig.assert_empty_show();
}

#[test]
fn identical_desired_patch_is_a_cached_noop_without_side_effects() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::None);
    let command = patch_batch(rig.ports.show_id(), reference, 1);
    rig.service
        .handle(
            envelope(command.clone(), "first-desired-state", 0),
            &rig.ports,
        )
        .unwrap();

    let noop = rig
        .service
        .handle(envelope(command, "same-desired-state", 1), &rig.ports)
        .unwrap();

    assert!(!noop.changed);
    assert!(noop.change.fixtures.is_empty());
    assert!(noop.change.removed_fixture_ids.is_empty());
    assert!(noop.change.profile_revisions.is_empty());
    assert_eq!(noop.change.show_revision.value(), 1);
    assert_eq!(noop.change.patch_revision.value(), 1);
    assert_eq!(noop.event_sequence, None);
    assert_eq!(rig.service.events().latest_sequence(), 1);
    assert_eq!(
        rig.counters(),
        CounterSnapshot {
            active_show_begins: 2,
            library_reads: 1,
            catalog_reads: 0,
            runtime_prepares: 1,
            backups: 1,
            commits: 1,
            written_fixtures: 1,
            written_profiles: 1,
            runtime_installs: 1,
            reconciliations: 1,
        }
    );
}

#[test]
fn snapshot_is_authoritative_and_captures_the_patch_event_cursor() {
    let (profile, reference) = profile_with_modes(2_000);
    let rig = TestRig::new(profile, FailurePoint::None);
    let command = patch_batch(rig.ports.show_id(), reference, 2);
    let context = envelope(command.clone(), "snapshot-add", 0).context;
    rig.service
        .handle(envelope(command, "snapshot-add", 0), &rig.ports)
        .unwrap();

    let snapshot = rig
        .service
        .snapshot(&context, rig.ports.show_id(), &rig.ports)
        .unwrap();

    assert_eq!(snapshot.show_revision.value(), 1);
    assert_eq!(snapshot.patch_revision.value(), 1);
    assert_eq!(snapshot.event_sequence, 1);
    assert_eq!(snapshot.fixtures.len(), 2);
    assert_eq!(snapshot.profile_revisions.len(), 1);
    assert_eq!(snapshot.profile_revisions[0].referenced_modes.len(), 1);
    assert_eq!(rig.counters().active_show_begins, 2);
    assert_eq!(rig.counters().library_reads, 1);
}

#[test]
fn shared_show_event_reaches_desks_only_when_the_show_object_is_subscribed() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile, FailurePoint::None);
    let patch_object = crate::EventObject::new(
        crate::EventCapability::Show,
        format!("patch:{}", rig.ports.show_id().0),
    );
    let subscribe = |desk_id| {
        rig.service.events().subscribe(
            crate::EventFilter::for_desk(desk_id).with_object(patch_object.clone()),
            crate::SubscriptionOptions::default(),
        )
    };
    let first_desk = subscribe(Uuid::from_u128(1));
    let second_desk = subscribe(Uuid::from_u128(2));
    let other_show = rig.service.events().subscribe(
        crate::EventFilter::for_desk(Uuid::from_u128(1)).with_object(crate::EventObject::new(
            crate::EventCapability::Show,
            format!("patch:{}", Uuid::from_u128(999)),
        )),
        crate::SubscriptionOptions::default(),
    );

    rig.service
        .handle(
            envelope(
                patch_batch(rig.ports.show_id(), reference, 1),
                "shared-show-event",
                0,
            ),
            &rig.ports,
        )
        .unwrap();

    for subscription in [&first_desk, &second_desk] {
        assert!(matches!(
            subscription.try_next(),
            Some(crate::SubscriptionDelivery::Event(event)) if event.sequence == 1
        ));
    }
    assert!(other_show.try_next().is_none());
}

#[test]
fn same_reference_legacy_edit_materializes_inline_profile_without_library_read() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile.clone(), FailurePoint::None);
    let mut command = patch_batch(rig.ports.show_id(), reference, 1);
    rig.seed_legacy_fixture(&profile, reference, &command.fixtures[0].patch);
    command.fixtures[0].patch.name = "Edited legacy fixture".into();

    let result = rig
        .service
        .handle(envelope(command, "legacy-same-ref", 0), &rig.ports)
        .unwrap();

    assert_eq!(result.change.show_revision.value(), 2);
    assert_eq!(rig.counters().library_reads, 0);
    assert_eq!(rig.counters().written_profiles, 1);
    rig.assert_portable_patch(1, 1);
}

#[test]
fn legacy_object_keys_remain_readable_and_are_canonicalized_when_touched() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile.clone(), FailurePoint::None);
    let mut command = patch_batch(rig.ports.show_id(), reference, 1);
    let fixture_id = command.fixtures[0].patch.fixture_id;
    rig.seed_legacy_fixture_as(&profile, reference, &command.fixtures[0].patch, "dimmer");
    let context = envelope(command.clone(), "legacy-key-snapshot", 0).context;

    let snapshot = rig
        .service
        .snapshot(&context, rig.ports.show_id(), &rig.ports)
        .unwrap();
    assert_eq!(snapshot.fixtures[0].patch.fixture_id, fixture_id);

    command.fixtures[0].patch.name = "Touched legacy fixture".into();
    rig.service
        .handle(envelope(command, "legacy-key-update", 0), &rig.ports)
        .unwrap();

    let document = rig.portable_document();
    assert!(document.object("patched_fixture", "dimmer").is_none());
    let canonical = document
        .object("patched_fixture", &fixture_id.0.to_string())
        .unwrap();
    assert!(canonical.body().get("definition").is_none());
}

#[test]
fn patching_cannot_silently_migrate_an_unrelated_legacy_fixture() {
    let (profile, reference) = profile_with_modes(1);
    let rig = TestRig::new(profile.clone(), FailurePoint::None);
    let mut command = patch_batch(rig.ports.show_id(), reference, 2);
    for fixture in &command.fixtures {
        rig.seed_legacy_fixture(&profile, reference, &fixture.patch);
    }
    let unrelated_id = command.fixtures[1].patch.fixture_id;
    command.fixtures.truncate(1);
    command.fixtures[0].patch.name = "Explicitly edited fixture".into();

    let error = rig
        .service
        .handle(envelope(command, "scoped-migration", 0), &rig.ports)
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Unavailable);
    assert_eq!(error.current_revision, Some(0));
    assert_eq!(rig.counters(), begun_only_counts());
    assert_eq!(rig.service.events().latest_sequence(), 0);
    let document = rig.portable_document();
    let unrelated = document
        .object("patched_fixture", &unrelated_id.0.to_string())
        .unwrap();
    assert!(unrelated.body().get("definition").is_some());
    assert!(document.fixture_profile_revisions().is_empty());
}

#[test]
fn materialized_inline_profile_retains_unknown_raw_fields() {
    let (profile, reference) = profile_with_modes(1);
    let profile = profile_with_unknown_field(&profile);
    let rig = TestRig::new(profile.clone(), FailurePoint::None);
    let command = patch_batch(rig.ports.show_id(), reference, 1);
    rig.seed_legacy_fixture(&profile, reference, &command.fixtures[0].patch);

    rig.service
        .handle(envelope(command, "legacy-unknown", 0), &rig.ports)
        .unwrap();

    let document = rig.portable_document();
    let stored = document
        .fixture_profile_revision(reference.profile_id, reference.profile_revision)
        .unwrap();
    assert_eq!(
        stored.profile()["future_profile_data"],
        json!({"retain": [1, 2, 3]})
    );
    assert_eq!(stored.profile(), profile.profile());
}

#[test]
fn profile_upgrade_stages_exact_old_inline_revision_with_new_revision() {
    let (old_profile, old_reference) = profile_with_modes(1);
    let old_profile = profile_with_unknown_field(&old_profile);
    let (new_profile, new_reference) = profile_with_modes(1);
    let rig = TestRig::new(new_profile.clone(), FailurePoint::None);
    let old_command = patch_batch(rig.ports.show_id(), old_reference, 1);
    rig.seed_legacy_fixture(&old_profile, old_reference, &old_command.fixtures[0].patch);
    let command = patch_batch(rig.ports.show_id(), new_reference, 1);

    rig.service
        .handle(envelope(command, "legacy-upgrade", 0), &rig.ports)
        .unwrap();

    let document = rig.portable_document();
    let stored_old = document
        .fixture_profile_revision(old_reference.profile_id, old_reference.profile_revision)
        .unwrap();
    let stored_new = document
        .fixture_profile_revision(new_reference.profile_id, new_reference.profile_revision)
        .unwrap();
    assert_eq!(stored_old.profile(), old_profile.profile());
    assert_eq!(stored_new.profile(), new_profile.profile());
    assert_eq!(rig.counters().library_reads, 1);
    assert_eq!(rig.counters().written_profiles, 2);
    rig.assert_portable_patch(1, 2);
}

#[test]
fn conflicting_inline_content_for_one_revision_is_rejected_before_library_or_backup() {
    let (profile, reference) = profile_with_modes(1);
    let conflicting = profile_with_unknown_field(&profile);
    let rig = TestRig::new(profile.clone(), FailurePoint::None);
    let command = patch_batch(rig.ports.show_id(), reference, 2);
    rig.seed_legacy_fixture(&profile, reference, &command.fixtures[0].patch);
    rig.seed_legacy_fixture(&conflicting, reference, &command.fixtures[1].patch);

    let error = rig
        .service
        .handle(envelope(command, "legacy-conflict", 0), &rig.ports)
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert!(error.message.contains("conflicting inline content digests"));
    assert_eq!(rig.counters(), begun_only_counts());
    assert_eq!(rig.service.events().latest_sequence(), 0);
    assert!(
        rig.portable_document()
            .fixture_profile_revisions()
            .is_empty()
    );
}

fn profile_with_unknown_field(profile: &FixtureProfileRevision) -> FixtureProfileRevision {
    let mut raw = profile.profile().clone();
    raw["future_profile_data"] = json!({"retain": [1, 2, 3]});
    FixtureProfileRevision::from_profile(raw).unwrap()
}

fn successful_counts(fixtures: usize) -> CounterSnapshot {
    CounterSnapshot {
        active_show_begins: 1,
        library_reads: 1,
        catalog_reads: 0,
        runtime_prepares: 1,
        backups: 1,
        commits: 1,
        written_fixtures: fixtures,
        written_profiles: 1,
        runtime_installs: 1,
        reconciliations: 1,
    }
}

fn begun_only_counts() -> CounterSnapshot {
    CounterSnapshot {
        active_show_begins: 1,
        ..CounterSnapshot::default()
    }
}
