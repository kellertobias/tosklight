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

fn patch_one(show_id: ShowId, profile: PatchedFixtureProfileReference) -> PatchFixturesCommand {
    PatchFixturesCommand {
        show_id,
        fixtures: vec![PatchFixtureCandidate {
            profile,
            patch: PatchedFixturePatch {
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
        })
        .expect("save paperwork");
    let reopened = PlanningDocument::open(&rig.path).expect("reopen document");
    assert_eq!(
        reopened.paperwork_metadata().expect("saved metadata"),
        PaperworkMetadata {
            lighting_designer: "Alex Designer".into(),
            show_version: "2.4".into(),
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
        "a profile with no retained source GDTF is referenced, not embedded"
    );
    assert_eq!(export.summary.missing_profiles.len(), 1);
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
