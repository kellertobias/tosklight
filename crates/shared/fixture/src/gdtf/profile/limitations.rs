//! Position and optics data that a generated GDTF cannot carry.
//!
//! Pan/Tilt and Zoom keep their directed physical endpoints and units through the function
//! writer. What is reported here has no GDTF field, so it is named instead of silently lost.

use super::GdtfExportDiagnostic;
use crate::{FixtureProfile, PhysicalDataQuality};

pub(super) fn position_and_optics(
    profile: &FixtureProfile,
    output: &mut Vec<GdtfExportDiagnostic>,
) {
    let mut report = |node: String, message: &str| {
        output.push(GdtfExportDiagnostic {
            node,
            message: message.to_owned(),
        });
    };
    if profile.geometry.physical_contract.is_some() {
        report(
            "Geometry".into(),
            "The physical geometry contract (coordinates and bracket) has no GDTF field and is not exported.",
        );
    }
    for mode in &profile.modes {
        if mode.position_physical.is_some() {
            report(
                mode.name.clone(),
                "Position physical bindings to geometry nodes are not exported: generated GDTF has no Axis geometry. Pan and Tilt keep their degree ranges.",
            );
        }
        if mode.geometry.physical_contract.is_some() {
            report(
                mode.name.clone(),
                "The mode's physical geometry contract has no GDTF field and is not exported.",
            );
        }
        for channel in &mode.channels {
            for function in &channel.functions {
                let node = format!("{}.{}.{}", mode.name, channel.attribute.0, function.name);
                if function.angular_motion.is_some_and(|motion| {
                    motion.max_speed_degrees_per_second.is_some()
                        || motion.acceleration_degrees_per_second_squared.is_some()
                        || motion.deceleration_degrees_per_second_squared.is_some()
                }) {
                    report(
                        node.clone(),
                        "Maximum angular speed, acceleration and deceleration have no GDTF field and are not exported.",
                    );
                }
                let Some(calibration) = &function.physical_mapping else {
                    continue;
                };
                if calibration.opening_convention.is_some() {
                    report(
                        node.clone(),
                        "GDTF Zoom has no beam/field opening convention; the convention is not exported and a GDTF import leaves it unknown.",
                    );
                }
                if calibration.quality != PhysicalDataQuality::Unknown {
                    report(
                        node,
                        "Physical mapping provenance (quality, source and revision) has no GDTF field; a GDTF import marks the mapping unverified.",
                    );
                }
            }
        }
    }
}
