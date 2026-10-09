//! Numeric Info edits address exact physical placements; unlike a gizmo base move, copies stay put.
use super::{
    CadState, EntityTransform, TransformOutcome, attachments, emit_scene_delta, selectable_ids,
};
#[cfg(test)]
use crate::contract::FixtureDto;
use crate::session::Session;
use light_application::{PatchFixtureCandidate, PatchFixturesCommand};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone)]
pub(super) struct NumericPoseRecord {
    pub before: Vec<EntityTransform>,
    pub after: Vec<EntityTransform>,
}

pub(super) fn write_poses(
    session: &Session,
    expected: u64,
    transforms: &[EntityTransform],
) -> Result<(u64, NumericPoseRecord), String> {
    let ids: BTreeSet<_> = transforms.iter().map(|t| t.id).collect();
    if ids.is_empty() || ids.len() != transforms.len() {
        return Err("Set each selected CAD placement exactly once".into());
    }
    if transforms
        .iter()
        .any(|t| t.rotation_degrees.iter().any(|v| !v.is_finite()))
    {
        return Err("CAD rotations must be finite".into());
    }
    let desired: HashMap<_, _> = transforms.iter().map(|t| (t.id, t)).collect();
    session.gesture(|| {
        let selectable = selectable_ids(session)?;
        let (command, record) = session.with(|document| {
            let snapshot = document.patch_snapshot().map_err(|e| e.to_string())?;
            if snapshot.patch_revision.value() != expected {
                return Err(format!(
                    "The rig changed at revision {}; refresh before committing revision {}",
                    snapshot.patch_revision.value(),
                    expected
                ));
            }
            let mut before = Vec::new();
            let mut fixtures = Vec::new();
            for source in snapshot.fixtures {
                let root = source.patch.fixture_id.0;
                let mut fixture = source.patch;
                let touched = desired.contains_key(&root)
                    || fixture
                        .multipatch
                        .iter()
                        .any(|c| desired.contains_key(&c.id));
                if !touched {
                    continue;
                }
                if !selectable.contains(&root) {
                    return Err(
                        "One or more selected CAD placements belong to a locked or hidden layer"
                            .into(),
                    );
                }
                if let Some(t) = desired.get(&root) {
                    before.push(EntityTransform {
                        id: root,
                        position_millimetres: [
                            fixture.location.x,
                            fixture.location.y,
                            fixture.location.z,
                        ],
                        rotation_degrees: [
                            fixture.rotation.x,
                            fixture.rotation.y,
                            fixture.rotation.z,
                        ],
                    });
                    fixture.location.x = t.position_millimetres[0];
                    fixture.location.y = t.position_millimetres[1];
                    fixture.location.z = t.position_millimetres[2];
                    fixture.rotation.x = t.rotation_degrees[0];
                    fixture.rotation.y = t.rotation_degrees[1];
                    fixture.rotation.z = t.rotation_degrees[2];
                }
                for copy in &mut fixture.multipatch {
                    if let Some(t) = desired.get(&copy.id) {
                        before.push(EntityTransform {
                            id: copy.id,
                            position_millimetres: [
                                copy.location.x,
                                copy.location.y,
                                copy.location.z,
                            ],
                            rotation_degrees: [copy.rotation.x, copy.rotation.y, copy.rotation.z],
                        });
                        copy.location.x = t.position_millimetres[0];
                        copy.location.y = t.position_millimetres[1];
                        copy.location.z = t.position_millimetres[2];
                        copy.rotation.x = t.rotation_degrees[0];
                        copy.rotation.y = t.rotation_degrees[1];
                        copy.rotation.z = t.rotation_degrees[2];
                    }
                }
                fixtures.push(PatchFixtureCandidate {
                    profile: source.profile,
                    patch: fixture,
                });
            }
            if before.len() != ids.len() {
                return Err("One or more selected CAD placements no longer exist".into());
            }
            // Retain operator order, including a multi-selection spread.
            let by_id: HashMap<_, _> = before.into_iter().map(|t| (t.id, t)).collect();
            let before: Vec<_> = transforms.iter().map(|t| by_id[&t.id].clone()).collect();
            let record = NumericPoseRecord {
                before,
                after: transforms.to_vec(),
            };
            if record.before.iter().zip(&record.after).all(|(a, b)| {
                a.position_millimetres == b.position_millimetres
                    && a.rotation_degrees == b.rotation_degrees
            }) {
                return Ok((None, record));
            }
            let command = PatchFixturesCommand {
                show_id: document.show_id(),
                fixtures,
                remove_fixture_ids: Vec::new(),
                placements: Vec::new(),
                vector_spreads: Vec::new(),
                fixture_updates: Vec::new(),
            };
            Ok((Some(command), record))
        })?;
        let revision = match command {
            None => expected,
            Some(command) => session.change(|document| {
                document
                    .patch_fixtures_at(command, expected)
                    .map(|outcome| outcome.change.patch_revision.value())
                    .map_err(|e| e.to_string())
            })?,
        };
        Ok((revision, record))
    })
}

