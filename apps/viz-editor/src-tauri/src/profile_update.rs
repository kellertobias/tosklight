//! Bringing a placed element up to this computer's copy of its profile.
//!
//! A patched element keeps the exact profile revision it was placed with, and a revision never
//! changes, so a show always draws what it drew when it was built. That is what makes a show
//! portable — and it is also why a part corrected since, such as a truss section built at 340 mm
//! that is now 290 or a corner block that was 1068 mm across and is now 500, keeps its old shape
//! while newly added parts beside it have the new one.
//!
//! This offers the operator the library's copy for one element and, when they take it, moves the
//! element onto it. Two things make that less simple than raising a revision number:
//!
//! * **Revision numbers are per library.** Two desks number their own revisions independently, so a
//!   show can hold revision 4 of a profile whose content is not what this library calls revision 4.
//!   What decides whether there is anything to offer is therefore the content digest, not the
//!   number.
//! * **A revision already embedded cannot be rewritten.** When the number this library would use is
//!   taken in the show by different content, the element moves onto the next free number above
//!   both, with the library's copy kept in the show under it. Elements still on the old revision
//!   keep drawing exactly as they did.

use crate::contract::{MutationDto, OutcomeDto};
use crate::session::{Session, apply_patch_mutation};
use light_application::PatchFixtureProjection;
use light_core::FixtureId;
use light_fixture::{FixtureLibrary, FixtureProfile};
use light_show::FixtureProfileRevision;
use serde::Serialize;
use uuid::Uuid;

type Answer<T> = Result<T, String>;

/// What the library holds for a placed element, when it is not what the element was built from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdateDto {
    /// The revision the element was placed with, as the show numbers it.
    pub from_revision: u32,
    /// The revision it would move onto: the library's own, or the next free one when that is taken.
    pub to_revision: u32,
    /// The profile's name, for the sentence the panel shows.
    pub name: String,
}

/// The move one element would make, worked out from the show and the library together.
struct Update {
    profile: FixtureProfile,
    from_revision: u32,
    to_revision: u32,
}

/// The library's newest copy of one profile, read by itself.
///
/// One profile rather than the whole library: a shipped profile carries its model, so listing every
/// profile to find one would read tens of megabytes each time an element is selected.
fn newest_in_library(library: &FixtureLibrary, profile_id: FixtureId) -> Option<FixtureProfile> {
    let newest = library
        .profile_revisions(profile_id)
        .ok()?
        .into_iter()
        .max()?;
    library.profile(profile_id, newest).ok().flatten()
}

/// Whether the show's copy of a revision is the same part as the library's copy.
///
/// Both sides are read back through the profile itself rather than compared as text: a show stores
/// the JSON it was patched from, and the same measurement written by two different paths can differ
/// in its last decimal place without being a different part.
fn same_content(stored: &FixtureProfileRevision, profile: &FixtureProfile, revision: u32) -> bool {
    let normalized = |profile: &FixtureProfile| {
        let mut profile = profile.clone();
        profile.revision = revision;
        serde_json::to_value(&profile).ok()
    };
    serde_json::from_value::<FixtureProfile>(stored.profile().clone())
        .ok()
        .and_then(|embedded| Some((normalized(&embedded)?, normalized(profile)?)))
        .is_some_and(|(embedded, library)| embedded == library)
}

