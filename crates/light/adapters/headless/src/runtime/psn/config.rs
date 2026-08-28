//! What the operator configured, as the desk stores it.
//!
//! This is show data. The bindings name 3D Points and Macros, the zones are boxes in the show's
//! own stage space, and all of it travels with the show to another desk — the tracking rig is
//! part of the production, not of the machine it is plugged into. The network group is here for
//! the same reason: a show that arrives at the venue already knowing which group the tracking
//! system transmits on is one fewer thing to rediscover at 4pm.
//!
//! Every field has a default, and an absent object is a valid disabled configuration. A show
//! written before this feature existed therefore loads and reads as "PSN off, nothing bound",
//! which is what it was.

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use uuid::Uuid;

/// How far a stored 3D Point offset can reach along one axis, in metres.
///
/// The same number the resolved value is read back with, so that writing a position and reading
/// it produce the same metres. It lives beside the reader in `programmer_aim_command` and is
/// re-exported rather than copied.
pub(in crate::runtime) use super::super::programmer_aim_command::POINT_AXIS_METRES;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::runtime) struct PsnConfiguration {
    #[serde(default = "one")]
    pub version: u32,
    /// Nothing is received and nothing is held while this is off. Turning it off is the coarse
    /// version of unbinding everything, and it is the switch an operator reaches for first.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_group")]
    pub group: Ipv4Addr,
    #[serde(default = "default_port")]
    pub port: u16,
    /// Which network card to listen on, when the desk has more than one and the tracking system
    /// is not on the card the routing table would have picked.
    #[serde(default)]
    pub interface: Option<Ipv4Addr>,
    /// After this long without a packet, a tracker is reported stale to the operator. It is not
    /// released: a stale binding still holds its last position (see `bindings`).
    #[serde(default = "default_stale_after_millis")]
    pub stale_after_millis: u64,
    #[serde(default)]
    pub calibration: PsnCalibration,
    #[serde(default)]
    pub bindings: Vec<PsnBinding>,
    #[serde(default)]
    pub zones: Vec<PsnZone>,
}

/// Where the tracking system's stage is, in the show's stage.
///
/// PSN and ToskLight already agree on units and axes — metres, positive x right, positive y up,
/// positive z depth — so an identity calibration is the honest default and a rig whose origin
/// matches the show's needs nothing here. What differs in practice is where the tracking system
/// was told its origin is, and which way round it was set up, so those are the two knobs.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::runtime) struct PsnCalibration {
    #[serde(default)]
    pub offset_metres: [f32; 3],
    /// About the show's up axis, applied before the offset.
    #[serde(default)]
    pub rotation_degrees: f32,
    #[serde(default = "unit_scale")]
    pub scale: f32,
}

/// One tracker driving one 3D Point.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::runtime) struct PsnBinding {
    pub id: Uuid,
    pub tracker_id: u16,
    /// The 3D Point this tracker *is*. While the binding exists nothing else writes it.
    pub point_fixture_id: Uuid,
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

/// A box on stage, and what the desk does when somebody is in it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::runtime) struct PsnZone {
    pub id: Uuid,
    #[serde(default)]
    pub name: String,
    pub min_metres: [f32; 3],
    pub max_metres: [f32; 3],
    /// Which trackers count. Empty means any of them, which is what a "somebody walked on"
    /// zone wants.
    #[serde(default)]
    pub tracker_ids: Vec<u16>,
    /// Run when the zone becomes occupied. A Macro rather than a playback: the operator already
    /// has a way to say "turn this on", and it is the same way whatever fires it.
    #[serde(default)]
    pub enter_macro_id: Option<Uuid>,
    /// Run when it becomes empty again. Leaving this unset is how a zone that should not turn
    /// itself off is configured.
    #[serde(default)]
    pub leave_macro_id: Option<Uuid>,
    /// How long the new state has to hold before it counts. A marker on the edge of a zone
    /// crosses it many times a second; without this, so would the macro.
    #[serde(default = "default_dwell_millis")]
    pub dwell_millis: u64,
}

impl Default for PsnConfiguration {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: false,
            group: default_group(),
            port: default_port(),
            interface: None,
            stale_after_millis: default_stale_after_millis(),
            calibration: PsnCalibration::default(),
            bindings: Vec::new(),
            zones: Vec::new(),
        }
    }
}

