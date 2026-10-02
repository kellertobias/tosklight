//! Deterministic Direct Color capture/compatibility samples (TL-595) for lifecycle tests.
//!
//! Test support only: compiled for this crate's tests or with the `test-support` feature.
//! Stable UUIDs make every compatibility OUTCOME deterministic. Profile digests hash the whole
//! profile (including geometry), so compare digests only within one [`direct_color_samples`]
//! call. All optical data is synthetic, not a fixture calibration.
use super::*;
use light_core::{
    AttributeKey, FixtureId, NativeColorBinding, NativeColorIdentity, NativeColorValue, Xyz,
    programming::NativeColorObservation,
};
use uuid::Uuid;

/// Source channel order: Red U8, Green U16, Blue U8, UV U32 (raw maxima 255, 65535, 255,
/// u32::MAX). UV has known violet leakage and a model maximum of 50% drive.
pub const SAMPLE_RAW_MAXIMA: [u32; 4] = [255, 65_535, 255, u32::MAX];
/// Visible XYZ of each emitter at full drive (exact binary fractions).
pub const SAMPLE_EMITTER_XYZ: [Xyz; 4] = [
    Xyz {
        x: 0.5,
        y: 0.25,
        z: 0.0,
    },
    Xyz {
        x: 0.25,
        y: 0.75,
        z: 0.125,
    },
    Xyz {
        x: 0.125,
        y: 0.0625,
        z: 1.0,
    },
    Xyz {
        x: 0.0625,
        y: 0.03125,
        z: 0.25,
    },
];
pub const SAMPLE_UV_MAXIMUM: f32 = 0.5;

pub struct DirectColorSamples {
    /// The recorded source: RGB+UV additive head at revision 1.
    pub source: FixtureProfile,
    /// Revision 2 of the same profile: channels reordered (every DMX slot moves), emitter
    /// calibration and model revision changed, identical native layout. Exact replay is
    /// eligible and the saved estimate must not be recomputed.
    pub compatible: FixtureProfile,
    /// Revision 3 of the same profile/mode/head/path: Green became U8. Incompatible layout.
    pub changed_layout: FixtureProfile,
    /// An independently authored lookalike with the same names, attributes, widths and DMX
    /// slots but its own identity. Incompatible source; names and slots prove nothing.
    pub lookalike: FixtureProfile,
    /// Its own profile whose UV leakage is unknown: visible appearance becomes unknown while
    /// UV is active, but the portable UV amount remains known.
    pub unknown_leakage: FixtureProfile,
}

/// Build all samples. The Unknown compatibility case is not a profile: it is a destination
/// whose model cannot be verified (for example, an unprepared or missing catalogue revision),
/// represented by `DirectDestination::Unverified` or an Unavailable catalogue capability.
pub fn direct_color_samples() -> DirectColorSamples {
    let source = profile(0x595_0000, 1);
    let mut compatible = source.clone();
    compatible.revision = 2;
    // UV, Red, Green, Blue: primary slots move from 1/2/4/5 to 2/4/5/1.
    compatible.modes[0].channels.rotate_right(1);
    let model = compatible.modes[0]
        .color_physical
        .as_mut()
        .expect("sample model");
    model.revision = 2;
    emitters_mut(model)[0].xyz = Some(Xyz {
        x: 0.375,
        y: 0.25,
        z: 0.0,
    });
    let mut changed_layout = source.clone();
    changed_layout.revision = 3;
    let mode = &mut changed_layout.modes[0];
    mode.splits[0].footprint = 7;
    mode.channels[1].resolution = ChannelResolution::U8;
    mode.channels[1].secondary_slots.clear();
    mode.channels[1].highlight_raw = 255;
    mode.channels[1].functions[0].dmx_to = 255;
    mode.channels[3].secondary_slots = vec![5, 6, 7];
    let mut unknown_leakage = profile(0x595_2000, 1);
    let model = unknown_leakage.modes[0]
        .color_physical
        .as_mut()
        .expect("sample model");
    emitters_mut(model)[3].xyz = None;
    for profile in [&compatible, &changed_layout, &unknown_leakage] {
        profile.validate().expect("valid Direct Color sample");
    }
    DirectColorSamples {
        source,
        compatible,
        changed_layout,
        lookalike: profile(0x595_1000, 1),
        unknown_leakage,
    }
}

/// The identity of the sample's only head.
pub fn sample_identity(profile: &FixtureProfile) -> NativeColorIdentity {
    profile
        .native_color_identity(profile.modes[0].id, profile.modes[0].heads[0].id)
        .expect("sample identity")
}