fn plan(
    document: &viz_document::PlanningDocument,
    library: &FixtureLibrary,
    fixture_id: Uuid,
) -> Result<Option<Update>, String> {
    let snapshot = document
        .patch_snapshot()
        .map_err(|error| error.to_string())?;
    let Some(fixture) = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.patch.fixture_id.0 == fixture_id)
    else {
        return Ok(None);
    };
    let profile_id = fixture.profile.profile_id;
    let from_revision = u32::try_from(fixture.profile.profile_revision).unwrap_or(u32::MAX);
    let Some(profile) = newest_in_library(library, profile_id) else {
        return Ok(None);
    };
    let embedded = document
        .fixture_profile_revisions_for(profile_id)
        .map_err(|error| error.to_string())?;
    let at = |revision: u32| {
        embedded
            .iter()
            .find(|stored| u32::try_from(stored.id().revision()).unwrap_or(0) == revision)
    };
    // Nothing to offer while the show already holds this library's copy for this element.
    if at(from_revision).is_some_and(|stored| same_content(stored, &profile, from_revision)) {
        return Ok(None);
    }
    let taken = at(profile.revision)
        .is_some_and(|stored| !same_content(stored, &profile, profile.revision));
    let to_revision = if taken || profile.revision <= from_revision {
        embedded
            .iter()
            .map(|stored| u32::try_from(stored.id().revision()).unwrap_or(0))
            .chain(std::iter::once(profile.revision))
            .max()
            .unwrap_or(profile.revision)
            + 1
    } else {
        profile.revision
    };
    Ok(Some(Update {
        profile,
        from_revision,
        to_revision,
    }))
}

/// Whether this computer's library holds a different copy of one element's profile.
#[tauri::command]
pub fn fixture_profile_update(
    session: tauri::State<'_, Session>,
    fixture_id: Uuid,
) -> Answer<Option<ProfileUpdateDto>> {
    let Some(library) = crate::session::open_library(&session) else {
        return Ok(None);
    };
    session.with(|document| {
        plan(document, &library, fixture_id).map(|update| {
            update.map(|update| ProfileUpdateDto {
                from_revision: update.from_revision,
                to_revision: update.to_revision,
                name: update.profile.name.clone(),
            })
        })
    })
}

/// Moves one element onto this computer's copy of its profile, keeping everything else about it.
///
/// The size the operator set is kept; a measurement the profile does not let them set — the section
/// of a truss, the size of a corner block — takes the new profile's own, which is the correction
/// they asked for.
#[tauri::command]
pub fn update_fixture_profile(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, crate::cad::CadState>,
    fixture_id: Uuid,
) -> Answer<OutcomeDto> {
    let library = crate::session::open_library(&session)
        .ok_or_else(|| "This computer has no fixture library.".to_owned())?;
    let prepared = session.change(|document| {
        let update = plan(document, &library, fixture_id)?
            .ok_or_else(|| "This element is already on the newest version.".to_owned())?;
        let mut profile = update.profile.clone();
        profile.revision = update.to_revision;
        document
            .retain_fixture_profile(serde_json::to_value(&profile).map_err(|e| e.to_string())?)
            .map_err(|error| error.to_string())?;
        let snapshot = document
            .patch_snapshot()
            .map_err(|error| error.to_string())?;
        let fixture = snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.patch.fixture_id.0 == fixture_id)
            .ok_or_else(|| "That element is no longer in the show.".to_owned())?;
        Ok(mutation(fixture, &profile))
    })?;
    apply_patch_mutation(&app, &session, &cad, Some(window.label()), prepared)
}

