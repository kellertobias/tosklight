//! The CAD's undo history: moves of the rig and deletions from it, in the order they were made.
//!
//! A deletion keeps the whole patched fixture it removed — its number, patch, multi-patch copies,
//! place and turn — and the rig attachments it had, so Undo patches the very same fixture back
//! under the same identity. Moves recorded before the deletion then find their fixture again.

use super::{
    CadState, EntityTransform, RigAttachment, TransformOutcome, apply_transforms, attachments,
    emit_scene_delta, restore_attachments, selectable_ids, selected_transforms,
};
use crate::annotation::{CadAnnotation, announce, read_annotations, store, validate};
use crate::contract::{FixtureDto, MutationDto};
use crate::session::{Session, apply_patch_mutation};
use serde::Serialize;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Default)]
pub(super) struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
}

#[derive(Clone)]
enum Step {
    Move(TransformRecord),
    Delete(DeleteRecord),
    /// New fixtures, such as copies: Undo removes them and Redo patches them back.
    Add(DeleteRecord),
    /// A drawn item changed — text moved or reworded: Undo writes it back as it was.
    Annotation(Box<AnnotationChange>),
}

#[derive(Clone)]
struct AnnotationChange {
    before: CadAnnotation,
    after: CadAnnotation,
}

#[derive(Clone)]
pub(super) struct TransformRecord {
    pub before: Vec<EntityTransform>,
    pub after: Vec<EntityTransform>,
    pub before_attachments: Vec<RigAttachment>,
    pub after_attachments: Vec<RigAttachment>,
}

/// What one deletion removed, kept whole so it can be patched back exactly.
#[derive(Clone, Debug)]
pub(super) struct DeleteRecord {
    /// Each removed fixture as the patch writes it, in the order it was deleted.
    fixtures: Vec<serde_json::Value>,
    attachments: Vec<RigAttachment>,
}

