use super::intent_wire::{IntoIntentDomain, ToIntentWire};
use light_application as application;
use light_core::{AttributeKey, AttributeValue, FixtureId, Xyz};
use light_wire::v2::{events::EventSnapshotCursor, preload_values as wire};

pub(crate) fn command(
    action: wire::ProgrammingPreloadValuesAction,
) -> application::ProgrammingPreloadValuesCommand {
    match action {
        wire::ProgrammingPreloadValuesAction::FinishGesture {
            attribute,
            undo_group,
        } => application::ProgrammingPreloadValuesCommand::FinishGesture {
            attribute: AttributeKey(attribute.into()),
            undo_group,
        },
        wire::ProgrammingPreloadValuesAction::ApplyIntent {
            fixture_ids,
            group_id,
            attribute,
            operation,
            undo_group,
            timing,
            displayed_source,
            native_reference,
            explicit_color_start,
        } => application::ProgrammingPreloadValuesCommand::ApplyIntent {
            intent: application::ProgrammingValueIntent {
                fixture_ids: fixture_ids.into_iter().map(FixtureId).collect(),
                group_id,
                attribute: AttributeKey(attribute.into()),
                operation: match operation {
                    wire::ProgrammingPreloadValueOperation::AbsoluteSet { value } => {
                        application::ProgrammingValueOperation::AbsoluteSet(application_value(
                            value,
                        ))
                    }
                    wire::ProgrammingPreloadValueOperation::ComponentEdits { edits } => {
                        application::ProgrammingValueOperation::ComponentEdits(
                            edits.into_intent_domain(),
                        )
                    }
                    wire::ProgrammingPreloadValueOperation::RelativeStep { delta } => {
                        application::ProgrammingValueOperation::RelativeStep(delta)
                    }
                },
                undo_group,
                timing: application::ProgrammingValueTiming {
                    fade: timing.fade,
                    fade_millis: timing.fade_millis,
                    delay_millis: timing.delay_millis,
                },
                displayed_source: displayed_source.map(super::displayed_source),
                color_adoption: super::color_adoption_request(
                    native_reference,
                    explicit_color_start,
                ),
            },
        },
        wire::ProgrammingPreloadValuesAction::ApplyIndexedPreset { .. } => {
            unreachable!("Indexed Preset actions are resolved against the active patch first")
        }
        wire::ProgrammingPreloadValuesAction::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingPreloadValuesCommand::SetFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute.into()),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingPreloadValuesAction::ReleaseFixture {
            fixture_id,
            attribute,
        } => application::ProgrammingPreloadValuesCommand::ReleaseFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute.into()),
        },
        wire::ProgrammingPreloadValuesAction::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingPreloadValuesCommand::SetGroup {
            group_id,
            attribute: AttributeKey(attribute.into()),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingPreloadValuesAction::ReleaseGroup {
            group_id,
            attribute,
        } => application::ProgrammingPreloadValuesCommand::ReleaseGroup {
            group_id,
            attribute: AttributeKey(attribute.into()),
        },
        wire::ProgrammingPreloadValuesAction::Batch { mutations } => {
            application::ProgrammingPreloadValuesCommand::Batch {
                mutations: mutations.into_iter().map(application_mutation).collect(),
            }
        }
    }
}

pub(crate) fn outcome(
    request_id: String,
    result: application::ProgrammingPreloadValuesResult,
) -> wire::ProgrammingPreloadValuesActionOutcome {
    let revision = result.outcome.revision();
    let outcome = match result.outcome {
        application::ProgrammingPreloadValuesOutcome::Changed {
            projection,
            event_sequence,
        } => wire::ProgrammingPreloadValuesActionState::Changed {
            projection: projection_from_application(&projection),
            event_sequence,
        },
        application::ProgrammingPreloadValuesOutcome::NoChange { .. } => {
            wire::ProgrammingPreloadValuesActionState::NoChange
        }
    };
    wire::ProgrammingPreloadValuesActionOutcome {
        request_id,
        correlation_id: result.context.correlation_id,
        revision,
        capture_mode_revision: result.capture_mode_revision,
        outcome,
        replayed: result.replayed,
        warning: result.warning,
        hold: result.hold.map(super::hold_reason),
        color_adoption: result.color_adoption.map(super::color_adoption_report),
    }
}

