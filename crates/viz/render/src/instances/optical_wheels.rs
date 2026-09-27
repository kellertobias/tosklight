//! Pack independently controlled fixture wheels without adding lights for their optical copies.
use super::*;
use viz_scene::{OpticalWheelValues, PhysicalMotionState};

pub(super) struct WheelPack {
    pub gobos: [[f32; 4]; MAX_OPTICAL_WHEELS],
    pub prisms: [[f32; 4]; MAX_OPTICAL_WHEELS],
    pub gobo_count: usize,
    pub prism_count: usize,
}

fn rotation(value: f32, motion: &PhysicalMotionState) -> f32 {
    if motion.target.is_some() {
        motion.position_degrees.to_radians()
    } else {
        value * std::f32::consts::TAU
    }
}

pub(super) fn pack(emitter: &EmitterInstance, value: &EmitterValues, half_angle: f32) -> WheelPack {
    let mut out = WheelPack {
        gobos: [[0.0, 0.0, -1.0, 0.0]; MAX_OPTICAL_WHEELS],
        prisms: [[0.0; 4]; MAX_OPTICAL_WHEELS],
        gobo_count: value
            .gobo_wheels
            .len()
            .max(emitter.optics.gobo_wheels.len())
            .max(1)
            .min(MAX_OPTICAL_WHEELS),
        prism_count: value
            .prism_wheels
            .len()
            .max(emitter.optics.prism_wheels.len())
            .max(1)
            .min(MAX_OPTICAL_WHEELS),
    };
    for index in 0..out.gobo_count {
        let wheel = emitter
            .optics
            .gobo_wheels
            .get(index)
            .or_else(|| (index == 0).then_some(&emitter.optics.gobo_wheel));
        let slots = wheel
            .filter(|slots| !slots.is_empty())
            .map_or(GOBO_SLOTS, |slots| slots.len() as u32);
        let slot = value.gobo_wheels.get(index).map_or_else(
            || {
                if index == 0 {
                    value.gobo_slot(slots)
                } else {
                    0
                }
            },
            |wheel| wheel.slot(slots),
        );
        let angle = value.gobo_wheels.get(index).map_or_else(
            || {
                if index == 0 {
                    rotation(value.gobo_rotation, &value.gobo_rotation_motion)
                } else {
                    0.0
                }
            },
            |wheel| rotation(wheel.rotation, &wheel.rotation_motion),
        );
        let artwork = wheel
            .and_then(|slots| slots.get(slot as usize))
            .and_then(|entry| entry.artwork)
            .map_or(-1.0, |layer| layer as f32);
        out.gobos[index] = [slot as f32, angle, artwork, 0.0];
    }
    for index in 0..out.prism_count {
        let state = value.prism_wheels.get(index);
        let legacy = OpticalWheelValues {
            position: if index == 0 { value.prism } else { 0.0 },
            ..Default::default()
        };
        let selected = state.unwrap_or(&legacy);
        let angle = state.map_or_else(
            || {
                if index == 0 {
                    rotation(value.prism_rotation, &value.prism_rotation_motion)
                } else {
                    0.0
                }
            },
            |wheel| rotation(wheel.rotation, &wheel.rotation_motion),
        );
        let declared = emitter
            .optics
            .prism_wheels
            .get(index)
            .filter(|slots| !slots.is_empty());
        if let Some(slots) = declared {
            let slot = selected.slot(slots.len() as u32);
            if let Some(prism) = slots.get(slot as usize) {
                out.prisms[index] = [
                    prism.facets as f32,
                    angle,
                    if prism.linear { 1.0 } else { 0.0 },
                    (prism.spread_degrees.to_radians().tan() / half_angle.tan().max(0.002))
                        .clamp(0.0, 0.95),
                ];
            }
        } else {
            let generic = EmitterValues {
                prism: selected.position,
                ..Default::default()
            };
            out.prisms[index] = [generic.prism_facets() as f32, angle, 0.0, 0.62];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_scene::{GoboSlot, PrismSlot};

    #[test]
    fn two_wheels_keep_their_own_artwork_rotation_and_prism_shape() {
        let mut emitter = super::super::tests::head();
        emitter.optics.gobo_wheels = vec![
            vec![
                GoboSlot::default(),
                GoboSlot {
                    artwork: Some(3),
                    name: "A".into(),
                },
            ],
            vec![
                GoboSlot::default(),
                GoboSlot {
                    artwork: Some(7),
                    name: "B".into(),
                },
            ],
        ];
        emitter.optics.prism_wheels = vec![
            vec![
                PrismSlot::default(),
                PrismSlot {
                    facets: 8,
                    linear: false,
                    spread_degrees: 3.0,
                },
            ],
            vec![
                PrismSlot::default(),
                PrismSlot {
                    facets: 5,
                    linear: true,
                    spread_degrees: 2.0,
                },
            ],
        ];
        let value = EmitterValues {
            gobo_wheels: vec![
                OpticalWheelValues {
                    position: 0.75,
                    rotation: 0.25,
                    ..Default::default()
                },
                OpticalWheelValues {
                    position: 0.75,
                    rotation: 0.5,
                    ..Default::default()
                },
            ],
            prism_wheels: vec![
                OpticalWheelValues {
                    position: 0.75,
                    ..Default::default()
                };
                2
            ],
            ..Default::default()
        };
        let packed = pack(&emitter, &value, 15.0_f32.to_radians());
        assert_eq!((packed.gobo_count, packed.prism_count), (2, 2));
        assert_eq!(packed.gobos[0][2], 3.0);
        assert_eq!(packed.gobos[1][2], 7.0);
        assert_eq!(packed.gobos[0][1], std::f32::consts::FRAC_PI_2);
        assert_eq!(packed.gobos[1][1], std::f32::consts::PI);
        assert_eq!(&packed.prisms[0][..3], &[8.0, 0.0, 0.0]);
        assert_eq!(&packed.prisms[1][..3], &[5.0, 0.0, 1.0]);
    }
}