impl History {
    /// A new move: it goes on the undo stack and ends any redo.
    pub(super) fn record_move(&mut self, record: TransformRecord) {
        self.undo.push(Step::Move(record));
        self.redo.clear();
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOutcome {
    pub scene_revision: u64,
    pub deleted_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddOutcome {
    pub scene_revision: u64,
    pub added_ids: Vec<Uuid>,
}

/// The fixtures `ids` names as they stand, ready to be deleted and later patched back.
fn deletion(session: &Session, ids: &[Uuid]) -> Result<DeleteRecord, String> {
    let wanted: BTreeSet<Uuid> = ids.iter().copied().collect();
    let patch =
        session.with(|document| document.patch_snapshot().map_err(|error| error.to_string()))?;
    let fixtures = patch
        .fixtures
        .into_iter()
        .filter(|fixture| wanted.contains(&fixture.patch.fixture_id.0))
        .map(|fixture| serde_json::to_value(FixtureDto::from(fixture)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if fixtures.is_empty() {
        return Err("None of the selected CAD entities exist any more".to_owned());
    }
    let attachments = attachments(session)?
        .into_iter()
        .filter(|attachment| wanted.contains(&attachment.fixture_id))
        .collect();
    Ok(DeleteRecord {
        fixtures,
        attachments,
    })
}

impl DeleteRecord {
    fn ids(&self) -> Vec<Uuid> {
        self.fixtures
            .iter()
            .filter_map(|fixture| fixture.get("fixtureId")?.as_str()?.parse().ok())
            .collect()
    }

    fn removal(&self) -> MutationDto {
        MutationDto {
            request_id: Uuid::new_v4().to_string(),
            fixtures: Vec::new(),
            remove_fixture_ids: self.ids(),
            placements: Vec::new(),
        }
    }

    fn restoration(&self) -> Result<MutationDto, String> {
        Ok(MutationDto {
            request_id: Uuid::new_v4().to_string(),
            fixtures: self
                .fixtures
                .iter()
                .map(|fixture| {
                    serde_json::from_value::<FixtureDto>(fixture.clone())
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<_, _>>()?,
            remove_fixture_ids: Vec::new(),
            placements: Vec::new(),
        })
    }
}

fn current_revision(session: &Session) -> Result<u64, String> {
    session.with(|document| document.patch_revision().map_err(|error| error.to_string()))
}

fn check_revision(session: &Session, expected: u64) -> Result<(), String> {
    let current = current_revision(session)?;
    if current != expected {
        return Err(format!(
            "The rig changed at revision {current}; refresh before committing revision {expected}"
        ));
    }
    Ok(())
}

fn put_attachments(session: &Session, stored: &[RigAttachment]) -> Result<(), String> {
    for attachment in stored {
        session.change(|document| {
            document
                .put_object(
                    "rig_attachment",
                    &attachment.fixture_id.to_string(),
                    &serde_json::to_value(attachment).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())
        })?;
    }
    Ok(())
}

fn drop_attachments(session: &Session, ids: &[Uuid]) -> Result<(), String> {
    for id in ids {
        session.change(|document| {
            document
                .delete_object("rig_attachment", &id.to_string())
                .map(|_| ())
                .map_err(|error| error.to_string())
        })?;
    }
    Ok(())
}

/// Removes what `record` holds from the show, telling every window (the patch mutation announces
/// it to the sheets and the CAD alike), and gives the new revision.
fn remove(
    app: &tauri::AppHandle,
    session: &Session,
    cad: &CadState,
    record: &DeleteRecord,
) -> Result<u64, String> {
    apply_patch_mutation(app, session, cad, None, record.removal())?;
    drop_attachments(session, &record.ids())?;
    current_revision(session)
}

/// Patches what `record` removed back, with its attachments, and gives the new revision.
fn restore(
    app: &tauri::AppHandle,
    session: &Session,
    cad: &CadState,
    record: &DeleteRecord,
) -> Result<u64, String> {
    apply_patch_mutation(app, session, cad, None, record.restoration()?)?;
    put_attachments(session, &record.attachments)?;
    current_revision(session)
}

/// Deletes fixtures from the CAD as one step that Undo brings back.
#[tauri::command]
pub fn cad_delete(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    expected_scene_revision: u64,
    fixture_ids: Vec<Uuid>,
) -> Result<DeleteOutcome, String> {
    check_revision(&session, expected_scene_revision)?;
    let record = deletion(&session, &fixture_ids)?;
    let scene_revision = remove(&app, &session, &cad, &record)?;
    let deleted_ids = record.ids();
    let mut history = cad.history.lock();
    history.undo.push(Step::Delete(record));
    history.redo.clear();
    Ok(DeleteOutcome {
        scene_revision,
        deleted_ids,
    })
}

/// Sets where fixtures stand and how they are turned, as one step Undo puts back: the rotate handle
/// turns a selection about a pivot, which moves each fixture's position as well as its rotation.
#[tauri::command]
pub fn cad_set_transforms(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    expected_scene_revision: u64,
    transforms: Vec<EntityTransform>,
) -> Result<TransformOutcome, String> {
    let ids: BTreeSet<Uuid> = transforms.iter().map(|transform| transform.id).collect();
    if ids.is_empty() || ids.len() != transforms.len() {
        return Err("Set each selected CAD entity's transform exactly once".to_owned());
    }
    if !ids.is_subset(&selectable_ids(&session)?) {
        return Err("One or more selected CAD entities belong to a locked layer".to_owned());
    }
    check_revision(&session, expected_scene_revision)?;
    let before = session.with(|document| {
        let patch = document
            .patch_snapshot()
            .map_err(|error| error.to_string())?;
        selected_transforms(&patch, &ids)
    })?;
    if before.len() != ids.len() {
        return Err("One or more selected CAD entities no longer exist".to_owned());
    }
    let held = attachments(&session)?
        .into_iter()
        .filter(|attachment| ids.contains(&attachment.fixture_id))
        .collect::<Vec<_>>();
    let revision = apply_transforms(&session, expected_scene_revision, &transforms)?;
    cad.history.lock().record_move(TransformRecord {
        before,
        after: transforms.clone(),
        before_attachments: held.clone(),
        after_attachments: held,
    });
    emit_scene_delta(&app, &session, &cad, revision, Vec::new())?;
    Ok(TransformOutcome {
        scene_revision: revision,
        transforms,
        attachments: attachments(&session)?,
    })
}

/// Adds fixtures to the show as one step Undo takes away again: the copies a duplicate makes.
///
/// Each arrives whole, as the patch writes it, with its own new identity; Redo patches the very
/// same fixtures back.
#[tauri::command]
pub fn cad_add(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    expected_scene_revision: u64,
    fixtures: Vec<serde_json::Value>,
) -> Result<AddOutcome, String> {
    if fixtures.is_empty() {
        return Err("There is nothing to add".to_owned());
    }
    check_revision(&session, expected_scene_revision)?;
    let record = DeleteRecord {
        fixtures,
        attachments: Vec::new(),
    };
    let scene_revision = restore(&app, &session, &cad, &record)?;
    let added_ids = record.ids();
    let mut history = cad.history.lock();
    history.undo.push(Step::Add(record));
    history.redo.clear();
    Ok(AddOutcome {
        scene_revision,
        added_ids,
    })
}

/// Changes a drawn item the CAD already holds — moves or rewords a piece of text — as one step
/// Undo puts back.
#[tauri::command]
pub fn cad_change_annotation(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    annotation: CadAnnotation,
) -> Result<CadAnnotation, String> {
    validate(&annotation)?;
    let before = read_annotations(&session)?
        .into_iter()
        .find(|stored| stored.id == annotation.id)
        .ok_or_else(|| "That drawn item is no longer in the show".to_owned())?;
    store(&session, &annotation)?;
    announce(&app, &session)?;
    let mut history = cad.history.lock();
    history
        .undo
        .push(Step::Annotation(Box::new(AnnotationChange {
            before,
            after: annotation.clone(),
        })));
    history.redo.clear();
    Ok(annotation)
}

/// Which way through the history a step is taken.
#[derive(Clone, Copy)]
enum Direction {
    Back,
    Forward,
}

fn take(
    app: &tauri::AppHandle,
    session: &Session,
    cad: &CadState,
    expected_scene_revision: u64,
    direction: Direction,
) -> Result<TransformOutcome, String> {
    let step = {
        let mut history = cad.history.lock();
        let stack = match direction {
            Direction::Back => &mut history.undo,
            Direction::Forward => &mut history.redo,
        };
        stack.pop().ok_or_else(|| match direction {
            Direction::Back => "There is nothing in the CAD to undo".to_owned(),
            Direction::Forward => "There is nothing in the CAD to redo".to_owned(),
        })?
    };
    let result = apply_step(app, session, cad, expected_scene_revision, &step, direction);
    let mut history = cad.history.lock();
    // A step that could not be taken stays where it was; one that was taken crosses over.
    let target = match (direction, result.is_ok()) {
        (Direction::Back, true) | (Direction::Forward, false) => &mut history.redo,
        (Direction::Forward, true) | (Direction::Back, false) => &mut history.undo,
    };
    target.push(step);
    result
}

fn apply_step(
    app: &tauri::AppHandle,
    session: &Session,
    cad: &CadState,
    expected_scene_revision: u64,
    step: &Step,
    direction: Direction,
) -> Result<TransformOutcome, String> {
    match step {
        Step::Annotation(change) => {
            let item = match direction {
                Direction::Back => &change.before,
                Direction::Forward => &change.after,
            };
            store(session, item)?;
            announce(app, session)?;
            Ok(TransformOutcome {
                scene_revision: current_revision(session)?,
                transforms: Vec::new(),
                attachments: attachments(session)?,
            })
        }
        Step::Move(record) => {
            let (transforms, stored) = match direction {
                Direction::Back => (&record.before, &record.before_attachments),
                Direction::Forward => (&record.after, &record.after_attachments),
            };
            let revision = apply_transforms(session, expected_scene_revision, transforms)?;
            restore_attachments(session, transforms, stored)?;
            emit_scene_delta(app, session, cad, revision, Vec::new())?;
            Ok(TransformOutcome {
                scene_revision: revision,
                transforms: transforms.clone(),
                attachments: attachments(session)?,
            })
        }
        Step::Delete(record) | Step::Add(record) => {
            check_revision(session, expected_scene_revision)?;
            // Undoing a deletion patches the fixtures back; undoing an addition takes them away.
            let restoring = matches!(
                (step, direction),
                (Step::Delete(_), Direction::Back) | (Step::Add(_), Direction::Forward)
            );
            let revision = if restoring {
                restore(app, session, cad, record)?
            } else {
                remove(app, session, cad, record)?
            };
            Ok(TransformOutcome {
                scene_revision: revision,
                transforms: Vec::new(),
                attachments: attachments(session)?,
            })
        }
    }
}

#[tauri::command]
pub fn cad_undo(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    expected_scene_revision: u64,
) -> Result<TransformOutcome, String> {
    take(
        &app,
        &session,
        &cad,
        expected_scene_revision,
        Direction::Back,
    )
}

#[tauri::command]
pub fn cad_redo(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    expected_scene_revision: u64,
) -> Result<TransformOutcome, String> {
    take(
        &app,
        &session,
        &cad,
        expected_scene_revision,
        Direction::Forward,
    )
}

#[cfg(test)]
mod tests {
    use super::deletion;
    use crate::cad::{EntityTransform, apply_transforms, tests::transform_session};
    use crate::contract::{FixtureDto, MutationDto};
    use crate::session::Session;

    fn patched(session: &Session) -> Vec<serde_json::Value> {
        session
            .with(|document| document.patch_snapshot().map_err(|error| error.to_string()))
            .unwrap()
            .fixtures
            .into_iter()
            .map(|fixture| serde_json::to_value(FixtureDto::from(fixture)).unwrap())
            .collect()
    }

    fn apply(session: &Session, mutation: MutationDto) -> u64 {
        session
            .change(|document| {
                let command = mutation.into_command(document.show_id());
                document
                    .patch_fixtures(command)
                    .map(|result| result.change.patch_revision.value())
                    .map_err(|error| error.to_string())
            })
            .unwrap()
    }

    #[test]
    fn a_deletion_patches_the_same_fixtures_back_and_earlier_moves_find_them_again() {
        let (session, path, ids) = transform_session();
        let before = patched(&session);
        let record = deletion(&session, &ids).unwrap();
        // The patch lists its fixtures in its own order, so the record holds them in that order.
        let mut deleted = record.ids();
        deleted.sort();
        let mut wanted = ids.to_vec();
        wanted.sort();
        assert_eq!(deleted, wanted);

        apply(&session, record.removal());
        assert!(patched(&session).is_empty());

        // Undo: the very same fixtures, with their numbers, patch and multi-patch copies.
        let revision = apply(&session, record.restoration().unwrap());
        assert_eq!(patched(&session), before);

        // A move made before the deletion is undone against the restored fixture, not refused as
        // a fixture that no longer exists.
        apply_transforms(
            &session,
            revision,
            &[EntityTransform {
                id: ids[0],
                position_millimetres: [500, 2_000, 3_000],
                rotation_degrees: [0.0, 0.0, 0.0],
            }],
        )
        .unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn an_added_copy_takes_its_own_identity_beside_the_original_and_goes_again() {
        let (session, path, ids) = transform_session();
        let mut copy = patched(&session)
            .into_iter()
            .find(|fixture| fixture["fixtureId"] == ids[1].to_string())
            .unwrap();
        let copy_id = uuid::Uuid::new_v4();
        copy["fixtureId"] = serde_json::json!(copy_id);
        copy["fixtureNumber"] = serde_json::json!(9);
        copy["location"]["x"] = serde_json::json!(5000);
        let record = super::DeleteRecord {
            fixtures: vec![copy],
            attachments: Vec::new(),
        };
        // Adding is the restoration of what the record holds; Undo is its removal.
        apply(&session, record.restoration().unwrap());
        let now = patched(&session);
        assert_eq!(now.len(), 3);
        let added = now
            .iter()
            .find(|fixture| fixture["fixtureId"] == copy_id.to_string())
            .expect("the copy is patched");
        assert_eq!(added["location"]["x"], 5000);
        let original = now
            .iter()
            .find(|fixture| fixture["fixtureId"] == ids[1].to_string())
            .unwrap();
        assert_eq!(original["location"]["x"], 1000);
        apply(&session, record.removal());
        assert_eq!(patched(&session).len(), 2);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deleting_only_fixtures_that_are_gone_says_so() {
        let (session, path, _) = transform_session();
        let error = deletion(&session, &[uuid::Uuid::new_v4()]).unwrap_err();
        assert!(error.contains("exist"), "{error}");
        let _ = std::fs::remove_file(path);
    }
}
