//! Preview values the planning window sets, for looking at a rig with no desk on the network.
//!
//! This is the planning provider's already-specified capability, not a hole in the MVP's two-plane
//! rule: the *lighting-desk* provider may not carry live values over the API and must receive real
//! Art-Net or sACN, while the planning provider may send local preview values through the
//! canonical renderer protocol. So this plane exists only here, and the renderer consumes it
//! exactly as it consumes a universe that arrived over the network.
//!
//! Preview values are session state of the planning window. They are never written into the show
//! file and never become a preset, a cue, or a stored look.
//!
//! ## Why universes rather than parameters
//!
//! The renderer already decodes DMX universes into a picture, and the fixture library is already
//! authoritative for channels, fine bytes, ranges, splits and logical heads. Projecting preview
//! values into universe frames here means Simple mode and Full DMX mode are the same path, the
//! precedence rule against real input is expressible per universe, and the renderer needs no
//! second way to be told what a fixture is doing.

use light_fixture::{FixtureMode, FixtureProfile};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Slots in one DMX universe.
pub const DMX_SLOTS: usize = 512;

/// The semantic parameters Simple mode offers.
///
/// Deliberately the five the plan names and no more. Anything that starts to need cues, tracking
/// or arbitration belongs on a desk, and the answer there is to connect to one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewParameter {
    Intensity,
    Pan,
    Tilt,
    Colour,
    Gobo,
}

/// One thing the operator set in the planning window.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PreviewSet {
    /// A semantic parameter, normalised 0..=1. Colour carries all three components because a
    /// colour is one operator gesture even where it is three or four channels.
    Semantic {
        fixture_id: Uuid,
        parameter: PreviewParameter,
        value: f32,
        /// Only read for [`PreviewParameter::Colour`], as red/green/blue in 0..=1.
        #[serde(default)]
        colour: [f32; 3],
    },
    /// One raw slot of a fixture's own footprint, by split and 1-based offset within that split.
    ///
    /// Full DMX mode sets these. They are expressed against the fixture rather than against a
    /// universe so that repatching the fixture moves its preview values with it instead of
    /// leaving them on somebody else's address.
    Slot {
        fixture_id: Uuid,
        split: u16,
        offset: u16,
        value: u8,
    },
}

impl PreviewSet {
    fn fixture_id(&self) -> Uuid {
        match self {
            Self::Semantic { fixture_id, .. } | Self::Slot { fixture_id, .. } => *fixture_id,
        }
    }
}

/// Everything the operator has set, in the order it has to be applied.
///
/// Semantic values are held per fixture and parameter, and raw slots per fixture, split and
/// offset, so setting the same thing twice replaces it rather than accumulating.
#[derive(Clone, Debug, Default)]
pub struct PreviewState {
    semantic: BTreeMap<(Uuid, String), PreviewSet>,
    slots: BTreeMap<(Uuid, u16, u16), PreviewSet>,
}

impl PreviewState {
    pub fn apply(&mut self, set: PreviewSet) {
        match set {
            PreviewSet::Semantic {
                fixture_id,
                parameter,
                ..
            } => {
                self.semantic
                    .insert((fixture_id, format!("{parameter:?}")), set);
            }
            PreviewSet::Slot {
                fixture_id,
                split,
                offset,
                ..
            } => {
                self.slots.insert((fixture_id, split, offset), set);
            }
        }
    }

    /// Drop everything, which is what the clear-to-defaults action does.
    pub fn clear(&mut self) {
        self.semantic.clear();
        self.slots.clear();
    }

    /// Drop everything belonging to these fixtures only.
    pub fn clear_fixtures(&mut self, fixtures: &[Uuid]) {
        self.semantic
            .retain(|(fixture, _), _| !fixtures.contains(fixture));
        self.slots
            .retain(|(fixture, _, _), _| !fixtures.contains(fixture));
    }

    pub fn is_empty(&self) -> bool {
        self.semantic.is_empty() && self.slots.is_empty()
    }

