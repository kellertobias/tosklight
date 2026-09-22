//! What the planning boundary has to be true for.
//!
//! These prove the claim the application rests on: a planning document patches a show with the
//! desk's own semantics and produces an ordinary show file, without a desk running.

use crate::{PaperworkMetadata, PlanningDocument};
use light_application::{PatchFixtureCandidate, PatchFixturesCommand};
use light_core::{FixtureId, Revision, ShowId};
use light_fixture::{
    FixtureLocation, FixtureProfile, FixtureVector, PatchedFixturePatch,
    PatchedFixtureProfileReference, SplitPatch,
};
use light_show::{FixtureProfileRevision, ShowStore};
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Repository rule: temporary work goes to the resolved artifact temp directory when one is set.
fn temp_path(name: &str) -> PathBuf {
    let base = std::env::var_os("LIGHT_TMP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let _ = std::fs::create_dir_all(&base);
    base.join(format!("viz-document-{name}-{}.show", Uuid::new_v4()))
}

struct Rig {
    document: PlanningDocument,
    profile: PatchedFixtureProfileReference,
    path: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A document holding one profile revision, as if it had already been patched from a library.
fn rig(name: &str) -> Rig {
    let path = temp_path(name);
    let document = PlanningDocument::create(&path, "Planning show").expect("create show");
    let mut profile = FixtureProfile::blank();
    profile.revision = 3;
    profile.manufacturer = "Acme".into();
    profile.name = "Planning Wash".into();
    profile.short_name = "Wash".into();
    let profile_id = profile.id;
    let profile_revision = Revision::from(profile.revision);
    let mode_id = profile.modes[0].id;
    let stored =
        FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap()).unwrap();
    ShowStore::open(&path)
        .unwrap()
        .insert_fixture_profile_revision(&stored)
        .expect("retain profile revision");
    Rig {
        document,
        profile: PatchedFixtureProfileReference {
            profile_id,
            profile_revision,
            mode_id,
        },
        path,
    }
}

/// A venue model imported into a show has no library entry: the show alone carries its profile.
#[test]
fn a_profile_kept_only_in_the_show_patches_and_reopens_without_a_library() {
    let path = temp_path("show-only-profile");
    let document = PlanningDocument::create(&path, "Venue show").expect("create show");
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Imported models".into();
    profile.name = "Hall".into();
    profile.short_name = "Hall".into();
    let reference = PatchedFixtureProfileReference {
        profile_id: profile.id,
        profile_revision: Revision::from(profile.revision),
        mode_id: profile.modes[0].id,
    };
    let body = serde_json::to_value(&profile).expect("profile JSON");
    document
        .retain_fixture_profile(body.clone())
        .expect("keep the profile in the show");
    document
        .retain_fixture_profile(body)
        .expect("keeping the same revision again changes nothing");
    document
        .patch_fixtures(patch_one(document.show_id(), reference))
        .expect("patched from the show's own profile");
    drop(document);

    let reopened = PlanningDocument::open(&path).expect("reopen without a library");
    let snapshot = reopened.patch_snapshot().expect("snapshot");
    assert_eq!(snapshot.fixtures.len(), 1);
    assert_eq!(snapshot.fixtures[0].profile.profile_id, profile.id);
    drop(reopened);
    let _ = std::fs::remove_file(&path);
}

fn patch_one(show_id: ShowId, profile: PatchedFixtureProfileReference) -> PatchFixturesCommand {
    PatchFixturesCommand {
        show_id,
        fixtures: vec![PatchFixtureCandidate {
            profile,
            patch: PatchedFixturePatch {
                model_scale: None,
                scenery_options: Default::default(),
                scenery_size_metres: None,
                fixture_id: FixtureId(Uuid::new_v4()),
                fixture_number: Some(1),
                virtual_fixture_number: None,
                name: "Wash 1".into(),
                universe: Some(1),
                address: Some(1),
                split_patches: vec![SplitPatch {
                    split: 1,
                    universe: Some(1),
                    address: Some(1),
                }],
                layer_id: "default".into(),
                note: None,
                position_master: None,
                direct_control: None,
                internal_bindings: Default::default(),
                location: FixtureLocation::default(),
                rotation: FixtureVector::default(),
                logical_heads: Vec::new(),
                multipatch: Vec::new(),
                group_masters_enabled: true,
                grand_master_enabled: true,
                invert_pan: false,
                invert_tilt: false,
                bracket_angle: 0.0,
                shaper_angle: None,
                installed_appearance: Default::default(),
                move_in_black_enabled: true,
                move_in_black_delay_millis: 0,
                highlight_overrides: BTreeMap::new(),
                freeze: Default::default(),
            },
        }],
        remove_fixture_ids: Vec::new(),
        placements: Vec::new(),
        vector_spreads: Vec::new(),
        fixture_updates: Vec::new(),
    }
}

#[test]
fn a_new_document_starts_with_an_empty_patch() {
    let rig = rig("empty");
    let snapshot = rig.document.patch_snapshot().expect("snapshot");
    assert!(snapshot.fixtures.is_empty());
}

#[test]
fn legacy_documents_default_to_blank_paperwork_and_round_trip_authored_values() {
    let rig = rig("paperwork");
    assert_eq!(
        rig.document.paperwork_metadata().expect("legacy metadata"),
        PaperworkMetadata::default()
    );

    rig.document
        .save_paperwork_metadata(&PaperworkMetadata {
            lighting_designer: "  Alex Designer  ".into(),
            show_version: "  2.4  ".into(),
            venue: "  Grand Hall  ".into(),
            contact_email: "  alex@example.com  ".into(),
            contact_phone: "  +49 123  ".into(),
            project: "  Summer Show  ".into(),
            show_date: "  2026-08-17  ".into(),
            company_logo: "  {\"mediaType\":\"image/jpeg\"}  ".into(),
        })
        .expect("save paperwork");
    let reopened = PlanningDocument::open(&rig.path).expect("reopen document");
    assert_eq!(
        reopened.paperwork_metadata().expect("saved metadata"),
        PaperworkMetadata {
            lighting_designer: "Alex Designer".into(),
            show_version: "2.4".into(),
            venue: "Grand Hall".into(),
            contact_email: "alex@example.com".into(),
            contact_phone: "+49 123".into(),
            project: "Summer Show".into(),
            show_date: "2026-08-17".into(),
            company_logo: "{\"mediaType\":\"image/jpeg\"}".into(),
        }
    );
}

#[test]
fn patching_a_fixture_needs_no_desk_runtime() {
    let rig = rig("patch");
    let result = rig
        .document
        .patch_fixtures(patch_one(rig.document.show_id(), rig.profile))
        .expect("patch a fixture with no engine, playback, programmer or session");
    assert_eq!(result.change.fixtures.len(), 1);

    let snapshot = rig.document.patch_snapshot().expect("snapshot");
    assert_eq!(snapshot.fixtures.len(), 1);
    assert_eq!(
        snapshot.fixtures[0].patch.universe,
        Some(1),
        "the committed patch is what the document reports"
    );
}

#[test]
fn a_patch_bumps_the_revision_it_is_versioned_against() {
    let rig = rig("revision");
    let before = rig.document.patch_revision().expect("revision");
    rig.document
        .patch_fixtures(patch_one(rig.document.show_id(), rig.profile))
        .expect("patch");
    let after = rig.document.patch_revision().expect("revision");
    assert!(
        after > before,
        "patch revision advances: {before} -> {after}"
    );
}

#[test]
fn a_second_fixture_may_share_an_address() {
    let rig = rig("shared-address");
    rig.document
        .patch_fixtures(patch_one(rig.document.show_id(), rig.profile))
        .expect("first fixture");
    rig.document
        .patch_fixtures(patch_one(rig.document.show_id(), rig.profile))
        .expect("a second fixture at the same address is accepted here");

    // Double-patching is legitimate rigging, so this boundary stores it. Surfacing the clash to
    // the operator is the patch sheet's job, exactly as it is on the desk.
    let snapshot = rig.document.patch_snapshot().expect("snapshot");
    assert_eq!(snapshot.fixtures.len(), 2);
    assert!(
        snapshot
            .fixtures
            .iter()
            .all(|fixture| fixture.patch.address == Some(1))
    );
}

#[test]
fn a_saved_document_reopens_as_an_ordinary_show_file() {
    let rig = rig("save");
    rig.document
        .patch_fixtures(patch_one(rig.document.show_id(), rig.profile))
        .expect("patch");
    let destination = temp_path("saved");
    rig.document.save_as(&destination).expect("save as");

    // The desk opens this through its own show library; nothing here is planning-specific.
    let (id, name) = light_show::validate_show_file(&destination).expect("a valid show file");
    assert_eq!(id, rig.document.show_id());
    assert_eq!(name, "Planning show");

    let reopened = PlanningDocument::open(&destination).expect("reopen");
    assert_eq!(
        reopened.patch_snapshot().expect("snapshot").fixtures.len(),
        1,
        "the patch survives the round trip without the library that supplied the profile"
    );
    let _ = std::fs::remove_file(&destination);
}

#[test]
fn exporting_mvr_carries_the_patched_rig() {
    let rig = rig("mvr");
    rig.document
        .patch_fixtures(patch_one(rig.document.show_id(), rig.profile))
        .expect("patch");
    let export = rig.document.export_mvr().expect("export");
    assert_eq!(export.summary.fixtures, 1);
    assert!(!export.data.is_empty());

    let read = PlanningDocument::read_mvr(&export.data).expect("read back what was written");
    assert_eq!(read.fixtures.len(), 1);
    assert_eq!(read.fixtures[0].universe, Some(1));
    assert_eq!(read.fixtures[0].address, Some(1));
    assert_eq!(
        export.summary.embedded_profiles, 0,
        "no source GDTF is retained"
    );
    assert_eq!(
        export.summary.generated_profiles, 1,
        "a profile with no retained source GDTF is embedded as a generated GDTF"
    );
    assert!(export.summary.missing_profiles.is_empty());

    // Another application resolves the fixture through the file its GDTFSpec names.
    let fixture = &read.fixtures[0];
    assert_eq!(fixture.gdtf_spec, "Acme@Wash.gdtf", "manufacturer@model");
    let gdtf = read
        .files
        .get(&fixture.gdtf_spec.to_ascii_lowercase())
        .expect("the GDTF the fixture names is in the archive");
    let modes = light_mvr::read_gdtf(gdtf).expect("a readable GDTF");
    assert!(
        modes.iter().any(|mode| mode.name == fixture.gdtf_mode),
        "the fixture's GDTFMode is a mode of that GDTF"
    );
}

#[test]
fn exported_mvr_layers_carry_the_patch_layer_names() {
    let rig = rig("mvr-layers");
    for (id, name, order) in [("default", "Front Truss", 0), ("spare", "Spare", 1)] {
        rig.document
            .put_object(
                "patch_layer",
                id,
                &serde_json::json!({"id": id, "name": name, "order": order}),
            )
            .expect("store the layer");
    }
    rig.document
        .patch_fixtures(patch_one(rig.document.show_id(), rig.profile))
        .expect("patch");
    let export = rig.document.export_mvr().expect("export");
    let archive = PlanningDocument::read_mvr(&export.data).expect("read back what was written");
    let xml = String::from_utf8(archive.files["generalscenedescription.xml"].clone()).unwrap();

    let truss = xml
        .find("name=\"Front Truss\"")
        .expect("the fixture's layer is named as the patch names it");
    let fixture = xml.find("name=\"Wash 1\"").expect("the fixture");
    let spare = xml
        .find("name=\"Spare\"")
        .expect("an empty layer is exported too");
    assert!(truss < fixture && fixture < spare, "{xml}");
    assert!(!xml.contains("name=\"default\""), "{xml}");
}

#[test]
fn an_exported_rig_imports_back_onto_layers_with_the_same_names() {
    let source = rig("mvr-layer-source");
    source
        .document
        .put_object(
            "patch_layer",
            "default",
            &serde_json::json!({"id": "default", "name": "Front Truss", "order": 0}),
        )
        .expect("store the layer");
    source
        .document
        .patch_fixtures(patch_one(source.document.show_id(), source.profile))
        .expect("patch");
    let archive = PlanningDocument::read_mvr(&source.document.export_mvr().expect("export").data)
        .expect("read the archive back");

    let target = rig("mvr-layer-target");
    target
        .document
        .import_mvr(archive, Default::default())
        .expect("import");

    let layers = target.document.objects("patch_layer").expect("layers");
    let truss = layers
        .iter()
        .find(|layer| layer.body["name"] == "Front Truss")
        .expect("the layer arrives under its own name");
    let snapshot = target.document.patch_snapshot().expect("snapshot");
    assert_eq!(snapshot.fixtures.len(), 1);
    assert_eq!(snapshot.fixtures[0].patch.layer_id, truss.id);
}

/// An import that cannot place a fixture has to say so before it writes, not count it afterwards.
#[test]
fn previewing_mvr_reports_what_the_archive_cannot_resolve_without_writing() {
    let rig = rig("mvr-preview");
    rig.document
        .patch_fixtures(patch_one(rig.document.show_id(), rig.profile))
        .expect("patch");
    let archive = PlanningDocument::read_mvr(&rig.document.export_mvr().expect("export").data)
        .expect("read the archive back");
    let revision = rig.document.patch_revision().expect("revision");

    let preview = rig.document.preview_mvr(&archive).expect("preview");

    assert_eq!(preview.fixtures.len(), 1);
    let fixture = &preview.fixtures[0];
    assert_eq!(fixture.universe, Some(1));
    assert_eq!(fixture.address, Some(1));
    assert!(
        !fixture.matched,
        "no fixture library is attached, so nothing here can be patched from one"
    );
    assert_eq!(
        preview.missing_profiles.len(),
        1,
        "the operator is told which GDTF has no profile, before deciding"
    );
    assert!(
        preview.address_conflicts.is_empty(),
        "a fixture that cannot be resolved has no footprint to conflict with"
    );
    assert_eq!(
        rig.document.patch_revision().expect("revision"),
        revision,
        "a preview writes nothing"
    );
}

/// A shipped part corrected after a show was built: the element takes the new revision on request.
///
/// A patched fixture keeps the exact revision it was placed with, so an old show keeps drawing what
/// it always drew. The Architect offers the operator the library's newer revision for a selected
/// element; this is the write that offer makes, and what the show must do with it — read the new
/// revision from the library, embed it beside the old one, and size the element by it.
#[test]
fn an_element_repatched_to_a_newer_revision_takes_it_from_the_library() {
    let path = temp_path("profile-upgrade");
    let library_path = path.with_extension("library.sqlite");
    let library = light_fixture::FixtureLibrary::open(&library_path).expect("library");

    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Venue".into();
    profile.name = "Four-Point Truss".into();
    profile.short_name = "4pt Truss".into();
    profile.patch_policy = light_fixture::PatchPolicy::VisualOnly;
    for mode in &mut profile.modes {
        mode.channels.clear();
        for split in &mut mode.splits {
            split.footprint = 0;
        }
    }
    profile.scenery = Some(truss_scenery(0.34));
    let old = library
        .save_profile(profile.clone(), 0)
        .expect("first revision");
    profile.scenery = Some(truss_scenery(0.29));
    let new = library
        .save_profile(profile, old.revision)
        .expect("corrected revision");
    assert!(new.revision > old.revision);

    let document = PlanningDocument::create(&path, "Upgrade show")
        .expect("create show")
        .with_library(library);
    let reference = |profile: &FixtureProfile| PatchedFixtureProfileReference {
        profile_id: profile.id,
        profile_revision: Revision::from(profile.revision),
        mode_id: profile.modes[0].id,
    };
    let mut command = patch_one(document.show_id(), reference(&old));
    // Stretched to 8 m while its section was still built at 340 mm.
    command.fixtures[0].patch.scenery_size_metres = Some(FixtureVector {
        x: 8000.0,
        y: 340.0,
        z: 340.0,
    });
    let fixture_id = command.fixtures[0].patch.fixture_id;
    document
        .patch_fixtures(command)
        .expect("patched from the old revision");

    let mut upgrade = patch_one(document.show_id(), reference(&new));
    upgrade.fixtures[0].patch.fixture_id = fixture_id;
    upgrade.fixtures[0].patch.scenery_size_metres = Some(FixtureVector {
        x: 8000.0,
        y: 290.0,
        z: 290.0,
    });
    document
        .patch_fixtures(upgrade)
        .expect("repatched to the newer revision");

    let snapshot = document.patch_snapshot().expect("snapshot");
    let patched = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.patch.fixture_id == fixture_id)
        .expect("the same element, not a second one");
    assert_eq!(snapshot.fixtures.len(), 1);
    assert_eq!(
        patched.profile.profile_revision,
        Revision::from(new.revision)
    );
    let size = patched.patch.scenery_size_metres.expect("its stored size");
    assert_eq!((size.x, size.y, size.z), (8000.0, 290.0, 290.0));
    // Both revisions stay in the show: the old one is what any other element still references.
    let store = ShowStore::open(&path).expect("reopen the store");
    for revision in [old.revision, new.revision] {
        assert!(
            store
                .resolve_fixture_profile_revision(old.id, Revision::from(revision))
                .expect("read the revision")
                .is_some(),
            "revision {revision} is embedded in the show"
        );
    }
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&library_path);
}

/// A truss section: made to measure along its length, fixed across it.
fn truss_scenery(section: f32) -> light_fixture::ProfileScenery {
    light_fixture::ProfileScenery {
        kind: light_fixture::ProfileSceneryKind::Truss,
        chords: 4,
        default_size_metres: light_fixture::Vector3 {
            x: 4.0,
            y: section,
            z: section,
        },
        adjustable: light_fixture::SceneryAxes {
            width: true,
            height: false,
            depth: false,
        },
        minimum_size_metres: light_fixture::Vector3 {
            x: 0.25,
            y: section,
            z: section,
        },
        maximum_size_metres: light_fixture::Vector3 {
            x: 24.0,
            y: section,
            z: section,
        },
        pattern: Default::default(),
        feet: Default::default(),
        handrails: false,
    }
}

#[path = "mvr_bracket_tests.rs"]
mod mvr_bracket;
