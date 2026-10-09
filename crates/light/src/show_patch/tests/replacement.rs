use super::*;
use crate::{PatchFixtureUpdateAction, PatchHeadReplacement};
use light_core::FixtureId;
use light_fixture::{FixtureProfile, InstalledPositionCalibration};

fn replacement_rig() -> (
    TestRig,
    crate::PatchFixturesResult,
    light_fixture::PatchedFixtureProfileReference,
) {
    let (old, source) = profile_with_modes(1);
    let mut authored: FixtureProfile = serde_json::from_value(old.profile().clone()).unwrap();
    authored.modes[0].heads[0].master_shared = false;
    let mut second = authored.modes[0].heads[0].clone();
    second.id = Uuid::new_v4();
    second.name = "Second".into();
    authored.modes[0].heads.push(second);
    let old =
        FixtureProfileRevision::from_profile(serde_json::to_value(authored).unwrap()).unwrap();
    let mut target: FixtureProfile = serde_json::from_value(old.profile().clone()).unwrap();
    target.id = FixtureId(Uuid::new_v4());
    target.manufacturer = "Other manufacturer".into();
    target.name = "Other physical product".into();
    target.modes[0].id = Uuid::new_v4();
    for head in &mut target.modes[0].heads {
        head.id = Uuid::new_v4();
    }
    let replacement = light_fixture::PatchedFixtureProfileReference {
        profile_id: target.id,
        profile_revision: target.revision.into(),
        mode_id: target.modes[0].id,
    };
    let mut rig = TestRig::new(old, FailurePoint::None);
    rig.ports.add_profile(
        FixtureProfileRevision::from_profile(serde_json::to_value(target).unwrap()).unwrap(),
    );
    let mut command = patch_batch(rig.ports.show_id(), source, 1);
    command.fixtures[0].patch.location.x = 1234;
    command.fixtures[0].patch.invert_pan = true;
    command.fixtures[0].patch.note = Some("Installed root metadata".into());
    command.fixtures[0]
        .patch
        .multipatch
        .push(light_fixture::MultiPatchInstance {
            id: Uuid::new_v4(),
            name: "Independent installed copy".into(),
            universe: Some(2),
            address: Some(301),
            split_patches: vec![light_fixture::SplitPatch {
                split: 1,
                universe: Some(2),
                address: Some(301),
            }],
            location: light_fixture::FixtureLocation {
                x: 9876,
                y: 321,
                z: 654,
            },
            invert_tilt: true,
            position_calibration: Some(InstalledPositionCalibration {
                tilt_zero_degrees: 13.0,
                ..Default::default()
            }),
            ..Default::default()
        });
    command.fixtures[0].patch.position_calibration = Some(InstalledPositionCalibration {
        pan_zero_degrees: 7.0,
        ..Default::default()
    });
    let first = rig
        .service
        .handle(envelope(command, "initial", 0), &rig.ports)
        .unwrap();
    (rig, first, replacement)
}

