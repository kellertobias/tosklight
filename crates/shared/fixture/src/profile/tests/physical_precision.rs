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

/// Whether a stored 32-bit figure is the nearest float to a value with `decimals` places.
fn stored_at_precision(value: f32, decimals: i32) -> bool {
    let scale = 10f64.powi(decimals);
    value == ((f64::from(value) * scale).round() / scale) as f32
}

/// Every shipped package is authored to the editor's precision, so an operator can save a new
/// revision of any of them without first correcting a figure the library itself shipped.
#[test]
fn shipped_packages_keep_the_editor_precision() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/fixture-library");
    let mut checked = 0;
    let mut problems = Vec::new();
    for entry in std::fs::read_dir(&directory).expect("fixture library directory") {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toskfixture") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("package bytes");
        let profile = crate::package::read_fixture_package(&bytes).expect("package reads");
        let physical = &profile.physical;
        let optics = &profile.optics;
        let figures = [
            ("width_millimetres", physical.width_millimetres, 0),
            ("height_millimetres", physical.height_millimetres, 0),
            ("depth_millimetres", physical.depth_millimetres, 0),
            ("weight_kilograms", physical.weight_kilograms, 2),
            ("power_watts", physical.power_watts, 0),
            // Stored as 0..1 and authored as a percentage to one decimal place.
            ("sharpness", optics.sharpness, 3),
            ("uniformity", optics.uniformity, 3),
        ];
        for (key, value, decimals) in figures {
            if let Some(value) = value
                && !stored_at_precision(value, decimals)
            {
                problems.push(format!("{}: {key} = {value}", path.display()));
            }
        }
        checked += 1;
    }
    assert!(
        checked > 0,
        "no shipped packages under {}",
        directory.display()
    );
    assert!(
        problems.is_empty(),
        "off-precision figures:\n{}",
        problems.join("\n")
    );
}

#[test]
fn precision_check_names_the_figures_the_editor_rejects() {
    assert!(stored_at_precision(1.98, 2));
    assert!(stored_at_precision(258.0, 0));
    assert!(stored_at_precision(0.289, 3));
    assert!(!stored_at_precision(1.975, 2));
    assert!(!stored_at_precision(257.8, 0));
    assert!(!stored_at_precision(0.2895, 3));
}