impl Default for PsnCalibration {
    fn default() -> Self {
        Self {
            offset_metres: [0.0; 3],
            rotation_degrees: 0.0,
            scale: 1.0,
        }
    }
}

impl PsnConfiguration {
    /// Say why this cannot be accepted, in the words the operator used to enter it.
    pub(in crate::runtime) fn validate(&self) -> Result<(), String> {
        if !self.group.is_multicast() {
            return Err(format!(
                "{} is not a multicast group; PosiStageNet transmits to one",
                self.group
            ));
        }
        if self.port == 0 {
            return Err("the PSN port must not be 0".into());
        }
        if !(50..=60_000).contains(&self.stale_after_millis) {
            return Err("the stale timeout must be between 50 and 60000 milliseconds".into());
        }
        let calibration = &self.calibration;
        if !calibration.scale.is_finite() || calibration.scale <= 0.0 {
            return Err("the calibration scale must be greater than zero".into());
        }
        if !calibration.rotation_degrees.is_finite()
            || calibration
                .offset_metres
                .iter()
                .any(|axis| !axis.is_finite())
        {
            return Err("the calibration offset and rotation must be real numbers".into());
        }
        for zone in &self.zones {
            for axis in 0..3 {
                if !zone.min_metres[axis].is_finite() || !zone.max_metres[axis].is_finite() {
                    return Err(format!(
                        "zone {} has a corner that is not a number",
                        zone.name
                    ));
                }
                if zone.min_metres[axis] > zone.max_metres[axis] {
                    return Err(format!(
                        "zone {} has its low corner above its high corner",
                        zone.name
                    ));
                }
            }
            if zone.dwell_millis > 60_000 {
                return Err(format!(
                    "zone {} would have to be held for more than a minute to count",
                    zone.name
                ));
            }
        }
        Ok(())
    }

    /// The bindings that are actually driving something.
    pub(in crate::runtime) fn active_bindings(&self) -> impl Iterator<Item = &PsnBinding> {
        let enabled = self.enabled;
        self.bindings
            .iter()
            .filter(move |binding| enabled && binding.enabled)
    }
}

impl PsnCalibration {
    /// A tracking-system position in the show's own stage space, in metres.
    #[must_use]
    pub(in crate::runtime) fn place_in_show(self, position: [f32; 3]) -> [f32; 3] {
        let (sin, cos) = self.rotation_degrees.to_radians().sin_cos();
        let scaled = [
            position[0] * self.scale,
            position[1] * self.scale,
            position[2] * self.scale,
        ];
        // About the up axis only: a tracking system set up facing the other way is the case this
        // exists for, and offering roll and pitch would invite a calibration nobody can check by
        // walking on stage.
        [
            scaled[0] * cos + scaled[2] * sin + self.offset_metres[0],
            scaled[1] + self.offset_metres[1],
            -scaled[0] * sin + scaled[2] * cos + self.offset_metres[2],
        ]
    }
}

impl PsnZone {
    /// Whether a show-space position is inside this box.
    #[must_use]
    pub(in crate::runtime) fn contains(&self, position: [f32; 3]) -> bool {
        (0..3).all(|axis| {
            position[axis] >= self.min_metres[axis] && position[axis] <= self.max_metres[axis]
        })
    }

    /// Whether this zone is watching a given tracker.
    #[must_use]
    pub(in crate::runtime) fn watches(&self, tracker_id: u16) -> bool {
        self.tracker_ids.is_empty() || self.tracker_ids.contains(&tracker_id)
    }
}

const fn one() -> u32 {
    1
}

const fn bool_true() -> bool {
    true
}

const fn unit_scale() -> f32 {
    1.0
}

const fn default_port() -> u16 {
    light_psn_wire::PSN_PORT
}

const fn default_group() -> Ipv4Addr {
    light_psn_wire::PSN_MULTICAST_ADDRESS
}

/// A sender transmits at 60 Hz, so a second of silence is roughly sixty missed frames: long
/// enough that a dropped packet is never called an outage, short enough that an operator finds
/// out before the number on screen is meaningless.
const fn default_stale_after_millis() -> u64 {
    1_000
}

/// Long enough that a marker jittering on the boundary does not fire anything, short enough that
/// walking into a zone feels like it did it immediately.
const fn default_dwell_millis() -> u64 {
    250
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
