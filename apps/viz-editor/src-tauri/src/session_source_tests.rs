//! Desk bindings are installation state, never part of the portable document.
use crate::{
    session::Session,
    sync::{SyncBinding, SyncBindingStore},
};

fn temporary(name: &str) -> std::path::PathBuf {
    let base = std::path::PathBuf::from(
        std::env::var_os("LIGHT_TMP_DIR").expect("canonical test temporary directory"),
    );
    let directory = base.join(format!("architect-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn session_with_store(directory: &std::path::Path) -> Session {
    let session = Session::default();
    session.set_binding_store(SyncBindingStore::at(directory.join("show-sync")));
    session
}

#[test]
fn a_binding_survives_reopening_but_does_not_travel_with_portable_copies() {
    let directory = temporary("binding-source");
    let path = directory.join("source.show");
    let copy = directory.join("portable.show");
    let document = viz_document::PlanningDocument::create(&path, "Source").unwrap();
    let id = document.show_id().0;
    drop(document);
    let session = session_with_store(&directory);
    session.open(&path).unwrap();
    session
        .set_binding(Some(SyncBinding::new(
            None,
            id,
            "http://127.0.0.1:5000".into(),
            "Desk".into(),
            1,
        )))
        .unwrap();
    session
        .with(|document| document.save_as(&copy).map_err(|e| e.to_string()))
        .unwrap();
    assert!(
        !std::fs::exists(path.with_extension("show.desk-source.json")).unwrap(),
        "no sidecar is written beside the document any more"
    );
    session.open(&copy).unwrap();
    assert!(session.binding.lock().is_none());
    session.open(&path).unwrap();
    assert_eq!(session.binding.lock().as_ref().unwrap().show_id, id);
    drop(session);
    let reopened = session_with_store(&directory);
    reopened.open(&path).unwrap();
    assert_eq!(reopened.binding.lock().as_ref().unwrap().desk_name, "Desk");
    drop(reopened);
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn a_damaged_binding_opens_the_document_standalone() {
    let directory = temporary("binding-damaged-open");
    let path = directory.join("source.show");
    let document = viz_document::PlanningDocument::create(&path, "Source").unwrap();
    let id = document.show_id().0;
    drop(document);
    let session = session_with_store(&directory);
    session.open(&path).unwrap();
    let binding = SyncBinding::new(None, id, "http://127.0.0.1:5000".into(), "Desk".into(), 1);
    session.set_binding(Some(binding.clone())).unwrap();
    std::fs::write(
        directory
            .join("show-sync")
            .join(binding.association_id.to_string())
            .join("binding.json"),
        b"{damaged",
    )
    .unwrap();
    let reopened = session_with_store(&directory);
    let summary = reopened.open(&path).unwrap();
    assert_eq!(summary.name, "Source", "the operator's file still opens");
    assert!(reopened.binding.lock().is_none());
    drop((session, reopened));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn a_save_completion_cannot_advance_a_reopened_copy_of_the_same_desk_show() {
    let directory = temporary("architect-race");
    let path = directory.join("source.show");
    let document = viz_document::PlanningDocument::create(&path, "Source").unwrap();
    let source = SyncBinding::new(
        None,
        document.show_id().0,
        "http://127.0.0.1:5000".into(),
        "Desk".into(),
        0,
    );
    drop(document);
    let session = session_with_store(&directory);
    session.open_from_desk(&path, source).unwrap();
    let (original, generation, _, _) = session.desk_save_snapshot().unwrap();
    session.open(&path).unwrap();
    session.confirm_desk_save(generation, 999).unwrap();
    assert_eq!(
        session
            .binding
            .lock()
            .as_ref()
            .unwrap()
            .acknowledged_show_revision,
        original.acknowledged_show_revision
    );
    drop(session);
    let _ = std::fs::remove_dir_all(directory);
}