#[test]
fn replacement_profile_preserves_root_and_explicit_head_programming_and_reopens() {
    let (rig, first, target) = replacement_rig();
    let old = &first.change.fixtures[0];
    assert!(!old.patch.logical_heads.is_empty());
    let mut group = group_mutation(rig.ports.show_id(), "replacement-targets");
    let members = vec![
        old.patch.fixture_id,
        old.patch.logical_heads[1].fixture_id,
        old.patch.logical_heads[0].fixture_id,
    ];
    let group_body = serde_json::to_value(light_programmer::GroupDefinition {
        id: "replacement-targets".into(),
        name: "Ordered replacement targets".into(),
        fixtures: members.clone(),
        source: Some(light_programmer::GroupFixtureSource::Explicit {
            fixture_ids: members,
        }),
        ..Default::default()
    })
    .unwrap();
    group.command.mutations[0].mutation = crate::ActiveShowObjectMutationKind::Put {
        body: Box::new(
            crate::ActiveShowObjectBody::decode(crate::ActiveShowObjectKind::Group, group_body)
                .unwrap(),
        ),
    };
    let grouped = rig.active_show.mutate_objects(group, &rig.ports).unwrap();
    let before_group = rig
        .portable_document()
        .object("group", "replacement-targets")
        .unwrap()
        .body()
        .clone();
    let target_profile = crate::ShowPatchPorts::resolve_profile_revision(
        &rig.ports,
        target.profile_id,
        target.profile_revision,
    )
    .unwrap();
    let profile: FixtureProfile = serde_json::from_value(target_profile.profile().clone()).unwrap();
    let mapping = old
        .patch
        .logical_heads
        .iter()
        .zip(profile.modes[0].heads.iter().filter(|h| !h.master_shared))
        .map(|(old, new)| PatchHeadReplacement {
            fixture_id: old.fixture_id,
            target_profile_head_id: Some(new.id),
        })
        .collect();
    let command = sparse_update(
        rig.ports.show_id(),
        old.patch.fixture_id,
        old.fixture_revision,
        grouped.show_revision.value(),
        None,
        PatchFixtureUpdateAction::ReplaceProfile {
            root_programming_mapping: Vec::new(),
            profile: target,
            head_mapping: mapping,
        },
    );
    let result = rig
        .service
        .handle(
            envelope(
                command.clone(),
                "replace",
                first.change.patch_revision.value(),
            ),
            &rig.ports,
        )
        .unwrap();
    let new = &result.change.fixtures[0];
    assert_eq!(new.profile, target);
    let mut expected = old.patch.clone();
    expected.logical_heads = new.patch.logical_heads.clone();
    assert_eq!(new.patch, expected);
    assert_eq!(
        new.patch.logical_heads[0].fixture_id,
        old.patch.logical_heads[0].fixture_id
    );
    assert_ne!(
        new.patch.logical_heads[0].profile_head_id,
        old.patch.logical_heads[0].profile_head_id
    );
    let replay = rig
        .service
        .handle(
            envelope(command, "replace", first.change.patch_revision.value()),
            &rig.ports,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.change, result.change);
    rig.assert_portable_patch(1, 2);
    assert_eq!(
        rig.portable_document()
            .object("group", "replacement-targets")
            .unwrap()
            .body(),
        &before_group
    );
    let body = rig
        .portable_document()
        .objects_of_kind("patched_fixture")
        .next()
        .unwrap()
        .body()
        .clone();
    let record = light_fixture::PortablePatchedFixtureRecord::decode(body).unwrap();
    assert_eq!(record.patch().unwrap(), new.patch);
    assert_eq!(record.selected_profile_reference().unwrap(), Some(target));
}