pub(super) fn snapshot(
    snapshot: application::ProgrammingPreloadValuesSnapshot,
) -> wire::ProgrammingPreloadValuesSnapshot {
    wire::ProgrammingPreloadValuesSnapshot {
        cursor: EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        projection: projection_from_application(&snapshot.projection),
    }
}

pub(in crate::runtime) fn change(
    change: &application::ProgrammingPreloadValuesChange,
) -> wire::ProgrammingPreloadValuesChange {
    wire::ProgrammingPreloadValuesChange {
        projection: projection_from_application(&change.projection),
    }
}

pub(super) fn projection_from_application(
    projection: &application::ProgrammingPreloadValuesProjection,
) -> wire::ProgrammingPreloadValuesProjection {
    wire::ProgrammingPreloadValuesProjection {
        revision: projection.revision,
        group_release_values: projection
            .group_release_values
            .iter()
            .map(|entry| wire::ProgrammingPreloadGroupReleaseValue {
                group_id: entry.group_id.clone(),
                attribute: entry.attribute.0.to_string(),
                programmer_order: entry.programmer_order,
                changed_at_millis: entry.changed_at_millis,
            })
            .collect(),
        fixture_values: projection
            .fixture_values
            .iter()
            .map(fixture_value)
            .collect(),
        group_values: projection.group_values.iter().map(group_value).collect(),
        dynamic_values: projection
            .dynamic_values
            .iter()
            .map(preload_dynamic_value)
            .collect(),
    }
}

/// Preload has no shared-definition table: each retained On row carries its exact fallback.
fn preload_dynamic_value(
    value: &light_dynamics::DynamicAddressValue,
) -> light_wire::v2::programming::ProgrammingDynamicValue {
    let mut projected = super::dynamics_wire::programming_value(value);
    if let (
        light_dynamics::DynamicSemanticValue::DynamicOn {
            dynamic: retained, ..
        },
        light_wire::v2::programming::ProgrammingDynamicSemanticValue::DynamicOn { dynamic, .. },
    ) = (&value.value, &mut projected.value)
    {
        dynamic.embedded_fallback = Some(super::dynamics_wire::definition(
            retained.embedded_fallback.definition.as_ref(),
        ));
    }
    projected
}

fn application_mutation(
    mutation: wire::ProgrammingPreloadValueMutation,
) -> application::ProgrammingPreloadValueMutation {
    match mutation {
        wire::ProgrammingPreloadValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingPreloadValueMutation::SetFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute.into()),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingPreloadValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => application::ProgrammingPreloadValueMutation::ReleaseFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute.into()),
        },
        wire::ProgrammingPreloadValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingPreloadValueMutation::SetGroup {
            group_id,
            attribute: AttributeKey(attribute.into()),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingPreloadValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => application::ProgrammingPreloadValueMutation::ReleaseGroup {
            group_id,
            attribute: AttributeKey(attribute.into()),
        },
    }
}

const fn application_timing(
    timing: wire::ProgrammingPreloadValueTiming,
) -> application::ProgrammingPreloadValueTiming {
    application::ProgrammingPreloadValueTiming {
        fade: timing.fade,
        fade_millis: timing.fade_millis,
        delay_millis: timing.delay_millis,
    }
}

