//! What the show remembers about an external tracking system.
//!
//! This is show data rather than desk data, and the reason is the bindings: they name 3D Points
//! and Macros, and the zones are boxes in the show's own stage space. All of that describes the
//! production, not the machine it is plugged into, so it travels with the show. The network group
//! travels too — a show that arrives at the venue already knowing which group the tracking system
//! transmits on is one fewer thing to rediscover during focus.
//!
//! Every field has a default and an absent object is a valid disabled configuration, so a show
//! written before any of this existed loads as "tracking off, nothing bound" — which is what it
//! was.

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use uuid::Uuid;

/// The group a PosiStageNet sender transmits to unless it has been moved.
pub const DEFAULT_PSN_GROUP: Ipv4Addr = Ipv4Addr::new(236, 10, 10, 10);
/// The port that goes with [`DEFAULT_PSN_GROUP`].
pub const DEFAULT_PSN_PORT: u16 = 56_565;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PsnConfiguration {
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
    /// Which network card to listen on, when the desk has more than one and the tracking system is
    /// not on the one the routing table would have picked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<Ipv4Addr>,
    /// After this long without a packet a tracker is reported stale. It is not released: a stale
    /// binding still holds its last position.
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
/// PosiStageNet and ToskLight already agree on units and axes — metres, positive x right, positive
/// y up, positive z depth — so the identity is the honest default, and a rig whose origin is the
/// show's origin needs nothing here. What differs in practice is where the tracking system was
/// told its origin is and which way round it was set up, so those are the two knobs.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PsnCalibration {
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
pub struct PsnBinding {
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
pub struct PsnZone {
    pub id: Uuid,
    #[serde(default)]
    pub name: String,
    pub min_metres: [f32; 3],
    pub max_metres: [f32; 3],
    /// Which trackers count. Empty means any of them, which is what a "somebody walked on" zone
    /// wants.
    #[serde(default)]
    pub tracker_ids: Vec<u16>,
    /// Run when the zone becomes occupied. A Macro rather than a playback: the operator already
    /// has a way to say "turn this on", and it is the same way whatever fires it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enter_macro_id: Option<Uuid>,
    /// Run when it becomes empty again. Leaving this unset is how a zone that should not turn
    /// itself off is configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leave_macro_id: Option<Uuid>,
    /// How long the new state has to hold before it counts. A marker on the edge of a zone crosses
    /// it many times a second; without this, so would the macro.
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
    ///
    /// # Errors
    /// When a field would leave the desk listening to nothing, or a zone that cannot be entered.
    pub fn validate(&self) -> Result<(), String> {
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
        self.calibration.validate()?;
        for zone in &self.zones {
            zone.validate()?;
        }
        Ok(())
    }

    /// The bindings that are actually driving something.
    pub fn active_bindings(&self) -> impl Iterator<Item = &PsnBinding> {
        let enabled = self.enabled;
        self.bindings
            .iter()
            .filter(move |binding| enabled && binding.enabled)
    }
}

impl PsnCalibration {
    /// A tracking-system position in the show's own stage space, in metres.
    #[must_use]
    pub fn place_in_show(self, position: [f32; 3]) -> [f32; 3] {
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

    fn validate(&self) -> Result<(), String> {
        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err("the calibration scale must be greater than zero".into());
        }
        if !self.rotation_degrees.is_finite()
            || self.offset_metres.iter().any(|axis| !axis.is_finite())
        {
            return Err("the calibration offset and rotation must be real numbers".into());
        }
        Ok(())
    }
}

impl PsnZone {
    /// Whether a show-space position is inside this box.
    #[must_use]
    pub fn contains(&self, position: [f32; 3]) -> bool {
        (0..3).all(|axis| {
            position[axis] >= self.min_metres[axis] && position[axis] <= self.max_metres[axis]
        })
    }

    /// Whether this zone is watching a given tracker.
    #[must_use]
    pub fn watches(&self, tracker_id: u16) -> bool {
        self.tracker_ids.is_empty() || self.tracker_ids.contains(&tracker_id)
    }

    fn validate(&self) -> Result<(), String> {
        for axis in 0..3 {
            if !self.min_metres[axis].is_finite() || !self.max_metres[axis].is_finite() {
                return Err(format!(
                    "zone {} has a corner that is not a number",
                    self.name
                ));
            }
            if self.min_metres[axis] > self.max_metres[axis] {
                return Err(format!(
                    "zone {} has its low corner above its high corner",
                    self.name
                ));
            }
        }
        if self.dwell_millis > 60_000 {
            return Err(format!(
                "zone {} would have to be held for more than a minute to count",
                self.name
            ));
        }
        Ok(())
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
    DEFAULT_PSN_PORT
}

const fn default_group() -> Ipv4Addr {
    DEFAULT_PSN_GROUP
}

/// A sender transmits at 60 Hz, so a second of silence is roughly sixty missed frames: long enough
/// that a dropped packet is never called an outage, short enough that an operator finds out before
/// the number on screen is meaningless.
const fn default_stale_after_millis() -> u64 {
    1_000
}

/// Long enough that a marker jittering on a boundary fires nothing, short enough that walking into
/// a zone feels like it did it immediately.
const fn default_dwell_millis() -> u64 {
    250
}