#[test]
fn replacement_missing_mapping_or_stale_consent_is_atomic() {
    let (rig, first, target) = replacement_rig();
    let old = &first.change.fixtures[0];
    let before = rig.portable_document();
    let counters = rig.counters();
    let invalid = sparse_update(
        rig.ports.show_id(),
        old.patch.fixture_id,
        old.fixture_revision,
        first.change.show_revision.value(),
        None,
        PatchFixtureUpdateAction::ReplaceProfile {
            root_programming_mapping: Vec::new(),
            profile: target,
            head_mapping: vec![],
        },
    );
    let error = rig
        .service
        .handle(
            envelope(
                invalid,
                "missing-head-consent",
                first.change.patch_revision.value(),
            ),
            &rig.ports,
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid);
    let stale = sparse_update(
        rig.ports.show_id(),
        old.patch.fixture_id,
        old.fixture_revision,
        0,
        None,
        PatchFixtureUpdateAction::ReplaceProfile {
            root_programming_mapping: Vec::new(),
            profile: target,
            head_mapping: vec![],
        },
    );
    let error = rig
        .service
        .handle(
            envelope(stale, "stale-consent", first.change.patch_revision.value()),
            &rig.ports,
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    let after = rig.portable_document();
    assert_eq!(after.revision(), before.revision());
    assert_eq!(
        after.fixture_profile_revisions().len(),
        before.fixture_profile_revisions().len()
    );
    assert_eq!(rig.counters().commits, counters.commits);
    assert_eq!(rig.counters().runtime_installs, counters.runtime_installs);
}

#[test]
fn replacement_footprint_conflict_rejects_root_copies_and_profile_publication() {
    let (old, source) = profile_with_modes(1);
    let mut target: FixtureProfile = serde_json::from_value(old.profile().clone()).unwrap();
    target.id = FixtureId(Uuid::new_v4());
    target.modes[0].id = Uuid::new_v4();
    target.modes[0].splits[0].footprint = 2;
    let reference = light_fixture::PatchedFixtureProfileReference {
        profile_id: target.id,
        profile_revision: target.revision.into(),
        mode_id: target.modes[0].id,
    };
    let mut rig = TestRig::new(old, FailurePoint::None);
    rig.ports.add_profile(
        FixtureProfileRevision::from_profile(serde_json::to_value(target).unwrap()).unwrap(),
    );
    let initial = rig
        .service
        .handle(
            envelope(
                patch_batch(rig.ports.show_id(), source, 2),
                "footprint-seed",
                0,
            ),
            &rig.ports,
        )
        .unwrap();
    let fixture = &initial.change.fixtures[0];
    let before = rig.portable_document();
    let counters = rig.counters();
    let request = sparse_update(
        rig.ports.show_id(),
        fixture.patch.fixture_id,
        fixture.fixture_revision,
        initial.change.show_revision.value(),
        None,
        PatchFixtureUpdateAction::ReplaceProfile {
            root_programming_mapping: Vec::new(),
            profile: reference,
            head_mapping: vec![],
        },
    );
    let error = rig
        .service
        .handle(
            envelope(
                request,
                "overlapping-replacement",
                initial.change.patch_revision.value(),
            ),
            &rig.ports,
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid);
    let after = rig.portable_document();
    assert_eq!(after.revision(), before.revision());
    assert_eq!(
        after.fixture_profile_revisions().len(),
        before.fixture_profile_revisions().len()
    );
    assert_eq!(rig.counters().commits, counters.commits);
    assert_eq!(rig.counters().runtime_installs, counters.runtime_installs);
}

fn shared_root_programming_rig() -> (
    TestRig,
    crate::PatchFixturesResult,
    light_fixture::PatchedFixtureProfileReference,
    Uuid,
    Vec<Uuid>,
) {
    let (stored, source) = profile_with_modes(1);
    let mut old: FixtureProfile = serde_json::from_value(stored.profile().clone()).unwrap();
    old.modes[0].heads[0].master_shared = true;
    let head = old.modes[0].heads[0].id;
    old.modes[0].channels = vec![light_fixture::FixtureChannel {
        id: Uuid::new_v4(),
        head_id: head,
        split: 1,
        fixture_attribute: light_core::AttributeKey::intensity(),
        attribute: light_core::AttributeKey::intensity(),
        canonical_transform: Default::default(),
        resolution: light_fixture::ChannelResolution::U8,
        secondary_slots: vec![],
        default_raw: 0,
        highlight_raw: 255,
        physical_min: Some(0.0),
        physical_max: Some(1.0),
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        behavior: light_fixture::ChannelBehavior::Controlled,
        functions: vec![light_fixture::ChannelFunction::continuous(
            "intensity",
            light_core::AttributeKey::intensity(),
            255,
        )],
    }];
    old.modes[0].splits[0].footprint = 1;
    let mut target = old.clone();
    target.id = FixtureId::new();
    target.modes[0].id = Uuid::new_v4();
    target.modes[0].heads[0].id = Uuid::new_v4();
    target.modes[0].heads[0].master_shared = false;
    let mut second = target.modes[0].heads[0].clone();
    second.id = Uuid::new_v4();
    target.modes[0].heads.push(second);
    let heads = target.modes[0]
        .heads
        .iter()
        .map(|head| head.id)
        .collect::<Vec<_>>();
    target.modes[0].channels[0].head_id = heads[0];
    let mut second = target.modes[0].channels[0].clone();
    second.id = Uuid::new_v4();
    second.head_id = heads[1];
    target.modes[0].channels.push(second);
    target.modes[0].splits[0].footprint = 2;
    let reference = light_fixture::PatchedFixtureProfileReference {
        profile_id: target.id,
        profile_revision: target.revision.into(),
        mode_id: target.modes[0].id,
    };
    let mut rig = TestRig::new(
        FixtureProfileRevision::from_profile(serde_json::to_value(old).unwrap()).unwrap(),
        FailurePoint::None,
    );
    rig.ports.add_profile(
        FixtureProfileRevision::from_profile(serde_json::to_value(target).unwrap()).unwrap(),
    );
    let first = rig
        .service
        .handle(
            envelope(
                patch_batch(rig.ports.show_id(), source, 1),
                "seed-shared-root",
                0,
            ),
            &rig.ports,
        )
        .unwrap();
    let root = first.change.fixtures[0].patch.fixture_id;
    let mut group = light_programmer::GroupDefinition {
        id: "1".into(),
        fixtures: vec![root],
        source: Some(light_programmer::GroupFixtureSource::Explicit {
            fixture_ids: vec![root],
        }),
        ..Default::default()
    };
    group.programming.insert(
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.5),
    );
    rig.seed_programming_object("group", "1", &serde_json::to_value(group).unwrap());
    (rig, first, reference, head, heads)
}

