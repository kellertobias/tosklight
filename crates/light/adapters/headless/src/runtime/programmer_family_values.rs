//! Typed family values from the command line, keypad and OSC (TL-544 G3).
//!
//! `<selection> AT COLOR 100 DIV 0 DIV 0` (keypad `[AT][^2] 100 [DIV] 0 [DIV] 0`) writes the
//! family's first encoder page in encoder order, `[THRU]` between complete tuples spreads each
//! component over the ordered selection, and a leading `+`/`-` steps a component relatively. The
//! request becomes the same semantic component-edit transaction an encoder sends: one
//! `ApplyIntent` per owner through the Programmer values service, so Normal/Blind and Preload,
//! live Groups, adoption and holds behave exactly as on the encoders. Nothing is converted to a
//! normalized channel value here.
use super::*;
use light_core::programming::{
    ComponentEdit, ProgrammingComponent, ProgrammingOwner, ScalarDomain, ScalarEdit, ScalarIntent,
};
use light_wire::v2::family_encoders as wire;

/// A family addressed by its second-layer key after `AT`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CommandFamily {
    Color,
    Position,
    Focus,
}

/// One encoder slot of a family's first page, in encoder order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FamilySlot {
    /// The slot id the family encoder pages publish.
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) component: ProgrammingComponent,
}

const fn slot(
    id: &'static str,
    label: &'static str,
    component: ProgrammingComponent,
) -> FamilySlot {
    FamilySlot {
        id,
        label,
        component,
    }
}

const COLOR_SLOTS: [FamilySlot; 4] = {
    use light_core::programming::ColorComponent as C;
    [
        slot("color.red", "Red", ProgrammingComponent::Color(C::Red)),
        slot(
            "color.green",
            "Green",
            ProgrammingComponent::Color(C::Green),
        ),
        slot("color.blue", "Blue", ProgrammingComponent::Color(C::Blue)),
        slot(
            "color.white_blend",
            "White Blend",
            ProgrammingComponent::Color(C::WhiteBlend),
        ),
    ]
};
const POSITION_SLOTS: [FamilySlot; 2] = [
    slot("position.pan", "Pan", ProgrammingComponent::Pan),
    slot("position.tilt", "Tilt", ProgrammingComponent::Tilt),
];
const FOCUS_SLOTS: [FamilySlot; 2] = [
    slot("focus", "Focus", ProgrammingComponent::Focus),
    slot("zoom", "Zoom", ProgrammingComponent::Zoom),
];

impl CommandFamily {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "COLOR" | "COLOUR" => Some(Self::Color),
            "POSITION" => Some(Self::Position),
            "FOCUS" => Some(Self::Focus),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Color => "COLOR",
            Self::Position => "POSITION",
            Self::Focus => "FOCUS",
        }
    }

    pub(super) fn slots(self) -> &'static [FamilySlot] {
        match self {
            Self::Color => &COLOR_SLOTS,
            Self::Position => &POSITION_SLOTS,
            Self::Focus => &FOCUS_SLOTS,
        }
    }
}

/// The family a value names, when it is a typed family value rather than a Preset recall
/// (`COLOR PRESET 22`) or an intensity.
pub(super) fn family_value_keyword(value: &[String]) -> Option<CommandFamily> {
    let family = CommandFamily::parse(value.first()?)?;
    (value.get(1).map(String::as_str) != Some("PRESET")).then_some(family)
}

/// One component entry of one `[THRU]` point, in display units (percent, degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Entry {
    Empty,
    Absolute(f32),
    Relative(f32),
}

/// A parsed typed family value: per component, the edit in descriptor units.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct FamilyValues {
    pub(super) family: CommandFamily,
    pub(super) edits: Vec<(FamilySlot, ScalarEdit)>,
    /// Control points of the widest spread, for the selection-size check.
    pub(super) spread_points: usize,
}

fn number(tokens: &[String], label: &str) -> Result<f32, String> {
    let text = tokens.concat();
    let value = if text == "FULL" {
        100.0
    } else {
        text.parse::<f32>()
            .map_err(|_| format!("{label} requires a number"))?
    };
    if !value.is_finite() {
        return Err(format!("{label} requires a finite number"));
    }
    Ok(value)
}

