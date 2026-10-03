use super::*;

fn temporary(name: &str) -> PathBuf {
    let base = PathBuf::from(
        std::env::var_os("LIGHT_TMP_DIR").expect("canonical test temporary directory"),
    );
    let directory = base.join(format!("architect-{name}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn binding(show_id: Uuid) -> SyncBinding {
    SyncBinding::new(
        Some(Uuid::new_v4()),
        show_id,
        "http://10.0.0.9:5000".into(),
        "FOH desk".into(),
        7,
    )
}

#[test]
fn a_binding_survives_a_new_store_and_is_found_by_its_document_only() {
    let directory = temporary("binding-roundtrip");
    let document = directory.join("rig.show");
    let copy = directory.join("rig copy.show");
    std::fs::write(&document, b"").unwrap();
    std::fs::write(&copy, b"").unwrap();
    let store = SyncBindingStore::at(directory.join("show-sync"));
    let bound = binding(Uuid::new_v4());
    store.bind(&document, &bound).unwrap();

    let reopened = SyncBindingStore::at(directory.join("show-sync"));
    assert_eq!(
        reopened.for_document(&document).unwrap(),
        Some(bound.clone())
    );
    assert_eq!(
        reopened.for_document(&copy).unwrap(),
        None,
        "a copy of the file carries no binding"
    );

    let mut confirmed = bound.clone();
    confirmed.acknowledged_show_revision = 12;
    reopened.update(&confirmed).unwrap();
    assert_eq!(
        store
            .for_document(&document)
            .unwrap()
            .unwrap()
            .acknowledged_show_revision,
        12
    );
    reopened.unbind(&document).unwrap();
    assert_eq!(store.for_document(&document).unwrap(), None);
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn a_legacy_sidecar_is_ignored_rather_than_migrated() {
    let directory = temporary("binding-sidecar");
    let document = directory.join("legacy.show");
    std::fs::write(&document, b"").unwrap();
    std::fs::write(
        document.with_extension("show.desk-source.json"),
        br#"{"base":"http://desk:5000","name":"Desk","show_id":"00000000-0000-0000-0000-000000000001","revision":3}"#,
    )
    .unwrap();
    let store = SyncBindingStore::at(directory.join("show-sync"));
    assert_eq!(store.for_document(&document).unwrap(), None);
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn a_damaged_index_or_binding_is_reported_and_left_in_place() {
    let directory = temporary("binding-damaged");
    let document = directory.join("rig.show");
    std::fs::write(&document, b"").unwrap();
    let root = directory.join("show-sync");
    let store = SyncBindingStore::at(&root);
    let bound = binding(Uuid::new_v4());
    store.bind(&document, &bound).unwrap();

    let binding_file = root
        .join(bound.association_id.to_string())
        .join("binding.json");
    std::fs::write(&binding_file, b"{not json").unwrap();
    assert!(
        store
            .for_document(&document)
            .unwrap_err()
            .contains("damaged")
    );
    assert_eq!(std::fs::read(&binding_file).unwrap(), b"{not json");

    std::fs::write(root.join("index.json"), b"[").unwrap();
    assert!(
        store
            .for_document(&document)
            .unwrap_err()
            .contains("damaged")
    );
    let _ = std::fs::remove_dir_all(directory);
}
