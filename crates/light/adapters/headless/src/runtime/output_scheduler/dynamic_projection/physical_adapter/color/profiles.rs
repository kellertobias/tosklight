//! Synthetic TL-592 destination profiles with authored physical Color models. Emitter XYZ and
//! spectra mirror the TL-568 fitter references; they are not lamp calibrations.
use light_core::{AttributeKey, FixtureId, Xyz, programming::WhiteTarget, srgb_to_xyz};
use light_fixture::forward::white_target_xyz;
use light_fixture::*;
use uuid::Uuid;

pub(in crate::runtime) fn provenance(quality: PhysicalDataQuality) -> OpticalProvenance {
    OpticalProvenance {
        quality,
        source: Some("Synthetic TL-592 reference".into()),
        revision: 1,
    }
}

pub(in crate::runtime) fn spectrum(f: impl Fn(u32) -> f32) -> Vec<SpectrumSample> {
    (360..=830)
        .map(|nm| SpectrumSample {
            wavelength_nm: nm as f32,
            value: f(nm),
        })
        .collect()
}

pub(in crate::runtime) fn pass(from: u32, to: u32) -> Vec<SpectrumSample> {
    spectrum(move |nm| if (from..=to).contains(&nm) { 1. } else { 0.02 })
}

/// Flat source normalized so its open output has Y ≈ 1.
fn flat_source() -> OpticalSource {
    OpticalSource::Fixed {
        xyz: None,
        spectrum: spectrum(|_| (1.0 / 106.856_915) as f32),
        provenance: provenance(PhysicalDataQuality::Measured),
    }
}

fn channel(
    head: Uuid,
    attribute: &str,
    resolution: ChannelResolution,
    slot: u16,
) -> FixtureChannel {
    let max = resolution.max_raw();
    let attribute = AttributeKey(attribute.into());
    FixtureChannel {
        id: Uuid::new_v4(),
        head_id: head,
        split: 1,
        fixture_attribute: attribute.clone(),
        attribute: attribute.clone(),
        canonical_transform: CanonicalTransform::Identity,
        resolution,
        // Fine bytes follow the coarse slot at every width (U16/U24/U32).
        secondary_slots: (1..resolution.bytes() as u16)
            .map(|fine| slot + fine)
            .collect(),
        default_raw: 0,
        highlight_raw: max,
        physical_min: Some(0.0),
        physical_max: Some(1.0),
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        reacts_to_sequence_master: true,
        reacts_to_group_master: true,
        reacts_to_grand_master: true,
        behavior: ChannelBehavior::Controlled,
        functions: vec![ChannelFunction::continuous(
            attribute.0.to_string(),
            attribute,
            max,
        )],
    }
}

/// Channel 0 is an Intensity channel that Color fitting must never touch.
pub(in crate::runtime) struct Builder {
    profile: FixtureProfile,
    head: Uuid,
    emitters: Vec<OpticalEmitter>,
    filters: Vec<OpticalFilter>,
    controls: Vec<Uuid>,
    slots: u16,
    source: Option<OpticalSource>,
}

impl Builder {
    pub fn new(name: &str) -> Self {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = name.into();
        profile.revision = 1;
        let head = profile.modes[0].heads[0].id;
        profile.modes[0].channels = vec![channel(head, "intensity", ChannelResolution::U8, 1)];
        Self {
            profile,
            head,
            emitters: vec![],
            filters: vec![],
            controls: vec![],
            slots: 1,
            source: None,
        }
    }

    fn channel(&mut self, attribute: &str, resolution: ChannelResolution) -> usize {
        let value = channel(self.head, attribute, resolution, self.slots + 1);
        self.slots += resolution.bytes() as u16;
        self.controls.push(value.id);
        let channels = &mut self.profile.modes[0].channels;
        channels.push(value);
        channels.len() - 1
    }

    pub fn emitter(mut self, attribute: &str, xyz: Option<Xyz>) -> Self {
        self.emitter_with(attribute, ChannelResolution::U8, |e| e.xyz = xyz);
        self
    }

    pub fn emitter_with(
        &mut self,
        attribute: &str,
        resolution: ChannelResolution,
        edit: impl FnOnce(&mut OpticalEmitter),
    ) {
        let index = self.channel(attribute, resolution);
        let native = &self.profile.modes[0].channels[index];
        let mut emitter = OpticalEmitter {
            id: Uuid::new_v4(),
            name: attribute.into(),
            binding: NativeColorBinding {
                channel_id: native.id,
                function_id: native.functions[0].id,
            },
            xyz: None,
            spectrum: vec![],
            band: if attribute == "color.uv" {
                OpticalEmitterBand::Ultraviolet
            } else {
                OpticalEmitterBand::Visible
            },
            native_reversed: false,
            maximum_level: 1.0,
            response_exponent: 1.0,
            provenance: provenance(PhysicalDataQuality::Manufacturer),
        };
        edit(&mut emitter);
        self.emitters.push(emitter);
    }

