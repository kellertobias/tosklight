use super::{read_fixture_package, write_fixture_package};
use crate::{OpticalSource, PhysicalDataQuality};
use std::{fs, path::Path};

#[test]
fn shipped_auro_wheel_retains_nominal_recipes_without_claiming_measurements() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library/cameo--auro-spot-z300.toskfixture");
    let profile = read_fixture_package(&fs::read(path).unwrap()).unwrap();
    let roundtrip = read_fixture_package(&write_fixture_package(&profile).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&profile).unwrap(),
        serde_json::to_value(&roundtrip).unwrap()
    );
    assert_eq!(profile.modes.len(), 2);
    for mode in &profile.modes {
        let model = mode.color_physical.as_ref().unwrap();
        let [path] = model.paths.as_slice() else {
            panic!("one wheel path per mode")
        };
        let OpticalSource::Fixed {
            xyz: Some(_),
            provenance,
            ..
        } = &path.source
        else {
            panic!("{} needs the nominal open source", mode.name);
        };
        assert_eq!(provenance.quality, PhysicalDataQuality::Estimated);
        assert_eq!(path.measurements.len(), 9);
        let red = path
            .measurements
            .iter()
            .find(|sample| sample.recipe.len() == 1 && (6..=11).contains(&sample.recipe[0].raw))
            .expect("manufacturer Deep Red slot must have a whole-path nominal recipe");
        assert_eq!(red.provenance.quality, PhysicalDataQuality::Unknown);
        assert!(
            red.provenance
                .source
                .as_deref()
                .unwrap()
                .contains("Uncalibrated")
        );
        assert_eq!(red.recipe[0].channel_id, path.controls[0]);
        assert!(path.filters.iter().any(|filter| {
            filter.binding.channel_id == red.recipe[0].channel_id
                && filter.binding.function_id == red.recipe[0].function_id
        }));
        assert!(
            path.measurements
                .iter()
                .all(|sample| sample.provenance.quality != PhysicalDataQuality::Measured)
        );
    }
}
