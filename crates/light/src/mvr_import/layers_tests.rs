use super::*;
use light_mvr::MvrLayer;

fn document(layers: &[(&str, &str)]) -> MvrDocument {
    MvrDocument {
        layers: layers
            .iter()
            .map(|(id, name)| MvrLayer {
                id: (*id).into(),
                name: (*name).into(),
            })
            .collect(),
        ..Default::default()
    }
}

fn stored(id: &str, name: &str, order: i64) -> (String, serde_json::Value) {
    (
        id.to_owned(),
        serde_json::json!({"id": id, "name": name, "order": order}),
    )
}

#[test]
fn a_new_layer_is_created_under_the_name_the_file_gives_it() {
    let file = document(&[("uuid-truss", "Front Truss"), ("uuid-floor", "Floor")]);
    let mut plan = MvrLayerPlan::new(&file, [stored("default", "Stage", 0)]);

    assert_eq!(plan.layer_for(Some("uuid-floor")), "uuid-floor");
    assert_eq!(plan.layer_for(Some("uuid-truss")), "uuid-truss");
    assert_eq!(
        plan.layer_for(Some("uuid-floor")),
        "uuid-floor",
        "created once"
    );

    let created: Vec<_> = plan
        .created()
        .iter()
        .map(|(id, body)| {
            (
                id.as_str(),
                body["name"].as_str().unwrap(),
                body["order"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        created,
        [("uuid-floor", "Floor", 1), ("uuid-truss", "Front Truss", 2)],
        "in the order they were first used, after the show's own layers"
    );
    assert_eq!(plan.created()[0].1["visible3d"], true);
}

#[test]
fn a_layer_the_show_already_has_receives_the_fixtures_by_name() {
    let file = document(&[("uuid-truss", "front truss")]);
    let mut plan = MvrLayerPlan::new(&file, [stored("truss-1", "Front Truss", 3)]);
    assert_eq!(plan.layer_for(Some("uuid-truss")), "truss-1");
    assert!(
        plan.created().is_empty(),
        "re-importing a rig creates no copies"
    );
}

#[test]
fn the_files_default_layer_and_no_layer_land_on_the_patch_default() {
    let file = document(&[("uuid-default", "Default"), ("uuid-unnamed", "")]);
    let mut plan = MvrLayerPlan::new(&file, []);
    assert_eq!(plan.layer_for(Some("uuid-default")), DEFAULT_PATCH_LAYER);
    assert_eq!(plan.layer_for(None), DEFAULT_PATCH_LAYER);
    assert_eq!(plan.layer_for(Some("  ")), DEFAULT_PATCH_LAYER);
    // A layer with no name keeps its identity as its name rather than joining the default.
    assert_eq!(plan.layer_for(Some("uuid-unnamed")), "uuid-unnamed");
    assert_eq!(plan.created().len(), 1);
    assert_eq!(plan.created()[0].1["name"], "uuid-unnamed");
}

#[test]
fn an_undeclared_layer_identity_is_kept_and_named_by_itself() {
    let mut plan = MvrLayerPlan::new(&document(&[]), [stored("mvr", "Imported", 0)]);
    assert_eq!(
        plan.layer_for(Some("mvr")),
        "mvr",
        "an existing identity is reused"
    );
    assert_eq!(plan.layer_for(Some("rig")), "rig");
    assert_eq!(plan.created()[0].1["name"], "rig");
}