pub(super) fn commit_numeric(
    session: &Session,
    cad: &CadState,
    expected: u64,
    transforms: &[EntityTransform],
) -> Result<TransformOutcome, String> {
    session.gesture(|| {
        let held = attachments(session)?;
        let (revision, record) = write_poses(session, expected, transforms)?;
        if revision != expected {
            cad.history.lock().record_numeric(record);
        }
        Ok(TransformOutcome {
            scene_revision: revision,
            transforms: transforms.to_vec(),
            attachments: held,
        })
    })
}

#[tauri::command]
pub fn cad_set_numeric_transforms(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    expected_scene_revision: u64,
    transforms: Vec<EntityTransform>,
) -> Result<TransformOutcome, String> {
    let outcome = commit_numeric(&session, &cad, expected_scene_revision, &transforms)?;
    if outcome.scene_revision != expected_scene_revision {
        emit_scene_delta(&app, &session, &cad, outcome.scene_revision, Vec::new())?;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::tests::transform_session;
    fn snapshot(session: &Session) -> Vec<serde_json::Value> {
        session
            .with(|d| d.patch_snapshot().map_err(|e| e.to_string()))
            .unwrap()
            .fixtures
            .into_iter()
            .map(|f| {
                let mut body = serde_json::to_value(FixtureDto::from(f)).unwrap();
                // Each real write advances authority revision; compare all actual fixture fields.
                body.as_object_mut().unwrap().remove("fixtureRevision");
                body
            })
            .collect()
    }
    fn revision(session: &Session) -> u64 {
        session
            .with(|d| d.patch_revision().map_err(|e| e.to_string()))
            .unwrap()
    }
    fn transform(id: uuid::Uuid, x: i32, rotation: f32) -> EntityTransform {
        EntityTransform {
            id,
            position_millimetres: [x, 2000, 3000],
            rotation_degrees: [0., 0., rotation],
        }
    }
    #[test]
    fn numeric_base_and_copy_are_exact_and_restore_persisted_poses() {
        let (session, path, ids) = transform_session();
        let cad = CadState::default();
        let original = snapshot(&session);
        let start = revision(&session);
        let first =
            commit_numeric(&session, &cad, start, &[transform(ids[0], -2500, 30.)]).unwrap();
        let changed = snapshot(&session);
        let root = changed
            .iter()
            .find(|f| f["fixtureId"] == ids[0].to_string())
            .unwrap();
        let old = original
            .iter()
            .find(|f| f["fixtureId"] == ids[0].to_string())
            .unwrap();
        assert_eq!(root["location"]["x"], -2500);
        assert_eq!(root["rotation"]["z"], 30.);
        assert_eq!(root["multipatch"], old["multipatch"]);
        let copy = uuid::Uuid::parse_str(old["multipatch"][0]["id"].as_str().unwrap()).unwrap();
        let (_, record) = write_poses(
            &session,
            first.scene_revision,
            &[transform(copy, 6000, 60.)],
        )
        .unwrap();
        let after_copy = snapshot(&session);
        let copied = after_copy
            .iter()
            .find(|f| f["fixtureId"] == ids[0].to_string())
            .unwrap();
        assert_eq!(copied["location"], root["location"]);
        assert_eq!(copied["rotation"], root["rotation"]);
        assert_eq!(copied["multipatch"][1], old["multipatch"][1]);
        write_poses(&session, revision(&session), &record.before).unwrap();
        assert_eq!(snapshot(&session), changed);
        write_poses(&session, revision(&session), &record.after).unwrap();
        assert_eq!(snapshot(&session), after_copy);
        let reopened = Session::default();
        reopened.open(&path).unwrap();
        assert_eq!(snapshot(&reopened), after_copy);
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn numeric_undo_preserves_newer_nonpose_patch_fields() {
        let (session, path, ids) = transform_session();
        let (_, record) = write_poses(
            &session,
            revision(&session),
            &[transform(ids[0], -2500, 30.)],
        )
        .unwrap();
        session
            .change(|d| {
                let source = d
                    .patch_snapshot()
                    .map_err(|e| e.to_string())?
                    .fixtures
                    .into_iter()
                    .find(|f| f.patch.fixture_id.0 == ids[0])
                    .unwrap();
                let mut patch = source.patch;
                patch.name = "Newer operator name".into();
                patch.note = Some("Keep this note".into());
                d.patch_fixtures(PatchFixturesCommand {
                    show_id: d.show_id(),
                    fixtures: vec![PatchFixtureCandidate {
                        profile: source.profile,
                        patch,
                    }],
                    remove_fixture_ids: vec![],
                    placements: vec![],
                    vector_spreads: vec![],
                    fixture_updates: vec![],
                })
                .map(|_| ())
                .map_err(|e| e.to_string())
            })
            .unwrap();
        let fresh = session
            .with(|d| d.patch_snapshot().map_err(|e| e.to_string()))
            .unwrap()
            .fixtures
            .into_iter()
            .find(|f| f.patch.fixture_id.0 == ids[0])
            .unwrap();
        write_poses(&session, revision(&session), &record.before).unwrap();
        let restored = session
            .with(|d| d.patch_snapshot().map_err(|e| e.to_string()))
            .unwrap()
            .fixtures
            .into_iter()
            .find(|f| f.patch.fixture_id.0 == ids[0])
            .unwrap();
        let mut expected = fresh.patch;
        expected.location.x = 0;
        expected.rotation.z = 0.;
        assert_eq!(
            serde_json::to_value(restored.patch).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_and_stale_numeric_batch_is_atomic() {
        let (session, path, ids) = transform_session();
        let cad = CadState::default();
        let before = snapshot(&session);
        let rev = revision(&session);
        for poses in [
            vec![
                transform(ids[0], 100, 0.),
                transform(uuid::Uuid::new_v4(), 200, 0.),
            ],
            vec![transform(ids[0], 100, f32::NAN)],
            vec![transform(ids[0], 100, 0.), transform(ids[0], 200, 0.)],
        ] {
            assert!(commit_numeric(&session, &cad, rev, &poses).is_err());
            assert_eq!(snapshot(&session), before);
        }
        assert!(commit_numeric(&session, &cad, rev - 1, &[transform(ids[0], 100, 0.)]).is_err());
        assert_eq!(snapshot(&session), before);
        session
            .change(|d| {
                d.put_object(
                    "patch_layer",
                    "default",
                    &serde_json::json!({"id":"default","locked":true}),
                )
                .map(|_| ())
                .map_err(|e| e.to_string())
            })
            .unwrap();
        assert!(
            commit_numeric(
                &session,
                &cad,
                revision(&session),
                &[transform(ids[0], 100, 0.)]
            )
            .is_err()
        );
        assert_eq!(snapshot(&session), before);
        session
            .change(|d| {
                d.put_object(
                    "patch_layer",
                    "default",
                    &serde_json::json!({"id":"default","locked":false}),
                )
                .map(|_| ())
                .map_err(|e| e.to_string())
            })
            .unwrap();
        let rev = revision(&session);
        let no_op = commit_numeric(&session, &cad, rev, &[transform(ids[0], 0, 0.)]).unwrap();
        assert_eq!(no_op.scene_revision, rev);
        let _ = std::fs::remove_file(path);
    }
}
