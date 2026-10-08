use super::*;
use serde_json::json;

fn document(name: &str) -> PlanningDocument {
    let base = std::path::PathBuf::from(
        std::env::var_os("LIGHT_TMP_DIR").expect("canonical test temporary directory"),
    );
    let directory = base.join(format!("viz-sync-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    PlanningDocument::create(directory.join("rig.show"), "Rig").unwrap()
}

#[test]
fn a_reread_sees_an_object_deleted_and_recreated_at_the_same_revision() {
    let document = document("reader-recreate");
    let key = ObjectKey::new("rig_attachment", "lamp");
    document
        .put_object("rig_attachment", "lamp", &json!({"pipe": "front"}))
        .unwrap();
    let mut reader = DocumentReader::default();
    assert_eq!(
        reader.read(&document).unwrap().state.objects[&key],
        json!({"pipe": "front"})
    );
    // A CAD undo clears an attachment and puts it back: the store numbers it 1 again.
    document.delete_object("rig_attachment", "lamp").unwrap();
    document
        .put_object("rig_attachment", "lamp", &json!({"pipe": "mid"}))
        .unwrap();
    let reread = reader.read(&document).unwrap();
    assert_eq!(reread.state.objects[&key], json!({"pipe": "mid"}));
    assert_eq!(reread.revisions[&key], 1);
}

#[test]
fn only_synchronized_kinds_and_metadata_are_read() {
    let document = document("reader-kinds");
    document
        .put_object("cad_annotation", "note", &json!({"text": "a"}))
        .unwrap();
    document
        .put_object("route", "artnet", &json!({"protocol": "art_net"}))
        .unwrap();
    document
        .replace_metadata_values(&[
            ("previs.show_version", Some("3")),
            ("description", Some("desk only")),
        ])
        .unwrap();
    let state = DocumentReader::default().read(&document).unwrap().state;
    assert_eq!(
        state.objects.keys().cloned().collect::<Vec<_>>(),
        [ObjectKey::new("cad_annotation", "note")]
    );
    assert_eq!(
        state.metadata.into_iter().collect::<Vec<_>>(),
        [("previs.show_version".to_owned(), "3".to_owned())]
    );
}
