//! Semantic family encoder pages for one fixture selection (TL-549/550/551 UI foundation).
//!
//! Every slot carries the compiled runtime descriptor of its component
//! (`ProgrammingComponent::descriptor`) unchanged, plus what only the selection can say: the
//! common physical limits of the selected profiles and, for Zoom, their common opening
//! convention. Nothing is guessed: disagreeing profiles report `mixed` without limits and a
//! profile without a declared range reports `unknown`.
//!
//! Layout decisions (owner, 2026-10-02): Position page 1 Pan/Tilt and page 2 Point/X/Y/Z; Color
//! pages 1/2 by the per-desk Easy/Advanced presentation with Direct pages 3/4 reserved for TL-554;
//! Focus page 1 in registry order Focus, Zoom, Softness. Return Home stays where it is.

use super::command_http::ToIntentWire;
use light_core::programming::{
    AuthoringCapability, ColorComponent, ComponentDescriptor, ComponentRole, ComponentUnit,
    ProgrammingComponent, ProgrammingOwner, ScalarDomain, ScalarInterpolation,
};
use light_core::{FixtureId, OpeningConvention};
use light_fixture::{
    ChannelFunctionBehavior, FixtureChannel, FixtureDefinition, Parameter, PatchedFixture,
    PhysicalUnit,
};
use light_wire::v2::family_encoders as wire;
use light_wire::v2::programming_intent as intent;

/// What the page builder reads. The route fills it from one engine snapshot and the desk
/// configuration; tests fill it directly.
pub(super) struct FamilyEncoderInputs<'a> {
    pub fixtures: &'a [PatchedFixture],
    pub requested: &'a [FixtureId],
    pub supported_contract: u16,
    pub presentation: wire::ColorEncoderPresentation,
    pub show_revision: u64,
}

pub(super) fn family_encoder_pages(
    inputs: &FamilyEncoderInputs<'_>,
) -> wire::FamilyEncoderPagesSnapshot {
    let selection: Vec<SelectedFixture<'_>> = inputs
        .requested
        .iter()
        .filter_map(|id| SelectedFixture::resolve(inputs.fixtures, *id))
        .collect();
    let semantic_contract = light_core::programming::PROGRAMMING_CONTRACT_VERSION;
    wire::FamilyEncoderPagesSnapshot {
        semantic: inputs.supported_contract >= semantic_contract,
        supported_programming_contract: inputs.supported_contract,
        semantic_programming_contract: semantic_contract,
        color_presentation: inputs.presentation,
        show_revision: inputs.show_revision,
        fixture_ids: inputs.requested.iter().map(|id| id.0).collect(),
        families: vec![
            position_group(&selection),
            color_group(&selection, inputs.presentation),
            focus_group(&selection),
        ],
    }
}

/// One requested id resolved to its definition: a whole fixture or one logical head.
struct SelectedFixture<'a> {
    id: FixtureId,
    definition: &'a FixtureDefinition,
    head: Option<u16>,
}

impl<'a> SelectedFixture<'a> {
    fn resolve(fixtures: &'a [PatchedFixture], id: FixtureId) -> Option<Self> {
        fixtures.iter().find_map(|fixture| {
            if fixture.fixture_id == id {
                return Some(Self {
                    id,
                    definition: &fixture.definition,
                    head: None,
                });
            }
            fixture
                .logical_heads
                .iter()
                .find(|head| head.fixture_id == id)
                .map(|head| Self {
                    id,
                    definition: &fixture.definition,
                    head: Some(head.head_index),
                })
        })
    }

    fn parameters(&self) -> impl Iterator<Item = &'a Parameter> + '_ {
        self.definition
            .heads
            .iter()
            .filter(move |head| self.head.is_none_or(|index| head.index == index))
            .flat_map(|head| head.parameters.iter())
    }

    fn has(&self, attribute: &str) -> bool {
        self.parameters()
            .any(|parameter| &*parameter.attribute.0 == attribute)
    }

