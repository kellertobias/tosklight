use light_application as application;
use light_core::{AttributeKey, AttributeValue, FixtureId, Xyz};
use light_wire::v2::{events::EventSnapshotCursor, preload_values as wire};

pub(super) fn command(
    action: wire::ProgrammingPreloadValuesAction,
) -> application::ProgrammingPreloadValuesCommand {
    match action {
        wire::ProgrammingPreloadValuesAction::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingPreloadValuesCommand::SetFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingPreloadValuesAction::ReleaseFixture {
            fixture_id,
            attribute,
        } => application::ProgrammingPreloadValuesCommand::ReleaseFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute),
        },
        wire::ProgrammingPreloadValuesAction::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingPreloadValuesCommand::SetGroup {
            group_id,
            attribute: AttributeKey(attribute),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingPreloadValuesAction::ReleaseGroup {
            group_id,
            attribute,
        } => application::ProgrammingPreloadValuesCommand::ReleaseGroup {
            group_id,
            attribute: AttributeKey(attribute),
        },
        wire::ProgrammingPreloadValuesAction::Batch { mutations } => {
            application::ProgrammingPreloadValuesCommand::Batch {
                mutations: mutations.into_iter().map(application_mutation).collect(),
            }
        }
    }
}

pub(super) fn outcome(
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
        user_id: projection.user_id.0,
        revision: projection.revision,
        fixture_values: projection
            .fixture_values
            .iter()
            .map(fixture_value)
            .collect(),
        group_values: projection.group_values.iter().map(group_value).collect(),
    }
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
            attribute: AttributeKey(attribute),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingPreloadValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => application::ProgrammingPreloadValueMutation::ReleaseFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute),
        },
        wire::ProgrammingPreloadValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingPreloadValueMutation::SetGroup {
            group_id,
            attribute: AttributeKey(attribute),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingPreloadValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => application::ProgrammingPreloadValueMutation::ReleaseGroup {
            group_id,
            attribute: AttributeKey(attribute),
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

fn application_value(value: wire::ProgrammingPreloadAttributeValue) -> AttributeValue {
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
        attribute: value.attribute.0.clone(),
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
        attribute: value.attribute.0.clone(),
        value: attribute_value(&value.value),
        programmer_order: value.programmer_order,
        fade: value.fade,
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

fn attribute_value(value: &AttributeValue) -> wire::ProgrammingPreloadAttributeValue {
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
        AttributeValue::RawDmx(value) => wire::ProgrammingPreloadAttributeValue::RawDmx(*value),
        AttributeValue::RawDmxExact(value) => {
            wire::ProgrammingPreloadAttributeValue::RawDmxExact(*value)
        }
    }
}