/// The write that moves one element onto a profile revision the show now holds.
fn mutation(fixture: &PatchFixtureProjection, profile: &FixtureProfile) -> MutationDto {
    let patch = &fixture.patch;
    let mode_id = profile
        .modes
        .iter()
        .find(|mode| mode.id == fixture.profile.mode_id)
        .or_else(|| profile.modes.first())
        .map(|mode| mode.id);
    let size = patch.scenery_size_metres.map(|size| {
        let scenery = profile.scenery.as_ref();
        let axis = |stored: f32, adjustable: bool, default: f32| {
            if scenery.is_none() || adjustable {
                stored
            } else {
                default * 1000.0
            }
        };
        match scenery {
            Some(scenery) => serde_json::json!({
                "x": axis(size.x, scenery.adjustable.width, scenery.default_size_metres.x),
                "y": axis(size.y, scenery.adjustable.height, scenery.default_size_metres.y),
                "z": axis(size.z, scenery.adjustable.depth, scenery.default_size_metres.z),
            }),
            None => serde_json::json!({ "x": size.x, "y": size.y, "z": size.z }),
        }
    });
    let fixture_json = serde_json::json!({
        "fixtureId": patch.fixture_id.0,
        "fixtureNumber": patch.fixture_number,
        "virtualFixtureNumber": patch.virtual_fixture_number,
        "name": patch.name,
        "profileId": profile.id.0,
        "profileRevision": profile.revision,
        "modeId": mode_id,
        "splitPatches": patch
            .split_patches
            .iter()
            .map(|split| {
                serde_json::json!({
                    "split": split.split,
                    "universe": split.universe,
                    "address": split.address,
                })
            })
            .collect::<Vec<_>>(),
        "layerId": patch.layer_id,
        "location": { "x": patch.location.x, "y": patch.location.y, "z": patch.location.z },
        "rotation": { "x": patch.rotation.x, "y": patch.rotation.y, "z": patch.rotation.z },
        "bracketAngle": patch.bracket_angle,
        "shaperAngle": patch.shaper_angle,
        "invertPan": patch.invert_pan,
        "invertTilt": patch.invert_tilt,
        "groupMastersEnabled": patch.group_masters_enabled,
        "grandMasterEnabled": patch.grand_master_enabled,
        "moveInBlackEnabled": patch.move_in_black_enabled,
        "moveInBlackDelayMillis": patch.move_in_black_delay_millis,
        "modelScale": patch.model_scale,
        "sceneryOptions": serde_json::to_value(&patch.scenery_options).unwrap_or(serde_json::Value::Null),
        "scenerySizeMetres": size,
        "multipatch": serde_json::to_value(&patch.multipatch).unwrap_or(serde_json::Value::Null),
    });
    serde_json::from_value(serde_json::json!({
        "requestId": Uuid::new_v4().to_string(),
        "fixtures": [fixture_json],
    }))
    .expect("the write is built from the element the show already holds")
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_fixture::{FixtureVector, PatchedFixturePatch, PatchedFixtureProfileReference};
    use viz_document::PlanningDocument;

    fn workspace(name: &str) -> std::path::PathBuf {
        let base = std::env::var_os("LIGHT_TMP_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("viz-editor-profile-update")
            .join(format!("{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&base).expect("workspace");
        base
    }

    /// The same truss profile at another section, so the library sees one part being corrected.
    fn corrected(profile: &FixtureProfile, section: f32) -> FixtureProfile {
        let mut next = truss(section);
        next.id = profile.id;
        next.modes = profile.modes.clone();
        next
    }

    /// A truss profile: made to measure along its length, fixed across its section.
    fn truss(section: f32) -> FixtureProfile {
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
        let across = light_fixture::Vector3 {
            x: 4.0,
            y: section,
            z: section,
        };
        profile.scenery = Some(light_fixture::ProfileScenery {
            kind: light_fixture::ProfileSceneryKind::Truss,
            chords: 4,
            default_size_metres: across,
            adjustable: light_fixture::SceneryAxes {
                width: true,
                height: false,
                depth: false,
            },
            minimum_size_metres: light_fixture::Vector3 { x: 0.25, ..across },
            maximum_size_metres: light_fixture::Vector3 { x: 24.0, ..across },
            pattern: Default::default(),
        });
        profile
    }

    fn patch(
        document: &PlanningDocument,
        profile: &FixtureProfile,
        length: f32,
        section: f32,
    ) -> Uuid {
        let fixture_id = Uuid::new_v4();
        document
            .patch_fixtures(light_application::PatchFixturesCommand {
                show_id: document.show_id(),
                fixtures: vec![light_application::PatchFixtureCandidate {
                    profile: PatchedFixtureProfileReference {
                        profile_id: profile.id,
                        profile_revision: light_core::Revision::from(profile.revision),
                        mode_id: profile.modes[0].id,
                    },
                    patch: PatchedFixturePatch {
                        fixture_id: light_core::FixtureId(fixture_id),
                        fixture_number: None,
                        virtual_fixture_number: Some(1),
                        name: "Four-Point Truss".into(),
                        universe: None,
                        address: None,
                        split_patches: vec![light_fixture::SplitPatch {
                            split: 1,
                            universe: None,
                            address: None,
                        }],
                        layer_id: "default".into(),
                        note: None,
                        position_master: None,
                        direct_control: None,
                        internal_bindings: Default::default(),
                        location: light_fixture::FixtureLocation::default(),
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
                        highlight_overrides: Default::default(),
                        freeze: Default::default(),
                        model_scale: None,
                        scenery_options: Default::default(),
                        scenery_size_metres: Some(FixtureVector {
                            x: length,
                            y: section,
                            z: section,
                        }),
                    },
                }],
                remove_fixture_ids: Vec::new(),
                placements: Vec::new(),
                vector_spreads: Vec::new(),
                fixture_updates: Vec::new(),
            })
            .expect("patched");
        fixture_id
    }

    #[test]
    fn an_element_on_this_library_s_own_copy_is_offered_nothing() {
        let root = workspace("current");
        let library = FixtureLibrary::open(root.join("fixtures.sqlite")).expect("library");
        let profile = library.save_profile(truss(0.29), 0).expect("saved");
        let document = PlanningDocument::create(root.join("show.show"), "Show")
            .expect("show")
            .with_library_at(root.join("fixtures.sqlite"))
            .expect("library");
        let fixture_id = patch(&document, &profile, 8.0, 0.29);
        assert!(
            plan(&document, &library, fixture_id)
                .expect("planned")
                .is_none()
        );
    }

    #[test]
    fn a_corrected_part_is_offered_to_the_elements_built_from_the_old_one() {
        let root = workspace("corrected");
        let library = FixtureLibrary::open(root.join("fixtures.sqlite")).expect("library");
        let old = library.save_profile(truss(0.34), 0).expect("saved");
        let document = PlanningDocument::create(root.join("show.show"), "Show")
            .expect("show")
            .with_library_at(root.join("fixtures.sqlite"))
            .expect("library");
        let fixture_id = patch(&document, &old, 8000.0, 340.0);
        let new = library
            .save_profile(corrected(&old, 0.29), old.revision)
            .expect("the corrected revision");

        let update = plan(&document, &library, fixture_id)
            .expect("planned")
            .expect("the corrected part is offered");
        assert_eq!(update.from_revision, old.revision);
        assert_eq!(update.to_revision, new.revision);

        // The write keeps the length the operator set and takes the corrected section.
        let mut moved = update.profile.clone();
        moved.revision = update.to_revision;
        let snapshot = document.patch_snapshot().expect("snapshot");
        let fixture = snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.patch.fixture_id.0 == fixture_id)
            .expect("the element");
        let written = mutation(fixture, &moved);
        let size = &written.fixtures[0].scenery_size_metres;
        assert_eq!(
            serde_json::to_value(size).expect("the size it writes"),
            serde_json::json!({ "x": 8000.0, "y": 290.0, "z": 290.0 })
        );
    }

    /// Two libraries number their own revisions, so a show can hold a different revision 2.
    #[test]
    fn a_number_taken_in_the_show_by_other_content_is_stepped_over() {
        let root = workspace("collision");
        let library = FixtureLibrary::open(root.join("fixtures.sqlite")).expect("library");
        let first = library.save_profile(truss(0.34), 0).expect("saved");
        let second = library
            .save_profile(corrected(&first, 0.29), first.revision)
            .expect("the corrected revision");
        let document = PlanningDocument::create(root.join("show.show"), "Show")
            .expect("show")
            .with_library_at(root.join("fixtures.sqlite"))
            .expect("library");
        // The show was built elsewhere: its own revision 2 of this profile is the 340 mm one.
        let mut elsewhere = corrected(&second, 0.34);
        elsewhere.revision = second.revision;
        document
            .retain_fixture_profile(serde_json::to_value(&elsewhere).expect("json"))
            .expect("the show keeps its own copy");
        let fixture_id = patch(&document, &elsewhere, 8000.0, 340.0);

        let update = plan(&document, &library, fixture_id)
            .expect("planned")
            .expect("the library's copy is offered");
        assert_eq!(update.from_revision, second.revision);
        assert_eq!(update.to_revision, second.revision + 1);
    }
}
