use super::*;

/// The editor stores whole millimetres and watts, kilograms to ten grams and sharpness and
/// uniformity to a tenth of a percent. The package format has to hand those figures back with no
/// binary noise, or a reloaded profile would read as off-precision in the editor.
#[test]
fn editor_precision_survives_the_package_format_exactly() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Generic".into();
    profile.name = "Precision".into();
    profile.physical.width_millimetres = Some(432.0);
    profile.physical.height_millimetres = Some(99_999.0);
    profile.physical.depth_millimetres = Some(1.0);
    profile.physical.weight_kilograms = Some(1.98);
    profile.physical.power_watts = Some(1_500.0);
    profile.optics.sharpness = Some(0.289);
    profile.optics.uniformity = Some(0.625);
    profile.validate().expect("valid");

    // The desk hands profiles out as JSON text; `to_value` would widen each f32 with noise.
    let text = serde_json::to_string(&profile).expect("serialises");
    let json: serde_json::Value = serde_json::from_str(&text).expect("parses");
    assert_eq!(
        json["physical"]["width_millimetres"],
        serde_json::json!(432.0)
    );
    assert_eq!(json["physical"]["weight_kilograms"].to_string(), "1.98");
    assert_eq!(json["physical"]["power_watts"], serde_json::json!(1500.0));
    assert_eq!(json["optics"]["sharpness"].to_string(), "0.289");
    assert_eq!(json["optics"]["uniformity"].to_string(), "0.625");
    let read_back: FixtureProfile = serde_json::from_value(json).expect("reads back");
    assert_eq!(read_back.physical, profile.physical);
    assert_eq!(read_back.optics, profile.optics);
}

/// Shipped and imported packages written before the editor enforced precision still load; the
/// editor names their off-precision figures instead of the library refusing the file.
#[test]
fn legacy_off_precision_physical_figures_still_load() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Generic".into();
    profile.name = "Legacy".into();
    profile.physical.height_millimetres = Some(498.2);
    profile.physical.weight_kilograms = Some(1.975);
    let json = serde_json::to_string(&profile).expect("serialises");
    let read_back: FixtureProfile = serde_json::from_str(&json).expect("reads back");
    read_back.validate().expect("legacy figures stay loadable");
    assert_eq!(read_back.physical.height_millimetres, Some(498.2));
    assert_eq!(read_back.physical.weight_kilograms, Some(1.975));
}