    fn has_owner(&self, owner: ProgrammingOwner) -> bool {
        match owner {
            ProgrammingOwner::Position => self.has("pan") || self.has("tilt"),
            ProgrammingOwner::Focus => self.has("focus"),
            ProgrammingOwner::Zoom => self.has("zoom"),
            ProgrammingOwner::Color => self.parameters().any(|parameter| {
                let attribute = &*parameter.attribute.0;
                attribute == "color" || attribute.starts_with("color.")
            }),
        }
    }

    /// The selected mode's channels for `attribute` (one head only for a logical head).
    fn mode_channels(&self, attribute: &str) -> Vec<&'a FixtureChannel> {
        let Some(profile) = self.definition.profile_snapshot.as_deref() else {
            return Vec::new();
        };
        let Some(mode) = self.definition.mode_id.and_then(|id| profile.mode(id)) else {
            return Vec::new();
        };
        let head = self
            .head
            .and_then(|index| mode.heads.get(usize::from(index)))
            .map(|head| head.id);
        mode.channels
            .iter()
            .filter(|channel| &*channel.attribute.0 == attribute)
            .filter(|channel| head.is_none_or(|id| channel.head_id == id))
            .collect()
    }

    /// The profile-declared physical range of `attribute` in degrees, ascending. The selected
    /// mode's continuous function is authoritative; the projected parameter metadata is the
    /// fallback for definitions without a profile snapshot.
    fn degree_range(&self, attribute: &str) -> Option<(f32, f32)> {
        let ascending = |a: f32, b: f32| {
            (a.is_finite() && b.is_finite() && a != b).then(|| (a.min(b), a.max(b)))
        };
        let degrees = |unit: Option<&str>| PhysicalUnit::parse(unit) == PhysicalUnit::Degrees;
        let channels = self.mode_channels(attribute);
        if !channels.is_empty() {
            return channels.iter().find_map(|channel| {
                channel
                    .functions
                    .iter()
                    .find_map(|function| match &function.behavior {
                        ChannelFunctionBehavior::Continuous {
                            physical_min,
                            physical_max,
                            unit,
                        } if degrees(unit.as_deref()) => ascending(*physical_min, *physical_max),
                        _ => None,
                    })
            });
        }
        let parameter = self.parameters().find(|p| &*p.attribute.0 == attribute)?;
        let metadata = &parameter.metadata;
        degrees(metadata.unit.as_deref())
            .then(|| ascending(metadata.physical_min, metadata.physical_max))
            .flatten()
    }

    /// The opening convention the profile's calibration declares for its Zoom function.
    fn zoom_convention(&self) -> Option<OpeningConvention> {
        self.mode_channels("zoom")
            .into_iter()
            .flat_map(|channel| channel.functions.iter())
            .find_map(|function| function.physical_mapping.as_ref()?.opening_convention)
    }
}

