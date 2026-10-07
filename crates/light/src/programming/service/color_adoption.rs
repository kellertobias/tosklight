//! TL-554: Color adoption checks of the first sample of a Color component gesture.
//!
//! Runs once per gesture, right after the transport captured its family contexts, under the
//! Programmer/desk mutation boundary and before anything is planned:
//! - A Direct (`native`) edit needs, for every target, the pinned source model plus either a
//!   seed that is already Direct of that source (edited in place, never reseeded) or the Direct
//!   value captured once from the reference head's published premaster output. Anything else
//!   holds the whole action quietly (`NativeColorUnavailable`): no partial edit, no Undo step.
//! - The first semantic edit of a Direct value adopts its modelled appearance and UV once
//!   (`semantic_color_adoption`): the original model's forward evaluation when the transport
//!   supplies it, else the recorded estimate. An unknown visible appearance is never invented:
//!   without the operator's explicit starting colour the action holds with
//!   `ExplicitColorStartRequired`. Successful adoptions are reported with the action.
use crate::{
    ActionContext, ActionError, ActionErrorKind, ProgrammingColorAdoption,
    ProgrammingColorAdoptionFixture, ProgrammingColorAdoptionStart, ProgrammingPorts,
    ProgrammingValueIntent, ProgrammingValueOperation, ProgrammingValuesEnvironment,
    ProgrammingValuesHold,
};
use light_core::programming::{
    ColorProgram, ComponentEdit, NativeColorRecipe, ProgrammingOwner, semantic_color_adoption,
};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use std::collections::HashMap;

type FixtureValues = HashMap<(FixtureId, AttributeKey), AttributeValue>;
type GroupValues = HashMap<(String, AttributeKey), AttributeValue>;

pub(super) fn prepare_color_adoption(
    ports: &dyn ProgrammingPorts,
    context: &ActionContext,
    intent: &ProgrammingValueIntent,
    members: &[FixtureId],
    environment: &mut ProgrammingValuesEnvironment,
    active: &FixtureValues,
    groups: &GroupValues,
) -> Result<(), ActionError> {
    let ProgrammingValueOperation::ComponentEdits(edits) = &intent.operation else {
        return Ok(());
    };
    if environment.displayed_source_hold.is_some()
        || edits
            .first()
            .is_none_or(|edit| edit.owner() != ProgrammingOwner::Color)
    {
        return Ok(());
    }
    let hold = if edits
        .iter()
        .any(|edit| matches!(edit, ComponentEdit::Native { .. }))
    {
        (!native_seeds_available(intent, members, environment, active, groups))
            .then_some(ProgrammingValuesHold::NativeColorUnavailable)
    } else {
        adopt_semantic(ports, context, intent, members, environment, active, groups)?
    };
    environment.displayed_source_hold = hold;
    Ok(())
}

/// The value a target edits, in the planner's order: a value stored on the edited Group
/// itself (its member's exception, else the Group value) wins over the fixture's own value.
fn seed<'a>(
    intent: &ProgrammingValueIntent,
    fixture: FixtureId,
    environment: &'a ProgrammingValuesEnvironment,
    active: &'a FixtureValues,
    groups: &'a GroupValues,
) -> Option<&'a AttributeValue> {
    if let Some(stored) = group_value(intent, groups) {
        return Some(match stored {
            AttributeValue::GroupFamily(assignment) => assignment.for_member(fixture),
            value => value,
        });
    }
    let address = (fixture, intent.attribute.clone());
    active
        .get(&address)
        .or_else(|| environment.current_values.get(&address))
        .or_else(|| environment.default_values.get(&address))
}

fn group_value<'a>(
    intent: &ProgrammingValueIntent,
    groups: &'a GroupValues,
) -> Option<&'a AttributeValue> {
    let group = intent.group_id.as_ref()?;
    groups.get(&(group.clone(), intent.attribute.clone()))
}

/// The Group's own template (the value future members receive), when it is stored.
fn group_template<'a>(
    intent: &ProgrammingValueIntent,
    groups: &'a GroupValues,
) -> Option<&'a AttributeValue> {
    group_value(intent, groups).map(|stored| match stored {
        AttributeValue::GroupFamily(assignment) => &assignment.template,
        value => value,
    })
}

fn direct(value: Option<&AttributeValue>) -> Option<(&NativeColorRecipe, &ColorProgram)> {
    match value? {
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Direct { recipe, .. } => Some((recipe, program.as_ref())),
            ColorProgram::Semantic { .. } => None,
        },
        _ => None,
    }
}