    fn sets(&self) -> impl Iterator<Item = &PreviewSet> {
        self.semantic.values().chain(self.slots.values())
    }
}

/// One universe of preview values, ready for the renderer.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreviewUniverse {
    pub universe: u16,
    /// All 512 slots. Sent whole because a preview universe is small, changes are coalesced by
    /// the renderer refetching, and a partial frame would need its own merge rule at the far end.
    pub slots: Vec<u8>,
}

/// What the renderer reads.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PreviewSnapshot {
    pub revision: u64,
    pub universes: Vec<PreviewUniverse>,
}

/// Project the operator's preview values onto universes, using the show's own patch.
///
/// Every fixture is resolved through its embedded profile revision, so channels, fine bytes,
/// ranges, splits and logical heads come from the fixture library exactly as they do on a desk.
/// A fixture that is unpatched contributes nothing: it is still part of the show and still
/// selectable, but there is no universe to put it on.
pub fn project(
    state: &PreviewState,
    patch: &light_application::PatchSnapshot,
    revision: u64,
) -> PreviewSnapshot {
    let mut universes: BTreeMap<u16, [u8; DMX_SLOTS]> = BTreeMap::new();
    let profiles = resolved_profiles(patch);

    for set in state.sets() {
        let Some(fixture) = patch
            .fixtures
            .iter()
            .find(|candidate| candidate.patch.fixture_id.0 == set.fixture_id())
        else {
            continue;
        };
        let Some(profile) = profiles.get(&(
            fixture.profile.profile_id.0,
            fixture.profile.profile_revision,
        )) else {
            continue;
        };
        let Some(mode) = profile
            .modes
            .iter()
            .find(|mode| mode.id == fixture.profile.mode_id)
        else {
            continue;
        };
        let Ok(plan) = mode.compile_encoding_plan() else {
            continue;
        };

        match set {
            PreviewSet::Slot {
                split,
                offset,
                value,
                ..
            } => {
                let Some(patched) = fixture
                    .patch
                    .split_patches
                    .iter()
                    .find(|candidate| candidate.split == *split)
                else {
                    continue;
                };
                let (Some(universe), Some(address)) = (patched.universe, patched.address) else {
                    continue;
                };
                let footprint = plan.split_footprint(*split).unwrap_or(0);
                if *offset == 0 || *offset > footprint {
                    continue;
                }
                let slot = usize::from(address) + usize::from(*offset) - 2;
                if let Some(target) = universes
                    .entry(universe)
                    .or_insert([0; DMX_SLOTS])
                    .get_mut(slot)
                {
                    *target = *value;
                }
            }
            PreviewSet::Semantic {
                parameter,
                value,
                colour,
                ..
            } => {
                let channels = channels_for(mode, *parameter, *value, *colour);
                for (split, values) in group_by_split(mode, &channels) {
                    let Some(patched) = fixture
                        .patch
                        .split_patches
                        .iter()
                        .find(|candidate| candidate.split == split)
                    else {
                        continue;
                    };
                    let (Some(universe), Some(address)) = (patched.universe, patched.address)
                    else {
                        continue;
                    };
                    let frame = universes.entry(universe).or_insert([0; DMX_SLOTS]);
                    // A refused batch leaves the frame exactly as it was, which is the encoding
                    // plan's own contract; a fixture patched past the end of its universe simply
                    // does not appear rather than corrupting a neighbour.
                    let _ = plan.encode_split(frame, address, split, &values);
                }
            }
        }
    }

    PreviewSnapshot {
        revision,
        universes: universes
            .into_iter()
            .map(|(universe, slots)| PreviewUniverse {
                universe,
                slots: slots.to_vec(),
            })
            .collect(),
    }
}