fn owner_fixtures(selection: &[SelectedFixture<'_>], owner: ProgrammingOwner) -> Vec<uuid::Uuid> {
    selection
        .iter()
        .filter(|f| f.has_owner(owner))
        .map(|f| f.id.0)
        .collect()
}

/// The limits every selected owner fixture agrees on, or why there are none.
fn common<T: PartialEq + Copy>(values: impl Iterator<Item = Option<T>>) -> Agreement<T> {
    let mut agreed = None;
    let mut any = false;
    for value in values {
        any = true;
        let Some(value) = value else {
            return Agreement::Unknown;
        };
        match agreed {
            None => agreed = Some(value),
            Some(existing) if existing != value => return Agreement::Mixed,
            Some(_) => {}
        }
    }
    match (any, agreed) {
        (true, Some(value)) => Agreement::Common(value),
        _ => Agreement::Unknown,
    }
}

enum Agreement<T> {
    Common(T),
    Mixed,
    Unknown,
}

struct SlotSpec {
    id: &'static str,
    label: &'static str,
    component: ProgrammingComponent,
}

const fn spec(id: &'static str, label: &'static str, component: ProgrammingComponent) -> SlotSpec {
    SlotSpec {
        id,
        label,
        component,
    }
}

fn component_slot(
    selection: &[SelectedFixture<'_>],
    spec: &SlotSpec,
) -> Option<wire::FamilyEncoderSlot> {
    let descriptor = spec.component.descriptor();
    let owner = descriptor.owner;
    let fixtures: Vec<&SelectedFixture<'_>> =
        selection.iter().filter(|f| f.has_owner(owner)).collect();
    let (limits, limits_source) = slot_limits(spec.component, &descriptor, &fixtures);
    let convention = match spec.component {
        ProgrammingComponent::Zoom => match common(fixtures.iter().map(|f| f.zoom_convention())) {
            Agreement::Common(convention) => Some(wire_convention(convention)),
            _ => None,
        },
        _ => None,
    };
    let edit = match spec.component {
        ProgrammingComponent::TargetReference => wire::FamilyEncoderEditKind::TargetReference,
        ProgrammingComponent::ColorWheel(_) | ProgrammingComponent::NativeColor(_) => {
            wire::FamilyEncoderEditKind::Unavailable
        }
        _ => wire::FamilyEncoderEditKind::Scalar,
    };
    Some(wire::FamilyEncoderSlot::Component(
        wire::FamilyEncoderComponentSlot {
            id: spec.id.into(),
            label: spec.label.into(),
            component: spec.component.to_intent_wire(),
            descriptor: wire_descriptor(&descriptor),
            limits,
            limits_source,
            convention,
            edit,
            fixture_ids: fixtures.iter().map(|f| f.id.0).collect(),
        },
    ))
}

fn slot_limits(
    component: ProgrammingComponent,
    descriptor: &ComponentDescriptor,
    fixtures: &[&SelectedFixture<'_>],
) -> (
    Option<intent::ProgrammingAttributeBounds>,
    wire::FamilyEncoderLimitsSource,
) {
    use wire::FamilyEncoderLimitsSource as Source;
    let profile_attribute = match component {
        ProgrammingComponent::Zoom => Some("zoom"),
        _ => None,
    };
    if let Some(attribute) = profile_attribute {
        return match common(fixtures.iter().map(|f| f.degree_range(attribute))) {
            Agreement::Common((min, max)) => (
                Some(intent::ProgrammingAttributeBounds { min, max }),
                Source::Selection,
            ),
            Agreement::Mixed => (None, Source::Mixed),
            Agreement::Unknown => (None, Source::Unknown),
        };
    }
    match (component, descriptor.domain) {
        // Production Pan/Tilt programming limits have no agreed source yet (TL-549 §6).
        (ProgrammingComponent::Pan | ProgrammingComponent::Tilt, _) => (None, Source::Unknown),
        (_, Some(ScalarDomain::Bounded { bounds } | ScalarDomain::Cyclic { bounds })) => (
            Some(intent::ProgrammingAttributeBounds {
                min: bounds.min,
                max: bounds.max,
            }),
            Source::Descriptor,
        ),
        (_, Some(ScalarDomain::Finite)) => (None, Source::Unbounded),
        (_, None) => (None, Source::Unknown),
    }
}

fn page(
    number: u8,
    label: &str,
    slots: Vec<Option<wire::FamilyEncoderSlot>>,
) -> wire::FamilyEncoderPage {
    let mut slots = slots;
    slots.resize(4, None);
    wire::FamilyEncoderPage {
        number,
        label: label.into(),
        slots,
    }
}

fn position_group(selection: &[SelectedFixture<'_>]) -> wire::FamilyEncoderGroup {
    use ProgrammingComponent as P;
    let slot = |s: SlotSpec| component_slot(selection, &s);
    wire::FamilyEncoderGroup {
        family: wire::FamilyEncoderFamily::Position,
        owners: vec![intent::ProgrammingOwner::Position],
        fixture_ids: owner_fixtures(selection, ProgrammingOwner::Position),
        replaces_attributes: vec!["pan".into(), "tilt".into()],
        replaces_attribute_prefixes: vec![],
        pages: vec![
            page(
                1,
                "Angles",
                vec![
                    slot(spec("position.pan", "Pan", P::Pan)),
                    slot(spec("position.tilt", "Tilt", P::Tilt)),
                ],
            ),
            page(
                2,
                "Target",
                vec![
                    slot(spec("position.target", "Point", P::TargetReference)),
                    slot(spec("position.target.x", "X", P::TargetX)),
                    slot(spec("position.target.y", "Y", P::TargetY)),
                    slot(spec("position.target.z", "Z", P::TargetZ)),
                ],
            ),
        ],
        reserved_pages: vec![],
    }
}

fn color_group(
    selection: &[SelectedFixture<'_>],
    presentation: wire::ColorEncoderPresentation,
) -> wire::FamilyEncoderGroup {
    use ColorComponent as C;
    use ProgrammingComponent as P;
    use wire::ColorEncoderPresentation as Layout;
    let slot = |s: SlotSpec| component_slot(selection, &s);
    let mut pages = vec![page(
        1,
        "Color",
        vec![
            slot(spec("color.red", "Red", P::Color(C::Red))),
            slot(spec("color.green", "Green", P::Color(C::Green))),
            slot(spec("color.blue", "Blue", P::Color(C::Blue))),
            slot(spec(
                "color.white_blend",
                "White Blend",
                P::Color(C::WhiteBlend),
            )),
        ],
    )];
    match presentation {
        Layout::EasyRgbw => {}
        Layout::EasyRgbwauv => pages.push(page(
            2,
            "Amber · UV",
            vec![
                slot(spec("color.amber", "Amber", P::Color(C::Amber))),
                slot(spec("color.uv", "UV", P::Color(C::Uv))),
            ],
        )),
        Layout::Advanced => pages.push(page(
            2,
            "White balance · Wheels",
            vec![
                slot(spec(
                    "color.temperature",
                    "Temperature",
                    P::Color(C::Temperature),
                )),
                slot(spec("color.duv", "Duv", P::Color(C::Duv))),
                slot(spec("color.wheel.1", "Wheel 1", P::ColorWheel(0))),
                slot(spec("color.wheel.2", "Wheel 2", P::ColorWheel(1))),
            ],
        )),
    }
    wire::FamilyEncoderGroup {
        family: wire::FamilyEncoderFamily::Color,
        owners: vec![intent::ProgrammingOwner::Color],
        fixture_ids: owner_fixtures(selection, ProgrammingOwner::Color),
        replaces_attributes: vec!["color".into()],
        replaces_attribute_prefixes: vec!["color.".into()],
        pages,
        reserved_pages: [3, 4]
            .map(|number| wire::FamilyEncoderReservedPage {
                number,
                reason: wire::FamilyEncoderReservation::NativeColor,
            })
            .to_vec(),
    }
}

fn focus_group(selection: &[SelectedFixture<'_>]) -> wire::FamilyEncoderGroup {
    use ProgrammingComponent as P;
    let mut fixture_ids = owner_fixtures(selection, ProgrammingOwner::Focus);
    for id in owner_fixtures(selection, ProgrammingOwner::Zoom) {
        if !fixture_ids.contains(&id) {
            fixture_ids.push(id);
        }
    }
    fixture_ids.sort_by_key(|id| selection.iter().position(|f| f.id.0 == *id));
    wire::FamilyEncoderGroup {
        family: wire::FamilyEncoderFamily::Focus,
        owners: vec![
            intent::ProgrammingOwner::Focus,
            intent::ProgrammingOwner::Zoom,
        ],
        fixture_ids,
        replaces_attributes: vec!["focus".into(), "zoom".into(), "softness".into()],
        replaces_attribute_prefixes: vec![],
        pages: vec![page(
            1,
            "Focus · Zoom",
            vec![
                component_slot(selection, &spec("focus", "Focus", P::Focus)),
                component_slot(selection, &spec("zoom", "Zoom", P::Zoom)),
                Some(wire::FamilyEncoderSlot::Attribute {
                    attribute: "softness".into(),
                    label: "Softness".into(),
                }),
            ],
        )],
        reserved_pages: vec![],
    }
}

fn wire_convention(convention: OpeningConvention) -> intent::ProgrammingOpeningConvention {
    match convention {
        OpeningConvention::Beam => intent::ProgrammingOpeningConvention::Beam,
        OpeningConvention::Field => intent::ProgrammingOpeningConvention::Field,
    }
}

/// The compiled descriptor, field for field. Domain conversion belongs to the adapter.
pub(super) fn wire_descriptor(d: &ComponentDescriptor) -> intent::ProgrammingComponentDescriptor {
    let bounds = |b: light_core::AttributeBounds| intent::ProgrammingAttributeBounds {
        min: b.min,
        max: b.max,
    };
    intent::ProgrammingComponentDescriptor {
        owner: d.owner.to_intent_wire(),
        role: match d.role {
            ComponentRole::ColorRecipe => intent::ProgrammingComponentRole::ColorRecipe,
            ComponentRole::ColorCoordinate => intent::ProgrammingComponentRole::ColorCoordinate,
            ComponentRole::ColorOrthogonal => intent::ProgrammingComponentRole::ColorOrthogonal,
            ComponentRole::ColorWheel => intent::ProgrammingComponentRole::ColorWheel,
            ComponentRole::NativeColor => intent::ProgrammingComponentRole::NativeColor,
            ComponentRole::Angle => intent::ProgrammingComponentRole::Angle,
            ComponentRole::Target => intent::ProgrammingComponentRole::Target,
            ComponentRole::Focus => intent::ProgrammingComponentRole::Focus,
            ComponentRole::Zoom => intent::ProgrammingComponentRole::Zoom,
        },
        unit: match d.unit {
            ComponentUnit::Percent => intent::ProgrammingComponentUnit::Percent,
            ComponentUnit::Degrees => intent::ProgrammingComponentUnit::Degrees,
            ComponentUnit::Metres => intent::ProgrammingComponentUnit::Metres,
            ComponentUnit::Kelvin => intent::ProgrammingComponentUnit::Kelvin,
            ComponentUnit::Duv => intent::ProgrammingComponentUnit::Duv,
            ComponentUnit::Factor => intent::ProgrammingComponentUnit::Factor,
            ComponentUnit::NativeInteger => intent::ProgrammingComponentUnit::NativeInteger,
            ComponentUnit::Selection => intent::ProgrammingComponentUnit::Selection,
        },
        domain: d.domain.map(|domain| match domain {
            ScalarDomain::Finite => intent::ProgrammingScalarDomain::Finite,
            ScalarDomain::Bounded { bounds: b } => {
                intent::ProgrammingScalarDomain::Bounded { bounds: bounds(b) }
            }
            ScalarDomain::Cyclic { bounds: b } => {
                intent::ProgrammingScalarDomain::Cyclic { bounds: bounds(b) }
            }
        }),
        step: d.step,
        fine_step: d.fine_step,
        display_scale: d.display_scale,
        interpolation: match d.interpolation {
            ScalarInterpolation::Linear => intent::ProgrammingScalarInterpolation::Linear,
            ScalarInterpolation::ShortestArc => intent::ProgrammingScalarInterpolation::ShortestArc,
            ScalarInterpolation::Reciprocal => intent::ProgrammingScalarInterpolation::Reciprocal,
        },
        capability: match d.capability {
            AuthoringCapability::SemanticIntent => {
                intent::ProgrammingAuthoringCapability::SemanticIntent
            }
            AuthoringCapability::VerifiedNativeControl => {
                intent::ProgrammingAuthoringCapability::VerifiedNativeControl
            }
            AuthoringCapability::FocusParameter => {
                intent::ProgrammingAuthoringCapability::FocusParameter
            }
        },
        spread: d.spread,
        align: d.align,
        dynamics: d.dynamics,
    }
}

#[cfg(test)]
#[path = "family_encoder_pages/tests.rs"]
pub(in crate::runtime) mod tests;
