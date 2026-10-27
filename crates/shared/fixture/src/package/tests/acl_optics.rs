use super::*;

#[test]
fn generic_acl_soft_field_preserves_narrow_angles_modes_and_portable_identity() {
    let profile = shipped_profile("generic--acl.toskfixture");
    assert_eq!(
        profile.id.0.to_string(),
        "99385b8d-f553-4e78-b1fd-97b1950c6a73"
    );
    assert_eq!(
        profile
            .modes
            .iter()
            .map(|mode| mode.name.as_str())
            .collect::<Vec<_>>(),
        ["16-bit", "8-bit"]
    );
    for mode in &profile.modes {
        let geometry = profile.mode_geometry(mode);
        assert_eq!(geometry.emitters.len(), 1);
        let emitter = &geometry.emitters[0];
        assert_eq!(
            (emitter.beam_angle_degrees, emitter.field_angle_degrees),
            (8.0, 10.0)
        );
        assert_eq!(emitter.feather, 0.6);
    }
    assert!(profile.notes.contains("not measured ACL photometry"));
    let returned = read_fixture_package(&write_fixture_package(&profile).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&profile).unwrap(),
        serde_json::to_value(returned).unwrap()
    );
}