    /// A wheel/flag channel with steady spectral samples; `rotation` adds an unmodeled spin.
    pub fn filter(
        self,
        attribute: &str,
        slots_to: u32,
        samples: Vec<(u32, u32, Vec<SpectrumSample>)>,
        rotation: bool,
    ) -> Self {
        self.filter_with(
            attribute,
            ChannelResolution::U8,
            slots_to,
            samples,
            rotation,
        )
    }

    /// `filter` on a channel of any width; raw ranges are given in that width's raw units.
    pub fn filter_with(
        mut self,
        attribute: &str,
        resolution: ChannelResolution,
        slots_to: u32,
        samples: Vec<(u32, u32, Vec<SpectrumSample>)>,
        rotation: bool,
    ) -> Self {
        let index = self.channel(attribute, resolution);
        let native = &mut self.profile.modes[0].channels[index];
        native.functions[0].dmx_to = slots_to;
        if rotation {
            let mut spin = ChannelFunction::continuous(
                "Rotation",
                AttributeKey(format!("{attribute}.rotation").into()),
                resolution.max_raw(),
            );
            spin.dmx_from = slots_to + 1;
            native.functions.push(spin);
        }
        self.filters.push(OpticalFilter {
            id: Uuid::new_v4(),
            name: attribute.into(),
            binding: NativeColorBinding {
                channel_id: native.id,
                function_id: native.functions[0].id,
            },
            transmission: OpticalTransmission::Spectral {
                samples: samples
                    .into_iter()
                    .map(|(raw_from, raw_to, spectrum)| FilterSpectrum {
                        raw_from,
                        raw_to,
                        spectrum,
                    })
                    .collect(),
            },
            provenance: provenance(PhysicalDataQuality::Measured),
        });
        self
    }

    pub fn fixed_source(mut self) -> Self {
        self.source = Some(flat_source());
        self
    }

    pub fn build(self) -> FixtureProfile {
        let mut profile = self.profile;
        let mode = &mut profile.modes[0];
        mode.splits[0].footprint = self.slots;
        mode.color_physical = Some(ColorPhysicalModel {
            version: 1,
            revision: 1,
            paths: vec![HeadOpticalPath {
                id: Uuid::new_v4(),
                head_id: self.head,
                controls: self.controls,
                source: self.source.unwrap_or(OpticalSource::Additive {
                    emitters: self.emitters,
                }),
                filters: self.filters,
                measurements: vec![],
            }],
        });
        profile.validate().unwrap();
        profile
    }
}

pub(in crate::runtime) fn rgb_columns() -> [Xyz; 3] {
    [
        srgb_to_xyz(1., 0., 0.),
        srgb_to_xyz(0., 1., 0.),
        srgb_to_xyz(0., 0., 1.),
    ]
}

pub(in crate::runtime) fn xyz(x: f32, y: f32, z: f32) -> Xyz {
    Xyz { x, y, z }
}

/// RGB with the red emitter on a U16 channel (the TL-568 reference layout) plus extras.
pub(in crate::runtime) fn additive(name: &str, extra: &[(&str, Option<Xyz>)]) -> FixtureProfile {
    let [r, g, b] = rgb_columns();
    let mut builder = Builder::new(name);
    builder.emitter_with("color.red", ChannelResolution::U16, |e| e.xyz = Some(r));
    let mut builder = builder
        .emitter("color.green", Some(g))
        .emitter("color.blue", Some(b));
    for (attribute, value) in extra {
        builder = builder.emitter(attribute, *value);
    }
    builder.build()
}

pub(in crate::runtime) fn white() -> Xyz {
    white_target_xyz(WhiteTarget::default())
}

pub(in crate::runtime) fn amber() -> Xyz {
    xyz(
        0.5752 / 0.4242 * 0.4,
        0.4,
        (1. - 0.5752 - 0.4242) / 0.4242 * 0.4,
    )
}

pub(in crate::runtime) fn rgb() -> FixtureProfile {
    additive("TL-592 RGB", &[])
}

