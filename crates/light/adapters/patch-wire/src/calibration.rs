//! Installed Position and color calibration between the application and its wire shape.

use light_fixture as fixture;
use light_wire::v2::patch as wire;

pub fn application_position_calibration(
    value: wire::PatchPositionCalibration,
) -> fixture::InstalledPositionCalibration {
    fixture::InstalledPositionCalibration {
        axis_overrides: value
            .axis_overrides
            .map(|v| fixture::InstalledAxisOverrides {
                version: v.version,
                source_identity: application_position_identity(v.source_identity),
                axes: v
                    .axes
                    .into_iter()
                    .map(|a| fixture::InstalledAxisCalibration {
                        node_id: a.node_id,
                        zero_degrees: a.zero_degrees,
                        invert: a.invert,
                    })
                    .collect(),
            }),
        revision: value.revision,
        quality: match value.quality {
            wire::PatchCalibrationQuality::Unknown => fixture::PhysicalDataQuality::Unknown,
            wire::PatchCalibrationQuality::Estimated => fixture::PhysicalDataQuality::Estimated,
            wire::PatchCalibrationQuality::Manufacturer => {
                fixture::PhysicalDataQuality::Manufacturer
            }
            wire::PatchCalibrationQuality::Measured => fixture::PhysicalDataQuality::Measured,
        },
        source: value.source,
        pan_zero_degrees: value.pan_zero_degrees,
        tilt_zero_degrees: value.tilt_zero_degrees,
    }
}

pub(crate) fn wire_position_calibration(
    value: &fixture::InstalledPositionCalibration,
) -> wire::PatchPositionCalibration {
    wire::PatchPositionCalibration {
        axis_overrides: value
            .axis_overrides
            .as_ref()
            .map(|v| wire::PatchAxisOverrides {
                version: v.version,
                source_identity: wire_position_identity(&v.source_identity),
                axes: v
                    .axes
                    .iter()
                    .map(|a| wire::PatchAxisCalibration {
                        node_id: a.node_id,
                        zero_degrees: a.zero_degrees,
                        invert: a.invert,
                    })
                    .collect(),
            }),
        revision: value.revision,
        quality: match value.quality {
            fixture::PhysicalDataQuality::Unknown => wire::PatchCalibrationQuality::Unknown,
            fixture::PhysicalDataQuality::Estimated => wire::PatchCalibrationQuality::Estimated,
            fixture::PhysicalDataQuality::Manufacturer => {
                wire::PatchCalibrationQuality::Manufacturer
            }
            fixture::PhysicalDataQuality::Measured => wire::PatchCalibrationQuality::Measured,
        },
        source: value.source.clone(),
        pan_zero_degrees: value.pan_zero_degrees,
        tilt_zero_degrees: value.tilt_zero_degrees,
    }
}

