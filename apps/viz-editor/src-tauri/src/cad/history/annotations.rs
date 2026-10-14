//! Drawing commands and history use the same atomic, show-owned object operation.
use super::{AnnotationChange, CadState, Direction, Step};
use crate::annotation::{CadAnnotation, validate};
use crate::session::Session;
use serde_json::Value;

const KIND: &str = "cad_annotation";

pub(crate) fn save_annotation(
    session: &Session,
    cad: &CadState,
    annotation: &CadAnnotation,
    require_existing: bool,
) -> Result<(), String> {
    validate(annotation)?;
    let body = serde_json::to_value(annotation).map_err(|e| e.to_string())?;
    record(session, cad, &annotation.id, Some(body), require_existing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::{CadAnnotationKind, commit_delete, commit_save, read_annotations};
    use crate::cad::{history::take_with, tests::transform_session};

    fn item(kind: CadAnnotationKind) -> CadAnnotation {
        CadAnnotation {
            id: String::new(),
            view: "top_down".into(),
            kind,
            points: if kind == CadAnnotationKind::Text {
                vec![[0., 0.]]
            } else {
                vec![[0., 0.], [1000., 500.]]
            },
            closed: false,
            text: "FOH".into(),
            text_height_millimetres: 250.,
            font: "osifont".into(),
        }
    }

    fn revision(session: &Session) -> u64 {
        session
            .with(|d| d.patch_revision().map_err(|e| e.to_string()))
            .unwrap()
    }

    fn take(
        session: &Session,
        cad: &CadState,
        direction: Direction,
        expected: u64,
    ) -> Result<(), String> {
        session.gesture(|| {
            take_with(&cad.history, direction, |step| {
                let Step::Annotation(change) = step else {
                    panic!("expected annotation step")
                };
                apply_annotation(session, expected, change, direction)
            })
        })
    }

    fn body(session: &Session, id: &str) -> Option<Value> {
        session
            .with(|d| {
                d.object(KIND, id)
                    .map(|o| o.map(|o| o.body))
                    .map_err(|e| e.to_string())
            })
            .unwrap()
    }

    #[test]
    fn drawing_commands_undo_redo_create_change_erase_exact_ids_and_bodies() {
        for kind in [
            CadAnnotationKind::Text,
            CadAnnotationKind::Measure,
            CadAnnotationKind::Box,
            CadAnnotationKind::Polyline,
        ] {
            let (session, path, _) = transform_session();
            let cad = CadState::default();
            let created = commit_save(&session, &cad, item(kind)).unwrap();
            let original = body(&session, &created.id);
            let mut changed = created.clone();
            changed.points[0] = [125., 500.];
            changed.text = "FOH changed".into();
            save_annotation(&session, &cad, &changed, true).unwrap();
            let edited = body(&session, &created.id);
            commit_delete(&session, &cad, &created.id).unwrap();
            assert_eq!(cad.history.lock().undo.len(), 3);
            let other = item(CadAnnotationKind::Text);
            let other_id = "unrelated";
            let mut other_body = serde_json::to_value(other).unwrap();
            other_body["id"] = other_id.into();
            session
                .change(|d| {
                    d.put_object(KIND, other_id, &other_body)
                        .map_err(|e| e.to_string())
                })
                .unwrap();
            take(&session, &cad, Direction::Back, revision(&session)).unwrap();
            assert_eq!(body(&session, &created.id), edited);
            take(&session, &cad, Direction::Back, revision(&session)).unwrap();
            assert_eq!(body(&session, &created.id), original);
            take(&session, &cad, Direction::Back, revision(&session)).unwrap();
            assert!(body(&session, &created.id).is_none());
            for expected in [original, edited, None] {
                take(&session, &cad, Direction::Forward, revision(&session)).unwrap();
                assert_eq!(body(&session, &created.id), expected);
                assert_eq!(body(&session, other_id), Some(other_body.clone()));
            }
            assert_eq!(cad.history.lock().undo.len(), 3);
            assert!(cad.history.lock().redo.is_empty());
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn no_op_invalid_missing_stale_and_foreign_changes_preserve_history() {
        let (session, path, _) = transform_session();
        let cad = CadState::default();
        let created = commit_save(&session, &cad, item(CadAnnotationKind::Text)).unwrap();
        take(&session, &cad, Direction::Back, revision(&session)).unwrap();
        commit_delete(&session, &cad, &created.id).unwrap();
        assert_eq!(cad.history.lock().redo.len(), 1);
        assert!(commit_save(&session, &cad, created.clone()).is_err());
        let mut invalid = item(CadAnnotationKind::Text);
        invalid.text.clear();
        assert!(commit_save(&session, &cad, invalid).is_err());
        assert_eq!(cad.history.lock().redo.len(), 1);
        take(&session, &cad, Direction::Forward, revision(&session)).unwrap();
        commit_save(&session, &cad, created.clone()).unwrap();
        assert_eq!(cad.history.lock().undo.len(), 1);
        let before = body(&session, &created.id);
        assert!(take(&session, &cad, Direction::Back, revision(&session) + 1).is_err());
        assert_eq!(body(&session, &created.id), before);
        assert_eq!(cad.history.lock().undo.len(), 1);
        let mut foreign = before.unwrap();
        foreign["text"] = "Another operator".into();
        foreign["vendor"] = serde_json::json!({"preserve":true});
        session
            .change(|d| {
                d.put_object(KIND, &created.id, &foreign)
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        assert!(take(&session, &cad, Direction::Back, revision(&session)).is_err());
        assert_eq!(body(&session, &created.id), Some(foreign.clone()));
        assert_eq!(cad.history.lock().undo.len(), 1);
        let mut edited = created;
        edited.text = "New edit".into();
        save_annotation(&session, &cad, &edited, true).unwrap();
        assert_eq!(
            body(&session, &edited.id).unwrap()["vendor"],
            foreign["vendor"]
        );
        take(&session, &cad, Direction::Back, revision(&session)).unwrap();
        assert_eq!(body(&session, &edited.id), Some(foreign));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn annotation_history_cannot_apply_to_another_show_or_window() {
        let (session, path, _) = transform_session();
        let cad = CadState::default();
        let created = commit_save(&session, &cad, item(CadAnnotationKind::Text)).unwrap();
        let (other, other_path, _) = transform_session();
        let before = read_annotations(&other).unwrap();
        assert!(
            take(&other, &cad, Direction::Back, revision(&other))
                .unwrap_err()
                .contains("another show")
        );
        assert_eq!(read_annotations(&other).unwrap().len(), before.len());
        assert_eq!(cad.history.lock().undo.len(), 1);
        assert!(
            take(
                &session,
                &CadState::default(),
                Direction::Back,
                revision(&session)
            )
            .is_err()
        );
        take(&session, &cad, Direction::Back, revision(&session)).unwrap();
        assert!(body(&session, &created.id).is_none());
        let missing_session = Session::default();
        assert!(commit_save(&missing_session, &cad, item(CadAnnotationKind::Text)).is_err());
        assert_eq!(cad.history.lock().redo.len(), 1);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(other_path);
    }

    #[test]
    fn unchanged_legacy_annotation_preserves_exact_body_revision_and_redo() {
        let (session, path, _) = transform_session();
        let cad = CadState::default();
        commit_save(&session, &cad, item(CadAnnotationKind::Text)).unwrap();
        take(&session, &cad, Direction::Back, revision(&session)).unwrap();
        let legacy = serde_json::json!({"id":"legacy", "view":"top_down", "kind":"box", "points":[[0,0],[1000,500]], "vendor":{"retain":true}});
        session
            .change(|d| {
                d.put_object(KIND, "legacy", &legacy)
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        let stamp = session
            .with(|d| {
                d.object(KIND, "legacy")
                    .map(|o| o.unwrap().revision)
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        let parsed = serde_json::from_value(legacy.clone()).unwrap();
        commit_save(&session, &cad, parsed).unwrap();
        assert_eq!(body(&session, "legacy"), Some(legacy));
        assert_eq!(
            session
                .with(|d| d
                    .object(KIND, "legacy")
                    .map(|o| o.unwrap().revision)
                    .map_err(|e| e.to_string()))
                .unwrap(),
            stamp
        );
        assert!(cad.history.lock().undo.is_empty());
        assert_eq!(cad.history.lock().redo.len(), 1);
        let _ = std::fs::remove_file(path);
    }
}

pub(crate) fn erase_annotation(session: &Session, cad: &CadState, id: &str) -> Result<(), String> {
    record(session, cad, id, None, false)
}

fn record(
    session: &Session,
    cad: &CadState,
    id: &str,
    after: Option<Value>,
    require_existing: bool,
) -> Result<(), String> {
    session.gesture(|| {
        session.change(|document| {
            let before = document
                .object(KIND, id)
                .map_err(|e| e.to_string())?
                .map(|o| o.body);
            if require_existing && before.is_none() {
                return Err("That drawn item is no longer in the show".into());
            }
            // Missing optional fields in a supported older show are the same values as their
            // reader defaults. Re-saving an unchanged item must neither rewrite it nor end Redo.
            if let (Some(old), Some(new)) = (&before, &after)
                && let (Ok(old), Ok(new)) = (
                    serde_json::from_value::<CadAnnotation>(old.clone()),
                    serde_json::from_value::<CadAnnotation>(new.clone()),
                )
                && old == new
            {
                return Ok(());
            }
            let after = after.map(|mut value| {
                // Preserve unknown authored fields when a supported older editor changes a body.
                if let (Some(Value::Object(old)), Value::Object(new)) = (&before, &mut value) {
                    for (key, value) in old {
                        new.entry(key.clone()).or_insert_with(|| value.clone());
                    }
                }
                value
            });
            if before == after {
                return Ok(());
            }
            match &after {
                Some(body) => document
                    .put_object(KIND, id, body)
                    .map_err(|e| e.to_string())?,
                None => {
                    document
                        .delete_object(KIND, id)
                        .map_err(|e| e.to_string())?;
                }
            }
            let change = AnnotationChange {
                show_id: document.show_id().0,
                id: id.into(),
                before,
                after,
            };
            // Keep document ownership through the history insertion: opening another show
            // cannot interleave the persisted annotation and its matching Undo record.
            let mut history = cad.history.lock();
            history.undo.push(Step::Annotation(Box::new(change)));
            history.redo.clear();
            Ok(())
        })
    })
}

pub(super) fn apply_annotation(
    session: &Session,
    expected_scene_revision: u64,
    change: &AnnotationChange,
    direction: Direction,
) -> Result<(), String> {
    session.change(|document| {
        if document.show_id().0 != change.show_id {
            return Err("This drawing history belongs to another show".into());
        }
        let revision = document.patch_revision().map_err(|e| e.to_string())?;
        if revision != expected_scene_revision {
            return Err("The rig changed; refresh before undoing a drawing".into());
        }
        let (expected, target) = match direction {
            Direction::Back => (&change.after, &change.before),
            Direction::Forward => (&change.before, &change.after),
        };
        let current = document
            .object(KIND, &change.id)
            .map_err(|e| e.to_string())?
            .map(|o| o.body);
        if &current != expected {
            return Err("That drawn item changed elsewhere; refresh before Undo or Redo".into());
        }
        match target {
            Some(body) => document
                .put_object(KIND, &change.id, body)
                .map_err(|e| e.to_string()),
            None => document
                .delete_object(KIND, &change.id)
                .map(|_| ())
                .map_err(|e| e.to_string()),
        }
    })
}