/// The profiles the snapshot carries, keyed by the revision each fixture actually embeds.
fn resolved_profiles(
    patch: &light_application::PatchSnapshot,
) -> BTreeMap<(Uuid, u64), FixtureProfile> {
    patch
        .profile_revisions
        .iter()
        .filter_map(|revision| {
            let profile: FixtureProfile =
                serde_json::from_value(revision.profile_snapshot.clone()).ok()?;
            Some(((revision.profile_id.0, revision.profile_revision), profile))
        })
        .collect()
}

/// Which channels one semantic parameter drives, and what raw value each takes.
///
/// Every head that has the attribute gets it: Simple mode is a look at a fixture, not a per-head
/// editor, and a wash with three cells should light all three.
fn channels_for(
    mode: &FixtureMode,
    parameter: PreviewParameter,
    value: f32,
    colour: [f32; 3],
) -> Vec<(Uuid, u32)> {
    let normalised = value.clamp(0.0, 1.0);
    let mut resolved = Vec::new();
    for channel in &mode.channels {
        let key = &*channel.attribute.0;
        let fraction = match parameter {
            PreviewParameter::Intensity if key == "intensity" => Some(normalised),
            PreviewParameter::Pan if key == "pan" => Some(normalised),
            PreviewParameter::Tilt if key == "tilt" => Some(normalised),
            PreviewParameter::Gobo if key.starts_with("gobo") && !key.ends_with(".rotation") => {
                Some(normalised)
            }
            PreviewParameter::Colour => colour_fraction(key, colour),
            _ => None,
        };
        let Some(fraction) = fraction else {
            continue;
        };
        // A subtractive channel is canonically its additive opposite with an inverting transform —
        // a CMY fixture's cyan is `color.red`, inverted — so the transform has to be applied or
        // every such fixture takes the complement of the colour the operator chose.
        let fraction = match channel.canonical_transform {
            light_fixture::CanonicalTransform::Identity => fraction,
            light_fixture::CanonicalTransform::InvertNormalized => 1.0 - fraction,
        };
        resolved.push((channel.id, raw_for(channel.resolution, fraction)));
    }
    resolved
}

/// A colour as this channel expresses it.
///
/// Additive fixtures take the component directly; subtractive ones take its complement, which is
/// what makes one operator colour work on a CMY fixture and an RGB one without the window having
/// to know which it is looking at.
fn colour_fraction(key: &str, [red, green, blue]: [f32; 3]) -> Option<f32> {
    let value = match key {
        "color.red" => red,
        "color.green" => green,
        "color.blue" => blue,
        // Retired spellings, still carried by profile revisions embedded in older shows.
        "color.cyan" => 1.0 - red,
        "color.magenta" => 1.0 - green,
        "color.yellow" => 1.0 - blue,
        // A single white emitter follows the dimmest component, so a saturated colour does not
        // wash out and white stays white.
        "color.white" => red.min(green).min(blue),
        _ => return None,
    };
    Some(value.clamp(0.0, 1.0))
}

/// The raw value for a fraction at this channel's resolution, so fine bytes are used rather than
/// merely present.
fn raw_for(resolution: light_fixture::ChannelResolution, fraction: f32) -> u32 {
    let maximum = match resolution.bytes() {
        1 => u32::from(u8::MAX),
        2 => u32::from(u16::MAX),
        3 => 0x00ff_ffff,
        _ => u32::MAX,
    };
    (f64::from(fraction.clamp(0.0, 1.0)) * f64::from(maximum)).round() as u32
}

/// Encoding is per split, so the resolved channels are grouped the way `encode_split` wants them.
fn group_by_split(mode: &FixtureMode, resolved: &[(Uuid, u32)]) -> Vec<(u16, Vec<(Uuid, u32)>)> {
    let mut grouped: BTreeMap<u16, Vec<(Uuid, u32)>> = BTreeMap::new();
    for &(channel_id, raw) in resolved {
        if let Some(channel) = mode
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
        {
            grouped
                .entry(channel.split)
                .or_default()
                .push((channel_id, raw));
        }
    }
    grouped.into_iter().collect()
}