/// `[]`, `v`, `+ v`, `- v`, or `- - v` (a negative absolute value, such as Pan −90°).
fn entry(tokens: &[String], label: &str) -> Result<Entry, String> {
    match tokens {
        [] => Ok(Entry::Empty),
        [minus, again, rest @ ..] if minus == "-" && again == "-" => {
            Ok(Entry::Absolute(-number(rest, label)?))
        }
        [sign, rest @ ..] if sign == "+" => Ok(Entry::Relative(number(rest, label)?)),
        [sign, rest @ ..] if sign == "-" => Ok(Entry::Relative(-number(rest, label)?)),
        _ => Ok(Entry::Absolute(number(tokens, label)?)),
    }
}

/// A display value converted to descriptor units and checked against a bounded domain.
fn descriptor_value(slot: &FamilySlot, display: f32) -> Result<f32, String> {
    let descriptor = slot.component.descriptor();
    let scale = if descriptor.display_scale == 0.0 {
        1.0
    } else {
        descriptor.display_scale
    };
    let value = display / scale;
    if let Some(ScalarDomain::Bounded { bounds }) = descriptor.domain
        && !(bounds.min..=bounds.max).contains(&value)
    {
        return Err(format!(
            "{} must be within {}-{}",
            slot.label,
            bounds.min * scale,
            bounds.max * scale
        ));
    }
    Ok(value)
}

