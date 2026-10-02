//! Pages reflect the compiled descriptors and the selection's profiles; nothing is guessed.
use super::*;
use light_core::AttributeKey;
use light_fixture::*;
use light_wire::v2::family_encoders::{
    ColorEncoderPresentation, FamilyEncoderComponentSlot, FamilyEncoderEditKind,
    FamilyEncoderFamily, FamilyEncoderGroup, FamilyEncoderLimitsSource, FamilyEncoderPagesSnapshot,
    FamilyEncoderReservation, FamilyEncoderSlot,
};
use light_wire::v2::programming_intent::{
    ProgrammingAttributeBounds, ProgrammingComponentUnit, ProgrammingOpeningConvention,
    ProgrammingScalarDomain,
};
use uuid::Uuid;

/// One channel: `(physical_min, physical_max, unit)` on its single continuous function.
pub(in crate::runtime) struct TestChannel {
    pub attribute: &'static str,
    pub range: Option<(f32, f32, &'static str)>,
    pub convention: Option<OpeningConvention>,
}

pub(in crate::runtime) fn test_channel(
    attribute: &'static str,
    range: Option<(f32, f32, &'static str)>,
    convention: Option<OpeningConvention>,
) -> TestChannel {
    TestChannel {
        attribute,
        range,
        convention,
    }
}

/// A patched single-head fixture whose selected mode carries `channels`.
pub(in crate::runtime) fn test_fixture(channels: &[TestChannel]) -> PatchedFixture {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Family encoder pages".into();
    let head = profile.modes[0].heads[0].id;
    for spec in channels {
        let attribute = AttributeKey(spec.attribute.into());
        let mut function = ChannelFunction::continuous(spec.attribute, attribute.clone(), 255);
        if let Some((min, max, unit)) = spec.range {
            function.behavior = ChannelFunctionBehavior::Continuous {
                physical_min: min,
                physical_max: max,
                unit: Some(unit.into()),
            };
        }
        function.physical_mapping = spec
            .convention
            .map(|convention| PhysicalMappingCalibration {
                opening_convention: Some(convention),
                ..Default::default()
            });
        profile.modes[0].channels.push(FixtureChannel {
            id: Uuid::new_v4(),
            head_id: head,
            split: 1,
            fixture_attribute: attribute.clone(),
            attribute,
            canonical_transform: CanonicalTransform::Identity,
            resolution: ChannelResolution::U8,
            secondary_slots: vec![],
            default_raw: 0,
            highlight_raw: 255,
            physical_min: None,
            physical_max: None,
            unit: None,
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            virtual_intensity_inverted: false,
            reacts_to_sequence_master: true,
            reacts_to_group_master: true,
            reacts_to_grand_master: false,
            behavior: ChannelBehavior::Controlled,
            functions: vec![function],
        });
    }
    profile.modes[0].splits[0].footprint = channels.len().max(1) as u16;
    blank_patched(profile.resolved_definition(profile.modes[0].id).unwrap())
}

fn blank_patched(definition: FixtureDefinition) -> PatchedFixture {
    PatchedFixture {
        model_scale: None,
        scenery_options: Default::default(),
        scenery_size_metres: None,
        fixture_id: FixtureId::new(),
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: "Family encoder pages".into(),
        definition,
        universe: Some(1),
        address: Some(1),
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

/// Pan/Tilt mover with Zoom 8..48° (field), Focus and RGB.
pub(in crate::runtime) fn spot(zoom: (f32, f32)) -> PatchedFixture {
    test_fixture(&[
        test_channel("pan", Some((-270., 270., "deg")), None),
        test_channel("tilt", Some((-135., 135., "deg")), None),
        test_channel(
            "zoom",
            Some((zoom.0, zoom.1, "deg")),
            Some(OpeningConvention::Field),
        ),
        test_channel("focus", None, None),
        test_channel("color.red", None, None),
    ])
}

fn pages(
    fixtures: &[PatchedFixture],
    contract: u16,
    presentation: ColorEncoderPresentation,
) -> FamilyEncoderPagesSnapshot {
    let requested: Vec<FixtureId> = fixtures.iter().map(|f| f.fixture_id).collect();
    family_encoder_pages(&FamilyEncoderInputs {
        fixtures,
        requested: &requested,
        supported_contract: contract,
        presentation,
        show_revision: 7,
    })
}

fn group(
    snapshot: &FamilyEncoderPagesSnapshot,
    family: FamilyEncoderFamily,
) -> &FamilyEncoderGroup {
    snapshot
        .families
        .iter()
        .find(|g| g.family == family)
        .unwrap()
}

fn component<'a>(group: &'a FamilyEncoderGroup, id: &str) -> &'a FamilyEncoderComponentSlot {
    group
        .pages
        .iter()
        .flat_map(|page| page.slots.iter().flatten())
        .find_map(|slot| match slot {
            FamilyEncoderSlot::Component(slot) if slot.id == id => Some(slot),
            _ => None,
        })
        .unwrap_or_else(|| panic!("slot {id}"))
}

fn slot_ids(group: &FamilyEncoderGroup, page: usize) -> Vec<Option<String>> {
    group.pages[page]
        .slots
        .iter()
        .map(|slot| {
            slot.as_ref().map(|slot| match slot {
                FamilyEncoderSlot::Component(slot) => slot.id.clone(),
                FamilyEncoderSlot::Attribute { attribute, .. } => attribute.clone(),
            })
        })
        .collect()
}

fn ids(values: &[&str]) -> Vec<Option<String>> {
    values
        .iter()
        .map(|value| (!value.is_empty()).then(|| (*value).to_owned()))
        .collect()
}

#[test]
fn the_semantic_flag_mirrors_the_supported_programming_contract() {
    let fixtures = [spot((8., 48.))];
    let legacy = pages(&fixtures, 0, ColorEncoderPresentation::EasyRgbw);
    assert!(!legacy.semantic);
    assert_eq!(legacy.supported_programming_contract, 0);
    assert_eq!(legacy.semantic_programming_contract, 1);
    assert_eq!(legacy.show_revision, 7);
    assert!(pages(&fixtures, 1, ColorEncoderPresentation::EasyRgbw).semantic);
}

#[test]
fn position_and_focus_pages_follow_the_owner_layout_and_compiled_descriptors() {
    let fixtures = [spot((48., 8.))];
    let snapshot = pages(&fixtures, 1, ColorEncoderPresentation::EasyRgbw);
    let position = group(&snapshot, FamilyEncoderFamily::Position);
    assert_eq!(
        slot_ids(position, 0),
        ids(&["position.pan", "position.tilt", "", ""])
    );
    assert_eq!(
        slot_ids(position, 1),
        ids(&[
            "position.target",
            "position.target.x",
            "position.target.y",
            "position.target.z"
        ])
    );
    let pan = component(position, "position.pan");
    assert_eq!(pan.descriptor.unit, ProgrammingComponentUnit::Degrees);
    assert_eq!((pan.descriptor.step, pan.descriptor.fine_step), (1., 0.1));
    assert_eq!(pan.descriptor.domain, Some(ProgrammingScalarDomain::Finite));
    assert_eq!(
        (pan.limits, pan.limits_source),
        (None, FamilyEncoderLimitsSource::Unknown)
    );
    let x = component(position, "position.target.x");
    assert_eq!(x.descriptor.unit, ProgrammingComponentUnit::Metres);
    assert_eq!(x.limits_source, FamilyEncoderLimitsSource::Unbounded);
    let point = component(position, "position.target");
    assert_eq!(point.edit, FamilyEncoderEditKind::TargetReference);
    assert_eq!(point.descriptor.unit, ProgrammingComponentUnit::Selection);
    assert_eq!(position.fixture_ids, vec![fixtures[0].fixture_id.0]);

    let focus = group(&snapshot, FamilyEncoderFamily::Focus);
    assert_eq!(slot_ids(focus, 0), ids(&["focus", "zoom", "softness", ""]));
    let focus_slot = component(focus, "focus");
    assert_eq!(
        focus_slot.descriptor.unit,
        ProgrammingComponentUnit::Percent
    );
    assert_eq!(focus_slot.descriptor.display_scale, 100.);
    assert_eq!(
        (focus_slot.limits, focus_slot.limits_source),
        (
            Some(ProgrammingAttributeBounds { min: 0., max: 1. }),
            FamilyEncoderLimitsSource::Descriptor
        )
    );
    let zoom = component(focus, "zoom");
    assert_eq!(zoom.descriptor.unit, ProgrammingComponentUnit::Degrees);
    assert_eq!(
        (zoom.limits, zoom.limits_source),
        (
            Some(ProgrammingAttributeBounds { min: 8., max: 48. }),
            FamilyEncoderLimitsSource::Selection
        ),
        "a reversed profile range is published ascending"
    );
    assert_eq!(zoom.convention, Some(ProgrammingOpeningConvention::Field));
    assert_eq!(focus.replaces_attributes, ["focus", "zoom", "softness"]);
}

#[test]
fn disagreeing_or_undeclared_zoom_ranges_publish_no_common_limit() {
    let mixed = [spot((8., 48.)), spot((10., 40.))];
    let snapshot = pages(&mixed, 1, ColorEncoderPresentation::EasyRgbw);
    let zoom = component(group(&snapshot, FamilyEncoderFamily::Focus), "zoom");
    assert_eq!(
        (zoom.limits, zoom.limits_source),
        (None, FamilyEncoderLimitsSource::Mixed)
    );
    assert_eq!(zoom.convention, Some(ProgrammingOpeningConvention::Field));

    let undeclared = [
        spot((8., 48.)),
        test_fixture(&[test_channel("zoom", None, None)]),
    ];
    let snapshot = pages(&undeclared, 1, ColorEncoderPresentation::EasyRgbw);
    let zoom = component(group(&snapshot, FamilyEncoderFamily::Focus), "zoom");
    assert_eq!(
        (zoom.limits, zoom.limits_source),
        (None, FamilyEncoderLimitsSource::Unknown)
    );
    assert_eq!(zoom.convention, None, "no common convention is claimed");
    assert_eq!(zoom.fixture_ids.len(), 2);
}

#[test]
fn color_pages_follow_the_desk_presentation_and_reserve_direct_pages() {
    let fixtures = [spot((8., 48.))];
    let easy = pages(&fixtures, 1, ColorEncoderPresentation::EasyRgbw);
    let color = group(&easy, FamilyEncoderFamily::Color);
    assert_eq!(color.pages.len(), 1);
    assert_eq!(
        slot_ids(color, 0),
        ids(&[
            "color.red",
            "color.green",
            "color.blue",
            "color.white_blend"
        ])
    );
    assert_eq!(
        color
            .reserved_pages
            .iter()
            .map(|p| (p.number, p.reason))
            .collect::<Vec<_>>(),
        [
            (3, FamilyEncoderReservation::NativeColor),
            (4, FamilyEncoderReservation::NativeColor)
        ]
    );
    assert_eq!(color.replaces_attribute_prefixes, ["color."]);
    let white = component(color, "color.white_blend");
    assert_eq!(white.descriptor.unit, ProgrammingComponentUnit::Percent);

    let extended = pages(&fixtures, 1, ColorEncoderPresentation::EasyRgbwauv);
    let color = group(&extended, FamilyEncoderFamily::Color);
    assert_eq!(
        slot_ids(color, 1),
        ids(&["color.amber", "color.uv", "", ""])
    );

    let advanced = pages(&fixtures, 1, ColorEncoderPresentation::Advanced);
    let color = group(&advanced, FamilyEncoderFamily::Color);
    assert_eq!(
        slot_ids(color, 1),
        ids(&[
            "color.temperature",
            "color.duv",
            "color.wheel.1",
            "color.wheel.2"
        ])
    );
    let temperature = component(color, "color.temperature");
    assert_eq!(
        temperature.descriptor.unit,
        ProgrammingComponentUnit::Kelvin
    );
    assert_eq!(
        temperature.limits_source,
        FamilyEncoderLimitsSource::Descriptor
    );
    assert_eq!(
        component(color, "color.wheel.1").edit,
        FamilyEncoderEditKind::Unavailable,
        "wheels have no typed component edit yet"
    );
    assert_eq!(
        advanced.color_presentation,
        ColorEncoderPresentation::Advanced
    );
}

#[test]
fn unknown_and_unsupporting_fixtures_are_kept_in_the_request_but_not_targeted() {
    let fixtures = [test_fixture(&[test_channel("dimmer", None, None)])];
    let mut requested = vec![fixtures[0].fixture_id, FixtureId::new()];
    requested.push(fixtures[0].fixture_id);
    let snapshot = family_encoder_pages(&FamilyEncoderInputs {
        fixtures: &fixtures,
        requested: &requested,
        supported_contract: 1,
        presentation: ColorEncoderPresentation::EasyRgbw,
        show_revision: 1,
    });
    assert_eq!(
        snapshot.fixture_ids.len(),
        3,
        "request order and duplicates are echoed"
    );
    for family in &snapshot.families {
        assert!(family.fixture_ids.is_empty(), "{:?}", family.family);
    }
}
