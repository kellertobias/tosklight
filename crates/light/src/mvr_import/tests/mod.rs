mod support;

use super::*;
use crate::{ActionErrorKind, ApplicationEvent, EventFilter, EventReplay, ShowEvent};
use light_core::FixtureId;
use light_fixture::{
    FixtureProfile, GelAssignment, InstalledFixtureAppearance, InstalledLightSource,
    MultiPatchInstance, PatchPolicy, PortablePatchedFixtureRecord, SplitPatch,
};
use std::convert::Infallible;
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
fn replace_and_reimport_preserve_reference_only_patch_settings_and_physical_copies() {
    let rig = Rig::new();
    let mut profile = FixtureProfile::blank();
    profile.id = FixtureId(Uuid::from_u128(800));
    profile.revision = 1;
    profile.manufacturer = "MVR Maker".into();
    profile.name = "MVR Model".into();
    profile.short_name = "MVR Model".into();
    profile.modes[0].id = Uuid::from_u128(801);
    profile.modes[0].name = "Standard".into();
    let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    let retained_id = FixtureId(Uuid::from_u128(301));
    let replaced_id = FixtureId(Uuid::from_u128(302));
    let retained_source = Uuid::from_u128(303);
    let store = rig.ports.store();
    let mut retained_fixture = stored_fixture(retained_id, definition.clone(), 10, (false, 750));
    retained_fixture.group_masters_enabled = false;
    retained_fixture.grand_master_enabled = false;
    retained_fixture.invert_pan = true;
    retained_fixture.invert_tilt = true;
    retained_fixture.bracket_angle = -25.0;
    retained_fixture.shaper_angle = Some(15.0);
    retained_fixture.installed_appearance = InstalledFixtureAppearance {
        light_source: InstalledLightSource::Tungsten,
        luminous_output_lumens: None,
        color_temperature_kelvin: Some(3_200),
        gel: GelAssignment::Custom {
            name: "MVR retained amber".into(),
            color_srgb: "#FFAA44".into(),
            note: Some("Installed at the rig".into()),
        },
        shaper_angles_degrees: [1.0, 2.0, 3.0, 4.0],
    };
    retained_fixture.multipatch.push(MultiPatchInstance {
        scenery_size_metres: None,
        id: Uuid::from_u128(305),
        name: "Retained copy".into(),
        universe: Some(2),
        address: Some(1),
        split_patches: vec![SplitPatch {
            split: 1,
            universe: Some(2),
            address: Some(1),
        }],
        location: Default::default(),
        rotation: Default::default(),
        invert_pan: false,
        invert_tilt: false,
        position_calibration: None,
        color_calibration: None,
        bracket_angle: 12.0,
        shaper_angle: None,
        installed_appearance: InstalledFixtureAppearance {
            light_source: InstalledLightSource::Led,
            color_temperature_kelvin: Some(5_600),
            ..Default::default()
        },
    });
    let retained_record =
        PortablePatchedFixtureRecord::from_runtime_fixture(&retained_fixture).unwrap();
    store
        .put_object(
            "patched_fixture",
            &retained_id.0.to_string(),
            retained_record.body(),
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
    assert!(!retained.patch.group_masters_enabled);
    assert!(!retained.patch.grand_master_enabled);
    assert!(retained.patch.invert_pan);
    assert!(retained.patch.invert_tilt);
    assert_eq!(retained.patch.bracket_angle, -25.0);
    assert_eq!(retained.patch.shaper_angle, Some(15.0));
    assert_eq!(
        retained.patch.installed_appearance,
        retained_fixture.installed_appearance
    );
    assert_eq!(retained.patch.multipatch, retained_fixture.multipatch);
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

#[test]
fn tosklight_mvr_round_trip_preserves_scenery_markers_and_ordinary_fixtures() {
    struct NoGdtf;
    impl crate::mvr_export::GdtfSource for NoGdtf {
        type Error = Infallible;

        fn source_gdtf(
            &self,
            _profile: FixtureId,
            _revision: u32,
        ) -> Result<Option<Vec<u8>>, Self::Error> {
            Ok(None)
        }
    }

    let mut venue = stored_fixture(
        FixtureId(Uuid::from_u128(501)),
        fixture_definition_from(1, FixtureId(Uuid::from_u128(511)), "Venue"),
        1,
        (true, 0),
    );
    venue.name = "Venue marker".into();
    venue.fixture_number = Some(201);

    let visual = visual_fixture(502, 512, "Independent visual-only", "Touring", 7);
    let reserved = visual_fixture(503, 513, "Reserved scenery", "Legacy", 8);

    let mut ordinary = stored_fixture(
        FixtureId(Uuid::from_u128(504)),
        fixture_definition(1),
        20,
        (true, 0),
    );
    ordinary.name = "Ordinary fixture".into();
    ordinary.fixture_number = Some(42);
    ordinary.definition.id = FixtureId(Uuid::from_u128(514));

    let source = vec![venue, visual, reserved, ordinary];
    let export_objects = source
        .iter()
        .cloned()
        .map(|fixture| (fixture.fixture_id.0.to_string(), fixture))
        .collect::<Vec<_>>();
    let (document, summary) = crate::mvr_export::build_mvr_document(
        &export_objects,
        &Default::default(),
        Vec::new(),
        &NoGdtf,
        |_| None,
    )
    .unwrap();
    assert_eq!(summary.fixtures, 4);
    assert_eq!(
        document
            .fixtures
            .iter()
            .map(|fixture| fixture.fixture_id.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["201", "0.7", "0.8", "42"]
    );

    let archive = light_mvr::write(&document).unwrap();
    let round_tripped = light_mvr::read(&archive).unwrap();
    let embedded = crate::mvr_export::tosklight_mvr_fixture_metadata(&round_tripped);
    assert_eq!(embedded.len(), 4);

    let rig = Rig::new();
    let mut envelope = rig.envelope(Vec::new(), Vec::new());
    envelope.command.document = round_tripped;
    let result = rig.service.apply(envelope, &rig.ports).unwrap();
    assert_eq!(result.imported_fixtures, 4);
    assert_eq!(result.unresolved_fixtures, 0);

    let installed = rig.ports.installed.lock();
    let fixtures = &installed.as_ref().unwrap().fixtures;
    let by_name = |name: &str| {
        fixtures
            .iter()
            .find(|fixture| fixture.name == name)
            .unwrap()
    };
    let imported_venue = by_name("Venue marker");
    assert_eq!(imported_venue.fixture_id, FixtureId(Uuid::from_u128(501)));
    assert_eq!(imported_venue.fixture_number, Some(201));
    assert_eq!(imported_venue.definition.manufacturer, "Venue");

    let imported_visual = by_name("Independent visual-only");
    assert_eq!(imported_visual.fixture_id, FixtureId(Uuid::from_u128(502)));
    assert_eq!(imported_visual.virtual_fixture_number, Some(7));
    assert_eq!(
        imported_visual
            .definition
            .profile_snapshot
            .as_ref()
            .unwrap()
            .patch_policy,
        PatchPolicy::VisualOnly
    );

    let imported_reserved = by_name("Reserved scenery");
    assert_eq!(
        imported_reserved.fixture_id,
        FixtureId(Uuid::from_u128(503))
    );
    assert_eq!(imported_reserved.virtual_fixture_number, Some(8));

    let imported_ordinary = by_name("Ordinary fixture");
    assert_eq!(
        imported_ordinary.fixture_id,
        FixtureId(Uuid::from_u128(504))
    );
    assert_eq!(imported_ordinary.fixture_number, Some(42));
    assert!(imported_ordinary.definition.is_dmx_patchable());
}

fn visual_fixture(
    fixture_identity: u128,
    profile_identity: u128,
    name: &str,
    manufacturer: &str,
    virtual_number: u32,
) -> light_fixture::PatchedFixture {
    let mut profile = FixtureProfile::blank();
    profile.id = FixtureId(Uuid::from_u128(profile_identity));
    profile.manufacturer = manufacturer.into();
    profile.name = name.into();
    profile.short_name = name.into();
    profile.patch_policy = PatchPolicy::VisualOnly;
    profile.modes[0].splits[0].footprint = 0;
    profile.modes[0].channels.clear();
    profile.modes[0].color_systems.clear();
    profile.modes[0].control_actions.clear();
    let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    let mut fixture = stored_fixture(
        FixtureId(Uuid::from_u128(fixture_identity)),
        definition,
        1,
        (true, 0),
    );
    fixture.name = name.into();
    fixture.fixture_number = None;
    fixture.virtual_fixture_number = Some(virtual_number);
    fixture.universe = None;
    fixture.address = None;
    fixture.split_patches = vec![SplitPatch {
        split: 1,
        universe: None,
        address: None,
    }];
    fixture
}

#[test]
fn imported_fixtures_land_on_layers_named_as_the_file_names_them() {
    let rig = Rig::new();
    let mut on_truss = mvr_fixture(Uuid::from_u128(1), "On truss", 1, 1);
    on_truss.layer = Some("uuid-truss".into());
    let mut on_default = mvr_fixture(Uuid::from_u128(2), "On default", 1, 10);
    on_default.layer = Some("uuid-default".into());
    let mut envelope = rig.envelope(vec![on_truss, on_default], vec![fixture_definition(1)]);
    envelope.command.document.layers = vec![
        light_mvr::MvrLayer {
            id: "uuid-truss".into(),
            name: "Front Truss".into(),
        },
        light_mvr::MvrLayer {
            id: "uuid-default".into(),
            name: "Default".into(),
        },
        light_mvr::MvrLayer {
            id: "uuid-audience".into(),
            name: "Audience".into(),
        },
    ];

    rig.service.apply(envelope, &rig.ports).unwrap();

    let document = rig.document();
    let layers: Vec<_> = document
        .objects_of_kind("patch_layer")
        .map(|object| {
            (
                object.key().id().to_owned(),
                object.body()["name"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        layers,
        [("uuid-truss".to_owned(), "Front Truss".to_owned())],
        "a layer no imported fixture is on is not created, and Default is the patch's own"
    );
    let layer_of = |name: &str| {
        document
            .objects_of_kind("patched_fixture")
            .map(|object| {
                PortablePatchedFixtureRecord::decode(object.body().clone())
                    .unwrap()
                    .patch()
                    .unwrap()
            })
            .find(|fixture| fixture.name == name)
            .unwrap()
            .layer_id
    };
    assert_eq!(layer_of("On truss"), "uuid-truss");
    assert_eq!(layer_of("On default"), DEFAULT_PATCH_LAYER);
}

#[test]
fn native_secondary_output_survives_ordinary_import_and_explicit_unpatch_frees_its_address() {
    struct NoGdtf;
    impl crate::mvr_export::GdtfSource for NoGdtf {
        type Error = Infallible;
        fn source_gdtf(&self, _: FixtureId, _: u32) -> Result<Option<Vec<u8>>, Self::Error> {
            Ok(None)
        }
    }
    let rig = Rig::new();
    // The legacy fixture factory has per-mode geometry; use its canonical loaded revision.
    let mut profile: FixtureProfile = serde_json::from_value(
        serde_json::to_value(fixture_definition(1).profile_snapshot.unwrap()).unwrap(),
    )
    .unwrap();
    profile.modes[0].splits.push(light_fixture::FixtureSplit {
        number: 2,
        footprint: 1,
    });
    let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    let mut fixture = stored_fixture(
        FixtureId(Uuid::from_u128(910)),
        definition.clone(),
        1,
        (true, 0),
    );
    fixture.universe = None;
    fixture.address = None;
    fixture.split_patches = vec![
        SplitPatch {
            split: 1,
            universe: None,
            address: None,
        },
        SplitPatch {
            split: 2,
            universe: Some(2),
            address: Some(10),
        },
    ];
    let (native, _) = crate::mvr_export::build_mvr_document(
        &[(fixture.fixture_id.0.to_string(), fixture.clone())],
        &Default::default(),
        Vec::new(),
        &NoGdtf,
        |_| None,
    )
    .unwrap();
    let uuid = native.fixtures[0].uuid;
    let mut first = rig.envelope(Vec::new(), Vec::new());
    first.command.document = native.clone();
    let result = rig.service.apply(first, &rig.ports).unwrap();
    assert_eq!(
        result.change.fixtures[0].patch.split_patches,
        fixture.split_patches
    );
    assert_eq!(occupied_patches(&rig.document()).unwrap().len(), 1);

    let incoming = mvr_fixture(Uuid::from_u128(911), "Freed address", 2, 10);
    let mut update = rig.envelope(vec![incoming.clone()], vec![definition]);
    update.command.document = native.clone();
    update.command.document.fixtures.push(incoming);
    update
        .command
        .resolutions
        .insert(uuid, MvrImportResolution::ImportUnpatched);
    let result = rig.service.apply(update, &rig.ports).unwrap();
    assert_eq!(result.imported_fixtures, 2);
    let original = result
        .change
        .fixtures
        .iter()
        .find(|item| item.patch.fixture_id == fixture.fixture_id)
        .unwrap();
    assert!(
        original
            .patch
            .split_patches
            .iter()
            .all(|patch| patch.address.is_none())
    );
    let incoming = result
        .change
        .fixtures
        .iter()
        .find(|item| item.patch.fixture_id != fixture.fixture_id)
        .unwrap();
    assert_eq!(incoming.patch.universe, Some(2));
    assert_eq!(incoming.patch.address, Some(10));
    assert!(
        !result
            .warnings
            .iter()
            .any(|warning| warning.contains("conflicts"))
    );
    // A requested Address may be rejected even when the source had no primary address.
    let mut rejected = rig.envelope(Vec::new(), Vec::new());
    rejected.command.document = native;
    rejected.command.resolutions.insert(
        uuid,
        MvrImportResolution::Address {
            universe: 2,
            address: 10,
        },
    );
    let result = rig.service.apply(rejected, &rig.ports).unwrap();
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("imported unpatched"))
    );
    assert!(
        result.change.fixtures[0]
            .patch
            .split_patches
            .iter()
            .all(|patch| patch.address.is_none())
    );
}

#[test]
fn large_import_plan_shares_one_profile_projection_and_stores_lean_records() {
    let rig = Rig::new();
    let mut profile =
        std::sync::Arc::unwrap_or_clone(fixture_definition(1).profile_snapshot.unwrap());
    let archive = light_fixture::gdtf::profile::package_profile(&profile).unwrap();
    profile.source_gdtf =
        Some(light_fixture::ProfileGdtfSource::associate(&profile, &archive).unwrap());
    let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    let fixtures = (0..300)
        .map(|index| {
            mvr_fixture(
                Uuid::from_u128(1000 + index),
                &format!("Rig {index}"),
                1,
                index as u16 + 1,
            )
        })
        .collect();
    let request = rig.envelope(fixtures, vec![definition]);
    let planned =
        super::plan::plan_import(&rig.document(), request.context, &request.command).unwrap();
    assert_eq!(planned.state.patch.profiles.len(), 1);
    let first = &planned.state.patch.fixtures[0].profile_projection;
    for fixture in &planned.state.patch.fixtures {
        assert!(std::sync::Arc::ptr_eq(first, &fixture.profile_projection));
        assert!(fixture.record.get("definition").is_none());
        assert!(fixture.record.get("source_gdtf").is_none());
        assert!(serde_json::to_vec(&fixture.record).unwrap().len() < 3000);
    }
    let candidate = crate::prepare_show_candidate(&rig.document(), planned.transaction).unwrap();
    let (transaction, snapshot) = candidate.into_parts();
    assert_eq!(snapshot.fixtures.len(), 300);
    let committed = rig
        .ports
        .store()
        .apply_portable_transaction(transaction)
        .unwrap();
    assert_eq!(committed.fixture_profile_revisions().len(), 1);
    let stored = rig.document();
    assert_eq!(stored.fixture_profile_revisions().len(), 1);
    assert_eq!(stored.objects_of_kind("patched_fixture").count(), 300);
}

#[test]
fn mvr_prepare_checks_preview_revision_against_the_actual_snapshot() {
    let rig = Rig::new();
    rig.service
        .apply(
            rig.envelope(
                vec![mvr_fixture(Uuid::from_u128(701), "First", 1, 1)],
                vec![fixture_definition(1)],
            ),
            &rig.ports,
        )
        .unwrap();
    let mut stale = rig.envelope(
        vec![mvr_fixture(Uuid::from_u128(702), "Stale", 1, 20)],
        vec![fixture_definition(1)],
    );
    stale.context = stale.context.with_expected_revision(0);
    let error = rig.service.prepare(stale, &rig.ports).err().unwrap();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(1));
    assert_eq!(count(&rig.ports.counters.commits), 1);
    assert_eq!(rig.document().objects_of_kind("patched_fixture").count(), 1);
}

#[test]
fn mvr_missing_full_library_revision_never_publishes_a_stripped_catalog() {
    let mut profile = FixtureProfile::blank();
    profile.revision = 1;
    profile.manufacturer = "Library".into();
    profile.name = "Exact".into();
    profile.short_name = "Exact".into();
    let mut source = mvr_fixture(Uuid::from_u128(1), "Missing", 1, 1);
    source.gdtf_spec = "Exact.gdtf".into();
    source.gdtf_mode = profile.modes[0].name.clone();
    let document = light_mvr::MvrDocument {
        fixtures: vec![source],
        ..Default::default()
    };
    let bound = bind_mvr_sources(&document, &[profile], &[], |_, _| Ok(None), |_| vec![]).unwrap();
    assert!(bound.definitions.is_empty());
    assert!(
        bound
            .warnings
            .iter()
            .any(|warning| warning.contains("disappeared during preview"))
    );
}

mod binding_candidates;

#[test]
fn preview_destination_revision_rejects_non_patch_edits_before_prepare_and_atomic_commit() {
    for edited_after_prepare in [false, true] {
        let rig = Rig::new();
        let preview = rig.document();
        let expected = Some((preview.revision(), preview.patch_revision()));
        let envelope = rig.envelope(
            vec![mvr_fixture(Uuid::from_u128(0x68099), "Preview", 1, 1)],
            vec![fixture_definition(1)],
        );
        let prepared = if edited_after_prepare {
            Some(
                rig.service
                    .prepare_at_preview_revision(envelope.clone(), expected, &rig.ports)
                    .unwrap(),
            )
        } else {
            None
        };
        let mut transaction = preview.transaction();
        transaction.put(
            "group",
            "1",
            serde_json::json!({"name":"Edited since preview","fixtures":[]}),
        );
        rig.ports
            .store()
            .apply_portable_transaction(transaction)
            .unwrap();
        let edited = rig.document();
        assert_ne!(edited.revision(), preview.revision());
        assert_eq!(edited.patch_revision(), preview.patch_revision());
        let error = if let Some(prepared) = prepared {
            rig.service.commit(prepared, &rig.ports).unwrap_err()
        } else {
            rig.service
                .apply_at_preview_revision(envelope, expected, &rig.ports)
                .unwrap_err()
        };
        assert_eq!(error.kind, ActionErrorKind::Conflict);
        assert_eq!(rig.document(), edited);
        assert_eq!(count(&rig.ports.counters.backups), 0);
        assert_eq!(count(&rig.ports.counters.commits), 0);
        assert_eq!(count(&rig.ports.counters.runtime_installs), 0);
    }
}
