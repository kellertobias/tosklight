use serde_json::Value;

pub(super) fn migrate(body: &Value) -> Value {
    let mut migrated = body.clone();
    light_fixture::migrate_legacy_stage_placement(&mut migrated);
    migrated
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stage_placement_migration_uses_the_legacy_coordinate_reader_and_preserves_intent() {
        let body = json!({
            "location":{"x":0.0,"y":1.2,"z":0.0},
            "future":{"keep":true},
            "multipatch":[
                {"location":{"x":0,"y":0,"z":0}},
                {"location":{"x":0,"y":0,"z":0},"stage_position_set":false},
                {"future_copy":42}
            ]
        });
        let migrated = migrate(&body);
        assert_eq!(migrated["stage_position_set"], true);
        assert_eq!(migrated["multipatch"][0]["stage_position_set"], false);
        assert_eq!(migrated["multipatch"][1]["stage_position_set"], false);
        assert_eq!(migrated["multipatch"][2]["stage_position_set"], false);
        assert_eq!(migrated["future"], body["future"]);
        assert_eq!(migrated["location"], body["location"]);
        assert_eq!(migrate(&migrated), migrated);
    }
}