/// Every target has its pinned model and a seed that is Direct of that source or the
/// captured reference seed. A Group target also needs its shared context.
fn native_seeds_available(
    intent: &ProgrammingValueIntent,
    members: &[FixtureId],
    environment: &ProgrammingValuesEnvironment,
    active: &FixtureValues,
    groups: &GroupValues,
) -> bool {
    let group_ready = intent.group_id.as_ref().is_none_or(|group| {
        environment
            .group_family_contexts
            .get(group)
            .is_some_and(|context| {
                context.native_model.is_some() && context.direct_color_seed.is_some()
            })
    });
    group_ready
        && members.iter().all(|fixture| {
            let Some(context) = environment.family_contexts.get(fixture) else {
                return false;
            };
            let Some(model) = context.native_model.as_deref() else {
                return false;
            };
            context.direct_color_seed.is_some()
                || direct(seed(intent, *fixture, environment, active, groups))
                    .is_some_and(|(recipe, _)| &recipe.source == model.source())
        })
}

/// All-or-nothing: every Direct target adopts, or the whole action holds. A Group stored as
/// Direct adopts its template too, so its future members start from the same appearance.
fn adopt_semantic(
    ports: &dyn ProgrammingPorts,
    context: &ActionContext,
    intent: &ProgrammingValueIntent,
    members: &[FixtureId],
    environment: &mut ProgrammingValuesEnvironment,
    active: &FixtureValues,
    groups: &GroupValues,
) -> Result<Option<ProgrammingValuesHold>, ActionError> {
    let mut report = ProgrammingColorAdoption::default();
    let mut adopted = Vec::new();
    for fixture in members {
        let Some(seed) = direct(seed(intent, *fixture, environment, active, groups)) else {
            continue;
        };
        let Some(adoption) = adopt_one(ports, context, intent, seed)? else {
            return Ok(Some(ProgrammingValuesHold::ExplicitColorStartRequired));
        };
        report.fixtures.push(ProgrammingColorAdoptionFixture {
            fixture_id: *fixture,
            start: if adoption.visible_from_start {
                ProgrammingColorAdoptionStart::Explicit
            } else {
                ProgrammingColorAdoptionStart::Approximate
            },
            uv_unknown: adoption.uv_unknown,
        });
        for limitation in &adoption.limitations {
            if !report.limitations.contains(limitation) {
                report.limitations.push(limitation.clone());
            }
        }
        adopted.push((*fixture, adoption.intent));
    }
    let template = match direct(group_template(intent, groups)) {
        Some(seed) => match adopt_one(ports, context, intent, seed)? {
            Some(adoption) => Some(adoption.intent),
            None => return Ok(Some(ProgrammingValuesHold::ExplicitColorStartRequired)),
        },
        None => None,
    };
    let group_context = intent
        .group_id
        .as_ref()
        .and_then(|group| environment.group_family_contexts.get(group))
        .cloned();
    for (fixture, intent) in adopted {
        // A member without its own context edits through the Group's shared context.
        environment
            .family_contexts
            .entry(fixture)
            .or_insert_with(|| group_context.clone().unwrap_or_default())
            .semantic_color_adoption = Some(intent);
    }
    if let (Some(group), Some(template)) = (intent.group_id.as_ref(), template) {
        environment
            .group_family_contexts
            .entry(group.clone())
            .or_default()
            .semantic_color_adoption = Some(template);
    }
    if !report.fixtures.is_empty() {
        environment.color_adoption = Some(report);
    }
    Ok(None)
}

/// One Direct value's semantic adoption; `None` when its appearance is unknown and the
/// operator gave no explicit start.
fn adopt_one(
    ports: &dyn ProgrammingPorts,
    context: &ActionContext,
    intent: &ProgrammingValueIntent,
    (recipe, program): (&NativeColorRecipe, &ColorProgram),
) -> Result<Option<light_core::programming::SemanticColorAdoption>, ActionError> {
    let ColorProgram::Direct { portable, .. } = program else {
        unreachable!("direct() returns Direct programs only")
    };
    // The original model's forward evaluation of the reference recipe (spreads removed,
    // as authoring predicts); the recorded estimate when the original is unavailable.
    let estimate = match ports.native_color_model(context, &recipe.source) {
        Some(model) => model
            .predict(&NativeColorRecipe {
                spreads: Vec::new(),
                ..recipe.clone()
            })
            .map_err(invalid)?,
        None => portable.clone(),
    };
    if estimate.visible.is_none() && intent.color_adoption.explicit_start.is_none() {
        return Ok(None);
    }
    semantic_color_adoption(&estimate, intent.color_adoption.explicit_start.as_ref())
        .map(Some)
        .map_err(invalid)
}

/// The adoption a values action reports: only with the sample that actually adopted (a held
/// sample changed nothing and reports nothing).
pub(super) fn reported(
    environment: Option<ProgrammingValuesEnvironment>,
    hold: Option<ProgrammingValuesHold>,
) -> Option<ProgrammingColorAdoption> {
    environment
        .and_then(|environment| environment.color_adoption)
        .filter(|_| hold.is_none())
}

fn invalid(error: light_core::programming::IntentError) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}
