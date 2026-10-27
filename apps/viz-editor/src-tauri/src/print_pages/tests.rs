use super::*;
use serde_json::json;

fn page() -> CadPrintPage {
    serde_json::from_value(json!({
        "kind":"plan", "id":"ground", "tileId":"tile-a", "name":"Südbahnhof · Ground Plan",
        "view":"top_down", "rotationQuarterTurns":3, "centreMillimetres":[1200.5,-4100.0],
        "widthMillimetres":18000.0, "included":true, "orientation":"portrait",
        "showFixtureIds":true, "showDmxAddresses":true, "showMountingHardware":false,
        "cutPlanes":{"nearMillimetres":-8000.0,"farMillimetres":-5000.0},
        "hiddenUnderlayIds":["section"], "futurePageOption":"preserved"
    }))
    .unwrap()
}
fn temporary() -> std::path::PathBuf {
    let base = std::path::PathBuf::from(
        std::env::var_os("LIGHT_TMP_DIR").expect("canonical test temporary directory"),
    );
    let path = base.join(format!("portable-pages-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn native_reopen_and_copy_preserve_every_page_field_without_leaking_to_another_show() {
    let directory = temporary();
    let first = directory.join("first.show");
    let second = directory.join("second.show");
    let copy = directory.join("copy.show");
    let original = PlanningDocument::create(&first, "Südbahnhof").unwrap();
    let id = original.show_id().0;
    drop(original);
    drop(PlanningDocument::create(&second, "Another show").unwrap());
    let session = Session::default();
    session.open(&first).unwrap();
    // A supported old show has no page object and remains loadable.
    assert!(session.with(read).unwrap().pages.is_none());
    let expected = serde_json::to_value(vec![page()]).unwrap();
    store_pages(&session, id, vec![page()], true, 0).unwrap();
    session
        .with(|d| d.save_as(&copy).map_err(|e| e.to_string()))
        .unwrap();
    session.open(&second).unwrap();
    assert!(
        store_pages(&session, id, vec![page()], false, 0)
            .unwrap_err()
            .contains("changed")
    );
    assert!(session.with(read).unwrap().pages.is_none());
    for path in [&first, &copy] {
        session.open(path).unwrap();
        assert_eq!(
            serde_json::to_value(session.with(read).unwrap().pages.unwrap()).unwrap(),
            expected
        );
    }
    drop(session);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn workspace_adoption_is_conditional_and_an_intentionally_empty_collection_stays_empty() {
    let directory = temporary();
    let path = directory.join("old.show");
    let original = PlanningDocument::create(&path, "Legacy").unwrap();
    let id = original.show_id().0;
    drop(original);
    let session = Session::default();
    session.open(&path).unwrap();
    store_pages(&session, id, vec![page()], true, 0).unwrap();
    let mut replacement = page();
    replacement.name = "Wrong repeated adoption".into();
    let result = store_pages(&session, id, vec![replacement], true, 0).unwrap();
    assert_eq!(result.pages.unwrap()[0].name, page().name);
    store_pages(
        &session,
        id,
        vec![],
        false,
        session.with(read).unwrap().revision,
    )
    .unwrap();
    assert!(
        store_pages(&session, id, vec![page()], true, 0)
            .unwrap()
            .pages
            .unwrap()
            .is_empty()
    );
    drop(session);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn malformed_pages_are_refused_instead_of_written() {
    let mut invalid = page();
    invalid.width_millimetres = -1.0;
    assert!(validate(&[invalid]).is_err());
    assert!(validate(&[page(), page()]).is_err());
}

#[test]
fn another_windows_stale_snapshot_cannot_overwrite_a_saved_page() {
    let directory = temporary();
    let path = directory.join("shared.show");
    let original = PlanningDocument::create(&path, "Shared").unwrap();
    let id = original.show_id().0;
    drop(original);
    let session = Session::default();
    session.open(&path).unwrap();
    let initial = store_pages(&session, id, vec![page()], true, 0).unwrap();
    let mut renamed = page();
    renamed.name = "Other window title".into();
    let saved = store_pages(&session, id, vec![renamed], false, initial.revision).unwrap();
    assert!(saved.revision > initial.revision);
    assert!(
        store_pages(&session, id, vec![page()], false, initial.revision)
            .unwrap_err()
            .contains("Another window")
    );
    assert_eq!(
        session.with(read).unwrap().pages.unwrap()[0].name,
        "Other window title"
    );
    drop(session);
    std::fs::remove_dir_all(directory).unwrap();
}
