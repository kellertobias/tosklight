//! Source-desk links are installation state, never part of the portable document.
use crate::{discovery::DeskSource, session::Session};

#[test]
fn source_desk_survives_reopening_but_does_not_travel_with_portable_copies() {
    let base = std::path::PathBuf::from(
        std::env::var_os("LIGHT_TMP_DIR").expect("canonical test temporary directory"),
    );
    let directory = base.join(format!("architect-source-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("source.show");
    let copy = directory.join("portable.show");
    let document = viz_document::PlanningDocument::create(&path, "Source").unwrap();
    let id = document.show_id().0.to_string();
    drop(document);
    let session = Session::default();
    session.open(&path).unwrap();
    session
        .set_desk_source(Some(DeskSource {
            base: "http://127.0.0.1:5000".into(),
            name: "Desk".into(),
            show_id: id.clone(),
            revision: 1,
        }))
        .unwrap();
    session
        .with(|document| document.save_as(&copy).map_err(|e| e.to_string()))
        .unwrap();
    session.open(&copy).unwrap();
    assert!(session.desk_source.lock().is_none());
    session.open(&path).unwrap();
    assert_eq!(session.desk_source.lock().as_ref().unwrap().show_id, id);
    drop(session);
    let reopened = Session::default();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.desk_source.lock().as_ref().unwrap().name, "Desk");
    drop(reopened);
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn a_save_completion_cannot_advance_a_reopened_copy_of_the_same_desk_show() {
    let directory = std::path::PathBuf::from(std::env::var_os("LIGHT_TMP_DIR").unwrap())
        .join(format!("architect-race-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("source.show");
    let document = viz_document::PlanningDocument::create(&path, "Source").unwrap();
    let source = DeskSource {
        base: "http://127.0.0.1:5000".into(),
        name: "Desk".into(),
        show_id: document.show_id().0.to_string(),
        revision: 0,
    };
    drop(document);
    let session = Session::default();
    session.open_from_desk(&path, source).unwrap();
    let (original, generation, _, _) = session.desk_save_snapshot().unwrap();
    session.open(&path).unwrap();
    session.confirm_desk_save(generation, 999).unwrap();
    assert_eq!(
        session.desk_source.lock().as_ref().unwrap().revision,
        original.revision
    );
    drop(session);
    let _ = std::fs::remove_dir_all(directory);
}
