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
fn rejected_canonical_open_keeps_the_current_document() {
    let directory = temporary("canonical-open-recovery");
    let current = directory.join("current.show");
    let rejected = directory.join("future.show");
    let document = viz_document::PlanningDocument::create(&current, "Current").unwrap();
    let current_id = document.show_id();
    drop(document);
    viz_document::PlanningDocument::create(&rejected, "Future").unwrap();
    let store = light_show::ShowStore::open(&rejected).unwrap();
    store
        .set_metadata_values(&[("light.programming_contract", "65535")])
        .unwrap();
    drop(store);
    let original = std::fs::read(&rejected).unwrap();
    let session = session_with_store(&directory);
    session.open(&current).unwrap();
    // Real synchronization authority, using an unpolled current-thread runtime: no sockets,
    // remote writes or live application. A failed load must not discard this engine.
    struct OfflineHost;
    impl viz_sync::DocumentHost for OfflineHost {
        fn apply_remote(
            &self,
            _: &mut dyn FnMut(&viz_document::PlanningDocument) -> Result<bool, String>,
        ) -> Result<(), String> {
            Err("offline test host".into())
        }
        fn status_changed(&self, _: &viz_sync::SyncStatus) {}
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    session.set_sync_host(std::sync::Arc::new(OfflineHost), runtime.handle().clone());
    let binding = SyncBinding::new(
        None,
        current_id.0,
        "http://127.0.0.1:1".into(),
        "Offline desk".into(),
        0,
    );
    session.open_from_desk(&current, binding.clone()).unwrap();
    session.sync_engine().unwrap().set_online(false);
    let status = serde_json::to_value(session.sync_engine().unwrap().status()).unwrap();
    assert!(session.open(&rejected).unwrap_err().contains("65535"));
    assert_eq!(
        session.with(|document| Ok(document.show_id())).unwrap(),
        current_id
    );
    assert_eq!(
        session.sync_engine().unwrap().binding().association_id,
        binding.association_id
    );
    assert_eq!(
        serde_json::to_value(session.sync_engine().unwrap().status()).unwrap(),
        status
    );
    assert_eq!(std::fs::read(&rejected).unwrap(), original);
    session.set_binding(None).unwrap();
    drop(session);
    drop(runtime);
    std::fs::remove_dir_all(directory).unwrap();
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