#[test]
fn replacement_shared_root_stages_exact_group_owner_consent_and_refuses_missing_choice() {
    let (rig, first, target, old_head, new_heads) = shared_root_programming_rig();
    let document = rig.portable_document();
    let root = first.change.fixtures[0].patch.fixture_id;
    let mut command = sparse_update(
        rig.ports.show_id(),
        root,
        first.change.fixtures[0].fixture_revision,
        document.revision().value(),
        None,
        PatchFixtureUpdateAction::ReplaceProfile {
            profile: target,
            head_mapping: vec![],
            root_programming_mapping: vec![],
        },
    );
    let refused = super::super::prepare::stage_patch_command(&document, &command, &rig.ports);
    assert!(refused.is_err());
    assert_eq!(
        rig.portable_document()
            .objects()
            .map(|object| object.body().clone())
            .collect::<Vec<_>>(),
        document
            .objects()
            .map(|object| object.body().clone())
            .collect::<Vec<_>>()
    );
    if let PatchFixtureUpdateAction::ReplaceProfile {
        root_programming_mapping,
        ..
    } = &mut command.fixture_updates[0].action
    {
        root_programming_mapping.push(crate::PatchRootProgramReplacement {
            source_profile_head_id: old_head,
            attribute: light_core::AttributeKey::intensity(),
            target_profile_head_ids: new_heads,
        });
    }
    let mut staged =
        super::super::prepare::stage_patch_command(&document, &command, &rig.ports).unwrap();
    let transaction = staged.take_transaction();
    let candidate = document.candidate(&transaction).unwrap();
    let after = candidate.object("group", "1").unwrap().body();
    let before = document.object("group", "1").unwrap().body();
    assert_eq!(after["fixtures"], before["fixtures"]);
    assert_eq!(after["source"], before["source"]);
    assert_eq!(after["programming"], before["programming"]);
    let envelope = &after["replacement_projections"]["intensity"][root.0.to_string()];
    assert_eq!(envelope["source_owner"], root.0.to_string());
    assert_eq!(envelope["targets"].as_array().unwrap().len(), 2);
    assert_eq!(envelope["source_head_id"], old_head.to_string());
    assert_eq!(document.object("group", "1").unwrap().body(), before);
}

