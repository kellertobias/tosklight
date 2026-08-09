//! Compiled references from decoded fixture parameters back to physical DMX slots.

use light_fixture::{AngularMotionKind, ChannelFunction, ChannelFunctionBehavior};
use viz_dmx::DMX_SLOTS;
use viz_scene::PhysicalMotionTarget;

pub const FALLBACK_ANGULAR_SPEED: f32 = 540.0;
pub const FALLBACK_ANGULAR_ACCELERATION: f32 = 1_080.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelTarget {
    pub index: usize,
    pub count: usize,
    pub max_speed: f32,
    pub acceleration: f32,
    pub deceleration: f32,
}

/// One channel resolved to absolute universe and slot addresses.
#[derive(Clone, Debug)]
pub struct ChannelRef {
    pub logical_universe: u16,
    /// Absolute 1-based DMX addresses, most significant component first.
    pub slots: Vec<u16>,
    pub max_raw: u32,
    pub invert: bool,
    pub physical_min: f32,
    pub physical_max: f32,
    pub snap: bool,
    pub default_raw: u32,
    pub functions: Vec<ChannelFunction>,
}

impl ChannelRef {
    /// Raw value from a universe frame, or the channel default when the frame is short.
    pub fn raw(&self, frame: &[u8; DMX_SLOTS]) -> u32 {
        let mut raw = 0_u32;
        for slot in &self.slots {
            let index = usize::from(*slot).saturating_sub(1);
            let byte = frame.get(index).copied().unwrap_or(0);
            raw = (raw << 8) | u32::from(byte);
        }
        raw
    }

    /// Normalised `0..=1` value after inversion.
    pub fn normalised(&self, frame: &[u8; DMX_SLOTS]) -> f32 {
        let raw = self.raw(frame);
        let level = raw as f32 / self.max_raw.max(1) as f32;
        if self.invert { 1.0 - level } else { level }
    }

    /// Absolute camera position in metres from an unsigned 24-bit offset-binary channel.
    ///
    /// Camera coordinates deliberately do not use a profile's physical range: the portable
    /// camera contract assigns every raw increment exactly half a millimetre and reserves
    /// `0x800000` as exact scene-origin zero.
    pub fn camera_position_metres(&self, frame: &[u8; DMX_SLOTS]) -> Option<f32> {
        (self.slots.len() == 3 && self.max_raw == 0x00ff_ffff).then(|| {
            let signed = i64::from(self.raw(frame)) - 8_388_608;
            (signed as f64 * 0.0005) as f32
        })
    }

    /// Absolute camera Euler axis in degrees from its unsigned 16-bit wire value.
    pub fn camera_angle_degrees(&self, frame: &[u8; DMX_SLOTS]) -> Option<f32> {
        (self.slots.len() == 2 && self.max_raw == 0xffff)
            .then(|| -360.0 + self.raw(frame) as f32 * (720.0 / 65_535.0))
    }

    /// Full-frame-equivalent focal length and vertical field of view from camera Zoom.
    pub fn camera_lens(&self, frame: &[u8; DMX_SLOTS]) -> Option<(f32, f32)> {
        if self.slots.len() != 2 || self.max_raw != 0xffff {
            return None;
        }
        const WIDE_MILLIMETRES: f32 = 18.0;
        const TELE_MILLIMETRES: f32 = 1_200.0;
        const SENSOR_HEIGHT_MILLIMETRES: f32 = 24.0;
        let level = self.raw(frame) as f32 / 65_535.0;
        let focal_length = WIDE_MILLIMETRES * (TELE_MILLIMETRES / WIDE_MILLIMETRES).powf(level);
        let vertical_fov =
            (2.0 * (SENSOR_HEIGHT_MILLIMETRES / (2.0 * focal_length)).atan()).to_degrees();
        Some((focal_length, vertical_fov))
    }

    /// Physical value in the channel's declared unit.
    pub fn physical(&self, frame: &[u8; DMX_SLOTS]) -> f32 {
        let level = self.normalised(frame);
        self.physical_min + (self.physical_max - self.physical_min) * level
    }

