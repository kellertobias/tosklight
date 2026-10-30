use serde_json::Value;

/// The backward-compatible placement reader for legacy records and retained Undo bodies.
/// Preserve the old nonzero-coordinate fallback without rewriting explicit flags.
pub fn migrate_legacy_stage_placement(body: &mut Value) {
    mark_legacy_instance(body, true);
    if let Some(copies) = body.get_mut("multipatch").and_then(Value::as_array_mut) {
        for copy in copies {
            mark_legacy_instance(copy, false);
        }
    }
}

fn mark_legacy_instance(body: &mut Value, root: bool) {
    if body.get("stage_position_set").is_some() {
        return;
    }
    let placed_location = body.get("location").is_some_and(|location| {
        ["x", "y", "z"]
            .iter()
            .all(|axis| location.get(*axis).is_some())
            && serde_json::from_value::<crate::FixtureLocation>(location.clone())
                .is_ok_and(|location| location != crate::FixtureLocation::default())
    });
    let linked = root
        && body
            .get("position_master")
            .is_some_and(|value| !value.is_null());
    if let Some(fields) = body.as_object_mut() {
        fields.insert(
            "stage_position_set".into(),
            Value::Bool(placed_location || linked),
        );
    }
}
