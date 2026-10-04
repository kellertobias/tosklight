//! G8c: a shipped Zoom without authored degrees fits a degree request through the derived
//! runtime range and convention (`light_fixture::apply_derived_zoom_physical`), verified through
//! encoded DMX and an independently compiled forward model (`Resolved::verify`).
use super::*;
use light_fixture::apply_runtime_profile_compatibility;

fn shipped(name: &str, mode: &str) -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    let mut profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    profile.modes.retain(|m| m.name == mode);
    assert_eq!(profile.modes.len(), 1, "{name} {mode}");
    apply_runtime_profile_compatibility(&mut profile);
    profile
}

fn beam(degrees: f32) -> AttributeValue {
    zoom(degrees, OpeningConvention::Beam)
}

#[test]
fn a_derived_robin_dls_zoom_fits_beam_degrees_to_the_documented_dmx() {
    let rig = Rig::new(&shipped("robe--robin-dls-profile", "Mode 2"));
    // The DMX chart runs from maximum (45°) to minimum (10°) beam angle.
    let wide = rig.zoom(&beam(45.));
    assert_eq!(wide.result.quality.status, OpticsFitStatus::Fitted);
    assert_eq!((wide.raw(), wide.achieved()), (0, 45.));
    assert_eq!(rig.zoom(&beam(10.)).raw(), 255);
    let middle = rig.zoom(&beam(27.5));
    assert!((127..=128).contains(&middle.raw()), "{}", middle.raw());
    assert!((middle.achieved() - 27.5).abs() <= 35. / 255. / 2. + 1e-6);
    assert_eq!(
        middle.result.quality.convention,
        Some(OpeningConvention::Beam)
    );
    assert_eq!(
        middle.result.quality.data_quality,
        Some(PhysicalDataQuality::Manufacturer)
    );
    // A Field request is checked against the derived Beam convention, never converted.
    let mismatch = rig.zoom(&field(27.5));
    assert_eq!(
        mismatch.result.quality.status,
        OpticsFitStatus::ConventionMismatch
    );
}

#[test]
fn a_derived_percent_zoom_fits_and_a_non_optical_zoom_stays_unknown() {
    // Stage Zoom 1200: percent travel, 0 = wide (24°), full = narrow (16°); estimated.
    let rig = Rig::new(&shipped("claypaky--stage-zoom-1200", "16-Channel"));
    let twenty = rig.zoom(&beam(20.));
    assert_eq!(twenty.result.quality.status, OpticsFitStatus::Fitted);
    assert!((127..=128).contains(&twenty.raw()), "{}", twenty.raw());
    assert_eq!(
        twenty.result.quality.data_quality,
        Some(PhysicalDataQuality::Estimated)
    );

    // Negative control: a cold spark's "zoom" is spark lifetime; no degrees are invented.
    let rig = Rig::new(&shipped(
        "generic--cold-spark",
        "Intensity, Height, Lifetime",
    ));
    let unknown = rig.zoom(&beam(20.));
    assert_eq!(
        unknown.result.quality.status,
        OpticsFitStatus::UnknownPhysicalMapping
    );
    assert_eq!(unknown.result.achieved, None);
}