pub(super) fn application_value(value: wire::ProgrammingPreloadAttributeValue) -> AttributeValue {
    match value {
        wire::ProgrammingPreloadAttributeValue::Normalized(value) => {
            AttributeValue::Normalized(value)
        }
        wire::ProgrammingPreloadAttributeValue::Spread(values) => AttributeValue::Spread(values),
        wire::ProgrammingPreloadAttributeValue::Discrete(value) => AttributeValue::Discrete(value),
        wire::ProgrammingPreloadAttributeValue::ColorXyz(value) => AttributeValue::ColorXyz(Xyz {
            x: value.x,
            y: value.y,
            z: value.z,
        }),
        wire::ProgrammingPreloadAttributeValue::ColorProgram(value) => {
            AttributeValue::ColorProgram(std::sync::Arc::new(value.into_intent_domain()))
        }
        wire::ProgrammingPreloadAttributeValue::Position(value) => {
            AttributeValue::Position(std::sync::Arc::new(value.into_intent_domain()))
        }
        wire::ProgrammingPreloadAttributeValue::Zoom(value) => {
            AttributeValue::Zoom(std::sync::Arc::new(value.into_intent_domain()))
        }
        wire::ProgrammingPreloadAttributeValue::GroupFamily(value) => {
            AttributeValue::GroupFamily(std::sync::Arc::new((*value).into_intent_domain()))
        }
        wire::ProgrammingPreloadAttributeValue::RawDmx(value) => AttributeValue::RawDmx(value),
        wire::ProgrammingPreloadAttributeValue::RawDmxExact(value) => {
            AttributeValue::RawDmxExact(value)
        }
    }
}

fn fixture_value(
    value: &light_programmer::PreloadProgrammerFixtureValue,
) -> wire::ProgrammingPreloadFixtureValue {
    wire::ProgrammingPreloadFixtureValue {
        fixture_id: value.fixture_id.0,
        attribute: value.attribute.0.to_string(),
        value: attribute_value(&value.value),
        programmer_order: value.programmer_order,
        fade: value.fade,
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

fn group_value(
    value: &light_programmer::PreloadProgrammerGroupValue,
) -> wire::ProgrammingPreloadGroupValue {
    wire::ProgrammingPreloadGroupValue {
        group_id: value.group_id.clone(),
        attribute: value.attribute.0.to_string(),
        value: attribute_value(&value.value),
        programmer_order: value.programmer_order,
        fade: value.fade,
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

pub(crate) fn attribute_value(value: &AttributeValue) -> wire::ProgrammingPreloadAttributeValue {
    match value {
        AttributeValue::Normalized(value) => {
            wire::ProgrammingPreloadAttributeValue::Normalized(*value)
        }
        AttributeValue::Spread(values) => {
            wire::ProgrammingPreloadAttributeValue::Spread(values.clone())
        }
        AttributeValue::Discrete(value) => {
            wire::ProgrammingPreloadAttributeValue::Discrete(value.clone())
        }
        AttributeValue::ColorXyz(value) => {
            wire::ProgrammingPreloadAttributeValue::ColorXyz(wire::ProgrammingPreloadColorXyz {
                x: value.x,
                y: value.y,
                z: value.z,
            })
        }
        AttributeValue::ColorProgram(value) => {
            wire::ProgrammingPreloadAttributeValue::ColorProgram(value.as_ref().to_intent_wire())
        }
        AttributeValue::Position(value) => {
            wire::ProgrammingPreloadAttributeValue::Position(value.as_ref().to_intent_wire())
        }
        AttributeValue::Zoom(value) => {
            wire::ProgrammingPreloadAttributeValue::Zoom(value.as_ref().to_intent_wire())
        }
        AttributeValue::GroupFamily(value) => wire::ProgrammingPreloadAttributeValue::GroupFamily(
            Box::new(value.as_ref().to_intent_wire()),
        ),
        AttributeValue::RawDmx(value) => wire::ProgrammingPreloadAttributeValue::RawDmx(*value),
        AttributeValue::RawDmxExact(value) => {
            wire::ProgrammingPreloadAttributeValue::RawDmxExact(*value)
        }
    }
}

#[cfg(test)]
#[path = "preload_values_wire_tests.rs"]
mod tests;