#[test]
fn replacement_shared_root_persists_reopens_and_refuses_old_reader_without_partial_commit() {
    let (mut rig, first, target, old_head, new_heads) = shared_root_programming_rig();
    let root = first.change.fixtures[0].patch.fixture_id;
    let preset = light_programmer::Preset {
        name: "Original source".into(),
        family: light_programmer::PresetFamily::Intensity,
        number: 1,
        instance_id: Some(Uuid::new_v4()),
        values: [(
            root,
            [(
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Normalized(0.5),
            )]
            .into(),
        )]
        .into(),
        group_values: Default::default(),
        universal_values: Default::default(),
        aim_at_fixture_number: None,
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
    };
    rig.seed_programming_object("preset", "1", &serde_json::to_value(preset).unwrap());
    let document = rig.portable_document();
    let command = sparse_update(
        rig.ports.show_id(),
        root,
        first.change.fixtures[0].fixture_revision,
        document.revision().value(),
        None,
        PatchFixtureUpdateAction::ReplaceProfile {
            profile: target,
            head_mapping: vec![],
            root_programming_mapping: vec![crate::PatchRootProgramReplacement {
                source_profile_head_id: old_head,
                attribute: light_core::AttributeKey::intensity(),
                target_profile_head_ids: new_heads,
            }],
        },
    );
    let original = document
        .objects()
        .map(|object| (object.key().clone(), object.body().clone()))
        .collect::<Vec<_>>();
    rig.ports.set_failure(FailurePoint::Commit);
    let rejected = rig
        .service
        .handle(
            envelope(
                command.clone(),
                "injected-replacement-failure",
                first.change.patch_revision.value(),
            ),
            &rig.ports,
        )
        .unwrap_err();
    assert_eq!(rejected.kind, ActionErrorKind::Internal);
    assert_eq!(
        rig.portable_document()
            .objects()
            .map(|object| (object.key().clone(), object.body().clone()))
            .collect::<Vec<_>>(),
        original
    );
    assert!(rig.accepts_programming_contract(3));
    rig.ports.set_failure(FailurePoint::None);
    let result = rig
        .service
        .handle(
            envelope(
                command,
                "committed-root-correspondence",
                first.change.patch_revision.value(),
            ),
            &rig.ports,
        )
        .unwrap();
    assert_eq!(result.change.fixtures[0].patch.fixture_id, root);
    assert_eq!(
        result.change.fixtures[0].patch.fixture_number,
        first.change.fixtures[0].patch.fixture_number
    );
    let reopened = rig.portable_document();
    assert_eq!(
        reopened.object("group", "1").unwrap().body()["fixtures"],
        document.object("group", "1").unwrap().body()["fixtures"]
    );
    assert_eq!(
        reopened.object("preset", "1").unwrap().body()["instance_id"],
        document.object("preset", "1").unwrap().body()["instance_id"]
    );
    let stored = &reopened.object("preset", "1").unwrap().body()["fixture_replacement_projections"]
        [root.0.to_string()]["intensity"];
    assert_eq!(stored["targets"].as_array().unwrap().len(), 2);
    assert!(rig.accepts_programming_contract(4));
    assert!(!rig.accepts_programming_contract(3));
    // The targeted object notification includes both same-number identities independently.
    let mut staged = super::super::prepare::stage_patch_command(
        &document,
        &sparse_update(
            rig.ports.show_id(),
            root,
            first.change.fixtures[0].fixture_revision,
            document.revision().value(),
            None,
            PatchFixtureUpdateAction::ReplaceProfile {
                profile: target,
                head_mapping: vec![],
                root_programming_mapping: vec![crate::PatchRootProgramReplacement {
                    source_profile_head_id: old_head,
                    attribute: light_core::AttributeKey::intensity(),
                    target_profile_head_ids: result.change.fixtures[0]
                        .patch
                        .logical_heads
                        .iter()
                        .map(|head| head.profile_head_id.unwrap())
                        .collect(),
                }],
            },
        ),
        &rig.ports,
    )
    .unwrap();
    let transaction = staged.take_transaction();
    let (_, changes) = staged
        .finish(
            &document,
            document.candidate(&transaction).unwrap(),
            &Default::default(),
        )
        .unwrap();
    assert_eq!(changes.len(), 2);
    assert!(
        changes
            .iter()
            .any(|change| change.kind == crate::ActiveShowObjectKind::Group
                && change.object_id == "1")
    );
    assert!(changes.iter().any(
        |change| change.kind == crate::ActiveShowObjectKind::Preset && change.object_id == "1"
    ));
}