    /// The function whose range contains the current raw value, if any.
    pub fn function<'a>(&'a self, frame: &[u8; DMX_SLOTS]) -> Option<&'a ChannelFunction> {
        let raw = self.raw(frame);
        self.functions
            .iter()
            .filter(|function| raw >= function.dmx_from && raw <= function.dmx_to)
            .max_by_key(|function| function.priority)
    }

    /// Physical value of the matched function, honouring its own range rather than the channel's.
    pub fn function_physical(&self, frame: &[u8; DMX_SLOTS]) -> Option<f32> {
        let raw = self.raw(frame);
        let function = self.function(frame)?;
        match &function.behavior {
            ChannelFunctionBehavior::Continuous {
                physical_min,
                physical_max,
                ..
            } => {
                let span = function.dmx_to.saturating_sub(function.dmx_from).max(1) as f32;
                let position = (raw.saturating_sub(function.dmx_from)) as f32 / span;
                Some(physical_min + (physical_max - physical_min) * position)
            }
            _ => None,
        }
    }

    /// Decode the current function's physical motion without normalising away its raw span.
    /// Legacy Pan/Tilt channels opt into an absolute target with deterministic fast defaults;
    /// other rotating attributes require explicit metadata so an ordinary normalized channel is
    /// never mistaken for degrees.
    pub fn angular_motion_target(
        &self,
        frame: &[u8; DMX_SLOTS],
        legacy_absolute: bool,
    ) -> Option<PhysicalMotionTarget> {
        let function = self.function(frame);
        let motion = function.and_then(|function| function.angular_motion);
        if motion.is_none() && !legacy_absolute {
            return None;
        }
        let physical = self
            .function_physical(frame)
            .unwrap_or_else(|| self.physical(frame));
        let maximum_speed = motion
            .and_then(|motion| motion.max_speed_degrees_per_second)
            .unwrap_or(FALLBACK_ANGULAR_SPEED);
        let acceleration = motion
            .and_then(|motion| motion.acceleration_degrees_per_second_squared)
            .unwrap_or(FALLBACK_ANGULAR_ACCELERATION);
        let deceleration = motion
            .and_then(|motion| motion.deceleration_degrees_per_second_squared)
            .unwrap_or(acceleration);
        match motion.map(|motion| motion.kind) {
            Some(AngularMotionKind::AngularVelocity) => Some(PhysicalMotionTarget::Velocity {
                degrees_per_second: physical.clamp(-maximum_speed, maximum_speed),
                acceleration,
                deceleration,
            }),
            Some(AngularMotionKind::AbsolutePosition) | None => {
                Some(PhysicalMotionTarget::Position {
                    degrees: physical,
                    max_speed: maximum_speed,
                    acceleration,
                    deceleration,
                })
            }
        }
    }

    /// Decode the profile's exact home raw value through the same function table as live DMX.
    pub fn angular_motion_default_target(
        &self,
        legacy_absolute: bool,
    ) -> Option<PhysicalMotionTarget> {
        let mut frame = [0_u8; DMX_SLOTS];
        let bytes = self.default_raw.to_be_bytes();
        let offset = bytes.len().saturating_sub(self.slots.len());
        for (slot, byte) in self.slots.iter().zip(bytes[offset..].iter()) {
            let index = usize::from(*slot).saturating_sub(1);
            if let Some(destination) = frame.get_mut(index) {
                *destination = *byte;
            }
        }
        self.angular_motion_target(&frame, legacy_absolute)
    }

    /// Resolve a discrete wheel slot by function raw-range order, with dynamics authored on the
    /// selected slot (or deterministic physical defaults for legacy profiles).
    pub fn wheel_target(&self, frame: &[u8; DMX_SLOTS]) -> Option<WheelTarget> {
        let selected = self.function(frame)?;
        let mut slots = self
            .functions
            .iter()
            .filter(|function| {
                matches!(
                    function.behavior,
                    ChannelFunctionBehavior::Indexed { .. } | ChannelFunctionBehavior::Fixed { .. }
                )
            })
            .collect::<Vec<_>>();
        slots.sort_by_key(|function| function.dmx_from);
        let index = slots
            .iter()
            .position(|function| function.id == selected.id)?;
        let motion = selected
            .angular_motion
            .or_else(|| slots.iter().find_map(|function| function.angular_motion));
        let max_speed = motion
            .and_then(|motion| motion.max_speed_degrees_per_second)
            .unwrap_or(FALLBACK_ANGULAR_SPEED);
        let acceleration = motion
            .and_then(|motion| motion.acceleration_degrees_per_second_squared)
            .unwrap_or(FALLBACK_ANGULAR_ACCELERATION);
        let deceleration = motion
            .and_then(|motion| motion.deceleration_degrees_per_second_squared)
            .unwrap_or(acceleration);
        Some(WheelTarget {
            index,
            count: slots.len(),
            max_speed,
            acceleration,
            deceleration,
        })
    }

    pub fn wheel_default_target(&self) -> Option<WheelTarget> {
        let mut frame = [0_u8; DMX_SLOTS];
        let bytes = self.default_raw.to_be_bytes();
        let offset = bytes.len().saturating_sub(self.slots.len());
        for (slot, byte) in self.slots.iter().zip(bytes[offset..].iter()) {
            if let Some(destination) = frame.get_mut(usize::from(*slot).saturating_sub(1)) {
                *destination = *byte;
            }
        }
        self.wheel_target(&frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::AttributeKey;
    use uuid::Uuid;

    fn channel(slots: Vec<u16>, max_raw: u32, invert: bool) -> ChannelRef {
        ChannelRef {
            logical_universe: 1,
            slots,
            max_raw,
            invert,
            physical_min: 0.0,
            physical_max: 1.0,
            snap: false,
            default_raw: 0,
            functions: Vec::new(),
        }
    }

    #[test]
    fn a_sixteen_bit_channel_combines_its_components_most_significant_first() {
        let mut frame = [0_u8; DMX_SLOTS];
        frame[0] = 0x12;
        frame[1] = 0x34;
        let channel = channel(vec![1, 2], 0xffff, false);
        assert_eq!(channel.raw(&frame), 0x1234);
        assert!((channel.normalised(&frame) - 0x1234 as f32 / 65_535.0).abs() < 1e-6);
    }

    #[test]
    fn inversion_flips_the_normalised_value() {
        let mut frame = [0_u8; DMX_SLOTS];
        frame[0] = 255;
        assert_eq!(channel(vec![1], 255, true).normalised(&frame), 0.0);
        assert_eq!(channel(vec![1], 255, false).normalised(&frame), 1.0);
    }

    #[test]
    fn physical_conversion_uses_the_declared_range() {
        let mut frame = [0_u8; DMX_SLOTS];
        frame[0] = 128;
        let mut reference = channel(vec![1], 255, false);
        reference.physical_min = -270.0;
        reference.physical_max = 270.0;
        let value = reference.physical(&frame);
        assert!(
            value.abs() < 3.0,
            "midpoint should be near zero, got {value}"
        );
    }

    #[test]
    fn camera_position_is_exact_offset_binary_in_half_millimetres() {
        let reference = channel(vec![1, 2, 3], 0x00ff_ffff, false);
        let mut frame = [0_u8; DMX_SLOTS];
        assert_eq!(reference.camera_position_metres(&frame), Some(-4_194.304));
        frame[..3].copy_from_slice(&[0x80, 0x00, 0x00]);
        assert_eq!(reference.camera_position_metres(&frame), Some(0.0));
        frame[..3].copy_from_slice(&[0xff, 0xff, 0xff]);
        assert_eq!(reference.camera_position_metres(&frame), Some(4_194.3035));
    }

    #[test]
    fn camera_orientation_and_zoom_use_the_stable_wire_mapping() {
        let angle = channel(vec![1, 2], 0xffff, false);
        let lens = channel(vec![3, 4], 0xffff, false);
        let mut frame = [0_u8; DMX_SLOTS];
        assert_eq!(angle.camera_angle_degrees(&frame), Some(-360.0));
        let (wide, wide_fov) = lens.camera_lens(&frame).expect("wide lens");
        assert!((wide - 18.0).abs() < 1e-5);
        assert!((wide_fov - 67.380_135).abs() < 1e-4);
        frame[..2].copy_from_slice(&[0xff, 0xff]);
        frame[2..4].copy_from_slice(&[0xff, 0xff]);
        assert_eq!(angle.camera_angle_degrees(&frame), Some(360.0));
        let (tele, tele_fov) = lens.camera_lens(&frame).expect("tele lens");
        assert!((tele - 1_200.0).abs() < 1e-3);
        assert!((tele_fov - 1.145_877).abs() < 1e-4);
    }

    #[test]
    fn function_ranges_select_the_highest_priority_match() {
        let mut frame = [0_u8; DMX_SLOTS];
        frame[0] = 40;
        let mut reference = channel(vec![1], 255, false);
        reference.functions = vec![
            ChannelFunction {
                id: Uuid::nil(),
                name: "closed".into(),
                dmx_from: 0,
                dmx_to: 63,
                attribute: AttributeKey("shutter".into()),
                priority: 0,
                angular_motion: None,
                behavior: ChannelFunctionBehavior::Fixed {
                    semantic_id: "closed".into(),
                    label: "Closed".into(),
                    raw_value: 0,
                },
            },
            ChannelFunction {
                id: Uuid::nil(),
                name: "strobe".into(),
                dmx_from: 32,
                dmx_to: 200,
                attribute: AttributeKey("shutter".into()),
                priority: 1,
                angular_motion: None,
                behavior: ChannelFunctionBehavior::Continuous {
                    physical_min: 1.0,
                    physical_max: 25.0,
                    unit: Some("hz".into()),
                },
            },
        ];
        let matched = reference.function(&frame).expect("a function matches");
        assert_eq!(matched.name, "strobe");
        let physical = reference.function_physical(&frame).expect("continuous");
        assert!(physical > 1.0 && physical < 25.0);
    }
}
