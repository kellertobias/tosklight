use super::*;
use crate::forward::{CompiledOpticsForward, OpticsForwardStatus};

fn example() -> FixtureMode {
    let mut mode = FixtureProfile::blank().modes.remove(0);
    let head = mode.heads[0].id;
    let mut zoom = physical_channel(ChannelResolution::U16, 0, 65535, 50., 5.);
    zoom.head_id = head;
    zoom.attribute = AttributeKey("zoom".into());
    zoom.fixture_attribute = zoom.attribute.clone();
    zoom.functions[0].attribute = zoom.attribute.clone();
    zoom.functions[0].physical_mapping = Some(PhysicalMappingCalibration {
        quality: PhysicalDataQuality::Measured,
        source: Some("Synthetic analytic reference".into()),
        opening_convention: Some(OpeningConvention::Field),
        samples: vec![
            PhysicalMappingPoint {
                raw: 0,
                physical: 50.,
            },
            PhysicalMappingPoint {
                raw: 32768,
                physical: 20.,
            },
            PhysicalMappingPoint {
                raw: 65535,
                physical: 5.,
            },
        ],
        ..Default::default()
    });
    let mut focus = physical_channel(ChannelResolution::U8, 10, 200, 100., 0.);
    focus.head_id = head;
    focus.attribute = AttributeKey("focus".into());
    focus.fixture_attribute = focus.attribute.clone();
    focus.functions[0].attribute = focus.attribute.clone();
    focus.functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 100.,
        physical_max: 0.,
        unit: Some("%".into()),
    };
    mode.channels = vec![zoom, focus];
    mode
}
#[test]
fn zoom_uses_piecewise_degrees_and_convention_while_focus_uses_independent_travel() {
    let m = example();
    let p = CompiledOpticsForward::compile(&m).unwrap();
    let mut out = p.create_output();
    p.evaluate(&[32768, 105], &mut out).unwrap();
    assert_eq!(out[0].zoom.unwrap().degrees, 20.);
    assert_eq!(
        out[0].zoom.unwrap().convention,
        Some(OpeningConvention::Field)
    );
    assert_eq!(out[0].zoom.unwrap().quality, PhysicalDataQuality::Measured);
    assert_eq!(out[0].focus.unwrap().percent, 50.);
    assert!(!out[0].focus.unwrap().nominal);
    p.evaluate(&[32769, 200], &mut out).unwrap();
    assert!((out[0].zoom.unwrap().degrees - (20. - 15. / 32767.)).abs() < 1e-10);
    assert_eq!(out[0].focus.unwrap().percent, 0.);
    p.evaluate(&[65535, 10], &mut out).unwrap();
    assert_eq!(out[0].zoom.unwrap().degrees, 5.);
    assert_eq!(out[0].focus.unwrap().percent, 100.);
}
#[test]
fn unknown_focus_distance_uses_nominal_travel_but_unknown_zoom_does_not_invent_degrees() {
    let mut m = example();
    m.channels[0].functions[0].physical_mapping = None;
    for c in &mut m.channels {
        if let ChannelFunctionBehavior::Continuous { unit, .. } = &mut c.functions[0].behavior {
            *unit = Some("m".into());
        }
    }
    let p = CompiledOpticsForward::compile(&m).unwrap();
    let mut out = p.create_output();
    p.evaluate(&[32768, 105], &mut out).unwrap();
    assert_eq!(out[0].focus.unwrap().percent, 50.);
    assert!(out[0].focus.unwrap().nominal);
    assert_eq!(
        out[0].focus.unwrap().quality,
        PhysicalDataQuality::Estimated
    );
    assert_eq!(
        out[0].zoom_status,
        OpticsForwardStatus::UnknownPhysicalMapping
    );
    assert!(out[0].zoom.is_none());
    p.evaluate(&[32768, 9], &mut out).unwrap();
    assert_eq!(out[0].focus_status, OpticsForwardStatus::UnknownFunction);
    assert!(out[0].focus.is_none());
}
#[test]
fn shared_head_optics_inherit_once_and_own_control_overrides_shared_binding() {
    let mut m = example();
    m.heads[0].master_shared = true;
    let id = Uuid::new_v4();
    m.heads.push(FixtureHead {
        id,
        name: "Second head".into(),
        master_shared: false,
    });
    let p = CompiledOpticsForward::compile(&m).unwrap();
    let mut out = p.create_output();
    p.evaluate(&[0, 10], &mut out).unwrap();
    assert_eq!(out[0].zoom, out[1].zoom);
    assert_eq!(out[0].focus, out[1].focus);
    let mut own = m.channels[1].clone();
    own.id = Uuid::new_v4();
    own.functions[0].id = Uuid::new_v4();
    own.head_id = id;
    m.channels.push(own);
    let p = CompiledOpticsForward::compile(&m).unwrap();
    p.evaluate(&[0, 10, 200], &mut out).unwrap();
    assert_eq!(out[0].focus.unwrap().percent, 100.);
    assert_eq!(out[1].focus.unwrap().percent, 0.);
    let mut ambiguous = m.channels[2].clone();
    ambiguous.id = Uuid::new_v4();
    ambiguous.functions[0].id = Uuid::new_v4();
    m.channels.push(ambiguous);
    let p = CompiledOpticsForward::compile(&m).unwrap();
    p.evaluate(&[0, 10, 200, 10], &mut out).unwrap();
    assert!(out[1].focus.is_none());
    assert_eq!(out[1].focus_status, OpticsForwardStatus::Ambiguous);
}
#[test]
fn final_encoded_values_are_not_inverted_twice_and_unrelated_channels_are_not_projected() {
    let mut m = example();
    m.channels[0].invert = true;
    m.channels[1].invert = true;
    let p = CompiledOpticsForward::compile(&m).unwrap();
    let mut out = p.create_output();
    p.evaluate(&[0, 10], &mut out).unwrap();
    assert_eq!(out[0].zoom.unwrap().degrees, 50.);
    assert_eq!(out[0].focus.unwrap().percent, 100.);
    m.channels[0].functions[0].physical_mapping = None;
    m.channels[0].functions[0].attribute = AttributeKey("iris".into());
    let p = CompiledOpticsForward::compile(&m).unwrap();
    p.evaluate(&[0, 10], &mut out).unwrap();
    assert_eq!(out[0].zoom_status, OpticsForwardStatus::Unsupported);
}