pub(in crate::runtime) fn rgbw() -> FixtureProfile {
    additive("TL-592 RGBW", &[("color.white", Some(white()))])
}

/// RGBWA plus UV; `leak` is the UV emitter's known visible leakage (None = unknown).
pub(in crate::runtime) fn rgbwauv(leak: Option<Xyz>) -> FixtureProfile {
    let [r, g, b] = rgb_columns();
    let mut builder = Builder::new("TL-592 RGBWAUV")
        .emitter("color.red", Some(r))
        .emitter("color.green", Some(g))
        .emitter("color.blue", Some(b))
        .emitter("color.white", Some(white()))
        .emitter("color.amber", Some(amber()));
    builder.emitter_with("color.uv", ChannelResolution::U8, |e| {
        e.xyz = leak;
        e.provenance = provenance(PhysicalDataQuality::Estimated);
    });
    builder.build()
}

pub(in crate::runtime) fn rgbal() -> FixtureProfile {
    additive(
        "TL-592 RGBAL",
        &[
            ("color.amber", Some(amber())),
            ("color.lime", Some(xyz(0.40, 0.80, 0.10))),
        ],
    )
}

pub(in crate::runtime) fn wheel_slots() -> Vec<(u32, u32, Vec<SpectrumSample>)> {
    vec![
        (0, 15, spectrum(|_| 1.)),
        (16, 31, pass(590, 830)),
        (32, 47, pass(360, 499)),
    ]
}

/// Fixed source through one wheel (open, red, blue) with an unmodeled rotation range.
pub(in crate::runtime) fn wheel_only() -> FixtureProfile {
    Builder::new("TL-592 wheel only")
        .fixed_source()
        .filter("color.wheel.1", 127, wheel_slots(), true)
        .build()
}

/// Fixed source through two independent wheels.
pub(in crate::runtime) fn dual_wheel() -> FixtureProfile {
    Builder::new("TL-592 dual wheel")
        .fixed_source()
        .filter("color.wheel.1", 127, wheel_slots(), false)
        .filter(
            "color.wheel.2",
            127,
            vec![
                (0, 15, spectrum(|_| 1.)),
                (16, 31, pass(480, 600)),
                (32, 47, pass(400, 520)),
            ],
            false,
        )
        .build()
}

/// Fixed source through CMY flags (clear / half / full) and a color wheel.
pub(in crate::runtime) fn cmy_wheel() -> FixtureProfile {
    let mut builder = Builder::new("TL-592 CMY wheel").fixed_source();
    for (attribute, blocked) in [
        ("color.cyan", 590..=830),
        ("color.magenta", 490..=589),
        ("color.yellow", 360..=489),
    ] {
        let (half, full) = (blocked.clone(), blocked);
        builder = builder.filter(
            attribute,
            255,
            vec![
                (0, 0, spectrum(|_| 1.)),
                (
                    1,
                    127,
                    spectrum(move |nm| if half.contains(&nm) { 0.5 } else { 1. }),
                ),
                (
                    128,
                    255,
                    spectrum(move |nm| if full.contains(&nm) { 0.02 } else { 1. }),
                ),
            ],
            false,
        );
    }
    builder
        .filter("color.wheel.1", 127, wheel_slots(), false)
        .build()
}

/// A patched root fixture on universe 1 at `address`.
pub(in crate::runtime) fn patched(
    profile: &FixtureProfile,
    id: FixtureId,
    address: u16,
) -> PatchedFixture {
    PatchedFixture {
        model_scale: None,
        scenery_options: Default::default(),
        scenery_size_metres: None,
        fixture_id: id,
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: profile.name.clone(),
        definition: profile.resolved_definition(profile.modes[0].id).unwrap(),
        universe: Some(1),
        address: Some(address),
        split_patches: vec![],
        layer_id: "default".into(),
        note: None,
        position_master: None,
        direct_control: None,
        internal_bindings: Default::default(),
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        multipatch: vec![],
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        position_calibration: None,
        color_calibration: None,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: Default::default(),
        freeze: Default::default(),
    }
}

/// TL-557 hybrid: additive RGBW emitters behind a color wheel (open, red, blue).
pub(in crate::runtime) fn hybrid() -> FixtureProfile {
    let [r, g, b] = rgb_columns();
    Builder::new("TL-557 hybrid RGBW wheel")
        .emitter("color.red", Some(r))
        .emitter("color.green", Some(g))
        .emitter("color.blue", Some(b))
        .emitter("color.white", Some(white()))
        .filter("color.wheel.1", 127, wheel_slots(), false)
        .build()
}