/// A complete observation of Red, Green, Blue and UV (source channel order), independent of
/// the profile's current channel order.
pub fn sample_observation(profile: &FixtureProfile, raws: [u32; 4]) -> NativeColorObservation {
    let path = &profile.modes[0]
        .color_physical
        .as_ref()
        .expect("model")
        .paths[0];
    let OpticalSource::Additive { emitters } = &path.source else {
        unreachable!("sample source is additive")
    };
    NativeColorObservation {
        source: sample_identity(profile),
        values: emitters
            .iter()
            .zip(raws)
            .map(|(emitter, raw)| NativeColorValue {
                channel_id: emitter.binding.channel_id,
                function_id: emitter.binding.function_id,
                raw,
            })
            .collect(),
    }
}

fn emitters_mut(model: &mut ColorPhysicalModel) -> &mut Vec<OpticalEmitter> {
    let OpticalSource::Additive { emitters } = &mut model.paths[0].source else {
        unreachable!("sample source is additive")
    };
    emitters
}

fn profile(seed: u128, revision: u32) -> FixtureProfile {
    let id = |n: u128| Uuid::from_u128((seed << 16) + n);
    let mut profile = FixtureProfile::blank();
    profile.id = FixtureId(id(1));
    profile.revision = revision;
    profile.manufacturer = "Synthetic".into();
    profile.name = "Direct Color sample".into();
    profile.short_name = "Direct sample".into();
    let head_id = id(3);
    profile.geometry = GeometryGraph::template(GeometryTemplate::Fixed, &[head_id]);
    let mode = &mut profile.modes[0];
    mode.id = id(2);
    mode.heads[0].id = head_id;
    mode.heads[0].master_shared = false;
    mode.splits[0].footprint = 8;
    let specs = [
        ("Red", "color.red", ChannelResolution::U8, vec![]),
        ("Green", "color.green", ChannelResolution::U16, vec![3]),
        ("Blue", "color.blue", ChannelResolution::U8, vec![]),
        ("UV", "color.uv", ChannelResolution::U32, vec![6, 7, 8]),
    ];
    mode.channels = specs
        .into_iter()
        .zip(0u128..)
        .map(|((name, attribute, resolution, secondary), n)| {
            let attribute = AttributeKey(attribute.into());
            let mut function =
                ChannelFunction::continuous(name, attribute.clone(), resolution.max_raw());
            function.id = id(0x20 + n);
            FixtureChannel {
                id: id(0x10 + n),
                head_id,
                split: 1,
                fixture_attribute: attribute.clone(),
                attribute,
                canonical_transform: CanonicalTransform::Identity,
                resolution,
                secondary_slots: secondary,
                default_raw: 0,
                highlight_raw: resolution.max_raw(),
                physical_min: None,
                physical_max: None,
                unit: None,
                invert: false,
                snap: false,
                reacts_to_virtual_intensity: false,
                virtual_intensity_inverted: false,
                reacts_to_sequence_master: false,
                reacts_to_group_master: false,
                reacts_to_grand_master: false,
                behavior: ChannelBehavior::Controlled,
                functions: vec![function],
            }
        })
        .collect();
    let emitters = mode
        .channels
        .iter()
        .zip(SAMPLE_EMITTER_XYZ)
        .zip(0u128..)
        .map(|((channel, xyz), n)| OpticalEmitter {
            id: id(0x30 + n),
            name: channel.functions[0].name.clone(),
            binding: NativeColorBinding {
                channel_id: channel.id,
                function_id: channel.functions[0].id,
            },
            xyz: Some(xyz),
            spectrum: vec![],
            band: if n == 3 {
                OpticalEmitterBand::Ultraviolet
            } else {
                OpticalEmitterBand::Visible
            },
            native_reversed: false,
            maximum_level: if n == 3 { SAMPLE_UV_MAXIMUM } else { 1.0 },
            response_exponent: 1.0,
            provenance: OpticalProvenance {
                quality: PhysicalDataQuality::Measured,
                source: Some("Synthetic TL-595 sample; not a fixture calibration".into()),
                revision: 1,
            },
        })
        .collect();
    mode.color_physical = Some(ColorPhysicalModel {
        version: 1,
        revision: 1,
        paths: vec![HeadOpticalPath {
            id: id(4),
            head_id,
            controls: mode.channels.iter().map(|c| c.id).collect(),
            source: OpticalSource::Additive { emitters },
            filters: vec![],
            measurements: vec![],
        }],
    });
    profile.validate().expect("valid Direct Color sample");
    profile
}