fn application_color_identity(v: wire::PatchNativeColorIdentity) -> fixture::NativeColorIdentity {
    fixture::NativeColorIdentity {
        profile_id: v.profile_id,
        profile_revision: v.profile_revision,
        profile_digest: v.profile_digest,
        mode_id: v.mode_id,
        head_id: v.head_id,
        path_id: v.path_id,
        model_revision: v.model_revision,
        native_layout_signature: v.native_layout_signature,
    }
}
pub(crate) fn wire_color_identity(
    v: &fixture::NativeColorIdentity,
) -> wire::PatchNativeColorIdentity {
    wire::PatchNativeColorIdentity {
        profile_id: v.profile_id,
        profile_revision: v.profile_revision,
        profile_digest: v.profile_digest.clone(),
        mode_id: v.mode_id,
        head_id: v.head_id,
        path_id: v.path_id,
        model_revision: v.model_revision,
        native_layout_signature: v.native_layout_signature.clone(),
    }
}
fn application_optical_provenance(v: wire::PatchOpticalProvenance) -> fixture::OpticalProvenance {
    fixture::OpticalProvenance {
        revision: v.revision,
        source: v.source,
        quality: match v.quality {
            wire::PatchCalibrationQuality::Unknown => fixture::PhysicalDataQuality::Unknown,
            wire::PatchCalibrationQuality::Estimated => fixture::PhysicalDataQuality::Estimated,
            wire::PatchCalibrationQuality::Manufacturer => {
                fixture::PhysicalDataQuality::Manufacturer
            }
            wire::PatchCalibrationQuality::Measured => fixture::PhysicalDataQuality::Measured,
        },
    }
}
fn wire_optical_provenance(v: &fixture::OpticalProvenance) -> wire::PatchOpticalProvenance {
    wire::PatchOpticalProvenance {
        revision: v.revision,
        source: v.source.clone(),
        quality: match v.quality {
            fixture::PhysicalDataQuality::Unknown => wire::PatchCalibrationQuality::Unknown,
            fixture::PhysicalDataQuality::Estimated => wire::PatchCalibrationQuality::Estimated,
            fixture::PhysicalDataQuality::Manufacturer => {
                wire::PatchCalibrationQuality::Manufacturer
            }
            fixture::PhysicalDataQuality::Measured => wire::PatchCalibrationQuality::Measured,
        },
    }
}
pub fn application_color_calibration(
    v: wire::PatchColorCalibration,
) -> fixture::InstalledColorCalibration {
    fixture::InstalledColorCalibration {
        version: v.version,
        revision: v.revision,
        paths: v
            .paths
            .into_iter()
            .map(|p| fixture::InstalledColorPathCalibration {
                source_identity: application_color_identity(p.source_identity),
                emitters: p
                    .emitters
                    .into_iter()
                    .map(|e| fixture::InstalledEmitterCalibration {
                        emitter_id: e.emitter_id,
                        output_gain: e.output_gain,
                        provenance: application_optical_provenance(e.provenance),
                    })
                    .collect(),
                measurements: p
                    .measurements
                    .into_iter()
                    .map(|m| fixture::ColorRecipeMeasurement {
                        recipe: m
                            .recipe
                            .into_iter()
                            .map(|v| fixture::NativeColorValue {
                                channel_id: v.channel_id,
                                function_id: v.function_id,
                                raw: v.raw,
                            })
                            .collect(),
                        xyz: light_core::Xyz {
                            x: m.xyz.x,
                            y: m.xyz.y,
                            z: m.xyz.z,
                        },
                        provenance: application_optical_provenance(m.provenance),
                    })
                    .collect(),
            })
            .collect(),
    }
}
pub(crate) fn wire_color_calibration(
    v: &fixture::InstalledColorCalibration,
) -> wire::PatchColorCalibration {
    wire::PatchColorCalibration {
        version: v.version,
        revision: v.revision,
        paths: v
            .paths
            .iter()
            .map(|p| wire::PatchColorPathCalibration {
                source_identity: wire_color_identity(&p.source_identity),
                emitters: p
                    .emitters
                    .iter()
                    .map(|e| wire::PatchEmitterCalibration {
                        emitter_id: e.emitter_id,
                        output_gain: e.output_gain,
                        provenance: wire_optical_provenance(&e.provenance),
                    })
                    .collect(),
                measurements: p
                    .measurements
                    .iter()
                    .map(|m| wire::PatchColorRecipeMeasurement {
                        recipe: m
                            .recipe
                            .iter()
                            .map(|v| wire::PatchNativeColorValue {
                                channel_id: v.channel_id,
                                function_id: v.function_id,
                                raw: v.raw,
                            })
                            .collect(),
                        xyz: wire::PatchColorXyz {
                            x: m.xyz.x,
                            y: m.xyz.y,
                            z: m.xyz.z,
                        },
                        provenance: wire_optical_provenance(&m.provenance),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod color_calibration_tests {
    use super::*;
    #[test]
    fn installed_color_wire_conversion_preserves_exact_identity_and_u32_recipe() {
        let value:fixture::InstalledColorCalibration=serde_json::from_value(serde_json::json!({
            "version":1,"revision":7,"paths":[{"source_identity":{
                "profile_id":uuid::Uuid::from_u128(1),"profile_revision":4,"profile_digest":"a".repeat(64),
                "mode_id":uuid::Uuid::from_u128(2),"head_id":uuid::Uuid::from_u128(3),"path_id":uuid::Uuid::from_u128(4),
                "model_revision":5,"native_layout_signature":"b".repeat(64)},
                "emitters":[{"emitter_id":uuid::Uuid::from_u128(5),"output_gain":0.0,"provenance":{"quality":"measured","source":"Test","revision":3}}],
                "measurements":[{"recipe":[{"channel_id":uuid::Uuid::from_u128(6),"function_id":uuid::Uuid::from_u128(7),"raw":4294967295u32}],
                    "xyz":{"x":0.0,"y":0.0,"z":0.0},"provenance":{"quality":"unknown","revision":0}}]
            }]
        })).unwrap();
        value.validate().unwrap();
        let wire = wire_color_calibration(&value);
        let restored: wire::PatchColorCalibration =
            serde_json::from_value(serde_json::to_value(wire).unwrap()).unwrap();
        assert_eq!(application_color_calibration(restored), value);
    }
}

fn application_position_identity(
    v: wire::PatchPositionCalibrationIdentity,
) -> fixture::PositionCalibrationIdentity {
    fixture::PositionCalibrationIdentity {
        profile_id: v.profile_id,
        mode_id: v.mode_id,
        geometry_digest: v.geometry_digest,
    }
}
pub(crate) fn wire_position_identity(
    v: &fixture::PositionCalibrationIdentity,
) -> wire::PatchPositionCalibrationIdentity {
    wire::PatchPositionCalibrationIdentity {
        profile_id: v.profile_id,
        mode_id: v.mode_id,
        geometry_digest: v.geometry_digest.clone(),
    }
}

#[cfg(test)]
#[test]
fn installed_position_axis_calibration_wire_round_trip() {
    let value = fixture::InstalledPositionCalibration {
        revision: 3,
        quality: fixture::PhysicalDataQuality::Measured,
        source: Some("Synthetic regression".into()),
        pan_zero_degrees: 999.0,
        tilt_zero_degrees: 0.0,
        axis_overrides: Some(fixture::InstalledAxisOverrides {
            version: 1,
            source_identity: fixture::PositionCalibrationIdentity {
                profile_id: uuid::Uuid::new_v4(),
                mode_id: uuid::Uuid::new_v4(),
                geometry_digest: "a".repeat(64),
            },
            axes: vec![fixture::InstalledAxisCalibration {
                node_id: uuid::Uuid::new_v4(),
                zero_degrees: -1080.5,
                invert: true,
            }],
        }),
    };
    let encoded = serde_json::to_value(wire_position_calibration(&value)).unwrap();
    let decoded: wire::PatchPositionCalibration = serde_json::from_value(encoded).unwrap();
    assert_eq!(application_position_calibration(decoded), value);
}