/// Parses the tokens after the family keyword.
pub(super) fn parse_family_values(
    family: CommandFamily,
    tokens: &[String],
) -> Result<FamilyValues, String> {
    let slots = family.slots();
    // A second `[DIV]` displays as OFFSET on the keypad; after a family key it is two separators.
    let tokens = tokens
        .iter()
        .flat_map(|token| match token.as_str() {
            "OFFSET" => vec!["DIV".to_owned(), "DIV".to_owned()],
            _ => vec![token.clone()],
        })
        .collect::<Vec<_>>();
    let points = tokens
        .split(|token| token == "THRU")
        .map(|point| {
            let entries = point.split(|token| token == "DIV").collect::<Vec<_>>();
            if entries.len() > slots.len() {
                return Err(format!(
                    "{} takes at most {} values separated by DIV",
                    family.name(),
                    slots.len()
                ));
            }
            entries
                .iter()
                .zip(slots)
                .map(|(tokens, slot)| entry(tokens, slot.label))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut edits = Vec::new();
    let spread = points.len() > 1;
    for (index, slot) in slots.iter().enumerate() {
        let entries = points
            .iter()
            .map(|point| point.get(index).copied().unwrap_or(Entry::Empty))
            .collect::<Vec<_>>();
        if entries.iter().all(|entry| *entry == Entry::Empty) {
            continue;
        }
        let edit = if spread {
            let values = entries
                .iter()
                .map(|entry| match entry {
                    Entry::Absolute(value) => descriptor_value(slot, *value),
                    Entry::Relative(_) => Err(format!(
                        "a {} spread takes absolute values, not + or -",
                        slot.label
                    )),
                    Entry::Empty => Err(format!(
                        "a {} spread needs a value at every THRU point",
                        slot.label
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?;
            ScalarEdit::Set(ScalarIntent::Spread(values))
        } else {
            match entries[0] {
                Entry::Absolute(value) => {
                    ScalarEdit::Set(ScalarIntent::Value(descriptor_value(slot, value)?))
                }
                Entry::Relative(delta) => {
                    let scale = slot.component.descriptor().display_scale;
                    ScalarEdit::Relative(delta / if scale == 0.0 { 1.0 } else { scale })
                }
                Entry::Empty => unreachable!("empty components were skipped"),
            }
        };
        edits.push((*slot, edit));
    }
    if edits.is_empty() {
        return Err(format!("{} requires a value", family.name()));
    }
    Ok(FamilyValues {
        family,
        edits,
        spread_points: if spread { points.len() } else { 1 },
    })
}

/// Which Programmer the edit writes: Preload while it captures, otherwise Normal (incl. Blind).
fn preload_lane(state: &AppState, session: &Session) -> bool {
    state
        .programming
        .get(session.id)
        .is_some_and(|current| current.blind && current.preload_capture_programmer)
}

/// The encoder slots of the current selection, exactly as the family encoder pages publish them.
fn published_slots(
    state: &AppState,
    selected: &[light_core::FixtureId],
) -> Vec<wire::FamilyEncoderComponentSlot> {
    let engine = state.output.engine();
    let snapshot = engine.snapshot();
    let pages = super::family_encoder_pages::family_encoder_pages(
        &super::family_encoder_pages::FamilyEncoderInputs {
            fixtures: &snapshot.fixtures,
            requested: selected,
            supported_contract: engine.supported_programming_contract(),
            presentation: state.installation.configuration().color_presentation,
            show_revision: snapshot.revision,
        },
    );
    pages
        .families
        .into_iter()
        .flat_map(|family| family.pages)
        .flat_map(|page| page.slots)
        .filter_map(|slot| match slot {
            Some(wire::FamilyEncoderSlot::Component(slot)) => Some(slot),
            _ => None,
        })
        .collect()
}

/// One owner's planned edit: its components and the fixtures that carry it.
struct OwnerEdit {
    owner: ProgrammingOwner,
    edits: Vec<ComponentEdit>,
    fixtures: Vec<light_core::FixtureId>,
}

fn owner_edits(
    state: &AppState,
    values: &FamilyValues,
    selected: &[light_core::FixtureId],
) -> Vec<OwnerEdit> {
    let published = published_slots(state, selected);
    let mut owners: Vec<OwnerEdit> = Vec::new();
    for (slot, edit) in &values.edits {
        let Some(published) = published.iter().find(|candidate| candidate.id == slot.id) else {
            continue;
        };
        // Parity with the encoders: a slot they do not edit (no published Zoom convention, a
        // non-scalar slot) is not applicable, and nothing is sent for it.
        if published.edit != wire::FamilyEncoderEditKind::Scalar
            || (slot.component == ProgrammingComponent::Zoom && published.convention.is_none())
            || published.fixture_ids.is_empty()
        {
            continue;
        }
        let owner = slot.component.owner();
        let edit = ComponentEdit::Scalar {
            component: slot.component,
            operation: edit.clone(),
        };
        if let Some(existing) = owners.iter_mut().find(|planned| planned.owner == owner) {
            existing.edits.push(edit);
        } else {
            owners.push(OwnerEdit {
                owner,
                edits: vec![edit],
                fixtures: published
                    .fixture_ids
                    .iter()
                    .copied()
                    .map(light_core::FixtureId)
                    .collect(),
            });
        }
    }
    owners
}

/// A typed family value entered on the command line for `fixtures`, or `None` when `value` is
/// not one. An inapplicable value (no fixture carries an addressed component) changes nothing,
/// not even the selection; otherwise `expression` becomes the selection (when given) and the
/// value is applied to it.
pub(super) fn typed_family_command(
    state: &AppState,
    session: &Session,
    value: &[String],
    fixtures: &[light_core::FixtureId],
    expression: Option<&light_programmer::SelectionExpression>,
    timing: CommandTiming,
) -> Option<Result<usize, String>> {
    let family = family_value_keyword(value)?;
    Some(parse_family_values(family, &value[1..]).and_then(|values| {
        if owner_edits(state, &values, fixtures).is_empty() {
            return Ok(0);
        }
        if let Some(expression) = expression {
            state
                .programming
                .select_expression(session.id, fixtures.to_vec(), expression.clone());
        }
        apply_family_values_to_selection(
            state,
            session,
            &values,
            timing,
            FamilyValueSource::CommandLine,
        )
    }))
}

/// Where a typed family value comes from, which decides persistence and attribution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FamilyValueSource {
    /// Inside the command line's staged transaction; the command flow persists.
    CommandLine,
    /// A direct OSC write against the live desk; the values service persists.
    Osc,
}

/// Applies typed family values to the current ordered selection through the Programmer values
/// service. Returns the number of selected fixtures an edit addressed (0: nothing applicable).
pub(super) fn apply_family_values_to_selection(
    state: &AppState,
    session: &Session,
    values: &FamilyValues,
    timing: CommandTiming,
    source: FamilyValueSource,
) -> Result<usize, String> {
    let current = state
        .programming
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let groups = current
        .selection_expression
        .as_ref()
        .map(light_programmer::SelectionExpression::live_group_owners)
        .unwrap_or_default();
    let owners = owner_edits(state, values, &current.selected);
    if owners.is_empty() {
        return Ok(0);
    }
    for owner in &owners {
        if values.spread_points > 1 {
            ensure_spread_fits(&vec![0.0; values.spread_points], owner.fixtures.len())?;
        }
    }
    let preload = preload_lane(state, session);
    let (action_source, port_source) = match source {
        FamilyValueSource::CommandLine => (
            light_application::ActionSource::Keyboard,
            "programmer_command",
        ),
        FamilyValueSource::Osc => (light_application::ActionSource::Osc, "osc_family_values"),
    };
    let ports = command_http::ServerProgrammingPorts::new(state, session, port_source, true);
    // Inside the command line's staged transaction the command flow persists the result.
    let ports = match source {
        FamilyValueSource::CommandLine => ports.without_persistence(),
        FamilyValueSource::Osc => ports,
    };
    let timing = light_application::ProgrammingValueTiming {
        fade: timing.fade,
        fade_millis: timing.fade_millis,
        delay_millis: timing.delay_millis,
    };
    for owner in &owners {
        let lanes: Vec<(Option<String>, Vec<light_core::FixtureId>)> = if groups.is_empty() {
            vec![(None, owner.fixtures.clone())]
        } else {
            groups
                .iter()
                .map(|group| (Some(group.clone()), Vec::new()))
                .collect()
        };
        for (group_id, fixture_ids) in lanes {
            let intent = light_application::ProgrammingValueIntent {
                fixture_ids,
                group_id,
                attribute: owner.owner.key(),
                operation: light_application::ProgrammingValueOperation::ComponentEdits(
                    owner.edits.clone(),
                ),
                undo_group: None,
                timing,
                displayed_source: None,
                color_adoption: light_application::ProgrammingColorAdoptionRequest::default(),
            };
            submit_intent(state, session, &ports, action_source, preload, intent)?;
        }
    }
    Ok(current.selected.len())
}

fn submit_intent(
    state: &AppState,
    session: &Session,
    ports: &dyn light_application::ProgrammingPorts,
    source: light_application::ActionSource,
    preload: bool,
    intent: light_application::ProgrammingValueIntent,
) -> Result<(), String> {
    let registry = state.programming.programmers();
    let context =
        operator_action_context(session, source).with_request_id(Uuid::new_v4().to_string());
    let capture_mode_revision = registry.capture_mode_revision();
    if preload {
        let context = context.with_expected_revision(registry.preload_values_revision());
        state
            .programming
            .handle_preload_values(
                light_application::ActionEnvelope {
                    context,
                    command: light_application::ProgrammingPreloadValuesRequest {
                        expected_capture_mode_revision: capture_mode_revision,
                        command: light_application::ProgrammingPreloadValuesCommand::ApplyIntent {
                            intent,
                        },
                    },
                },
                ports,
            )
            .map(|_| ())
            .map_err(|error| error.message)
    } else {
        let context = context.with_expected_revision(registry.normal_values_revision());
        state
            .programming
            .handle_values(
                light_application::ActionEnvelope {
                    context,
                    command: light_application::ProgrammingValuesRequest {
                        expected_capture_mode_revision: capture_mode_revision,
                        command: light_application::ProgrammingValuesCommand::ApplyIntent {
                            intent,
                        },
                    },
                },
                ports,
            )
            .map(|_| ())
            .map_err(|error| error.message)
    }
}

#[cfg(test)]
#[path = "programmer_family_values_tests.rs"]
mod tests;
