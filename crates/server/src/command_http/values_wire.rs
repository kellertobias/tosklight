use light_application as application;
use light_core::{AttributeKey, AttributeValue, FixtureId, Xyz};
use light_wire::v2::{events::EventSnapshotCursor, programming as wire};

pub(super) fn values_command(
    action: wire::ProgrammingValuesAction,
) -> application::ProgrammingValuesCommand {
    match action {
        wire::ProgrammingValuesAction::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingValuesCommand::SetFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingValuesAction::ReleaseFixture {
            fixture_id,
            attribute,
        } => application::ProgrammingValuesCommand::ReleaseFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute),
        },
        wire::ProgrammingValuesAction::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingValuesCommand::SetGroup {
            group_id,
            attribute: AttributeKey(attribute),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingValuesAction::ReleaseGroup {
            group_id,
            attribute,
        } => application::ProgrammingValuesCommand::ReleaseGroup {
            group_id,
            attribute: AttributeKey(attribute),
        },
        wire::ProgrammingValuesAction::Batch { mutations } => {
            application::ProgrammingValuesCommand::Batch {
                mutations: mutations.into_iter().map(application_mutation).collect(),
            }
        }
        wire::ProgrammingValuesAction::Clear => application::ProgrammingValuesCommand::Clear,
    }
}

pub(super) fn values_outcome(
    request_id: String,
    result: application::ProgrammingValuesResult,
) -> wire::ProgrammingValuesActionOutcome {
    let revision = result.outcome.revision();
    let outcome = match result.outcome {
        application::ProgrammingValuesOutcome::Changed {
            projection,
            event_sequence,
        } => wire::ProgrammingValuesActionState::Changed {
            projection: values_projection(&projection),
            event_sequence,
        },
        application::ProgrammingValuesOutcome::NoChange { .. } => {
            wire::ProgrammingValuesActionState::NoChange
        }
    };
    wire::ProgrammingValuesActionOutcome {
        request_id,
        correlation_id: result.context.correlation_id,
        revision,
        capture_mode_revision: result.capture_mode_revision,
        outcome,
        replayed: result.replayed,
        warning: result.warning,
    }
}

pub(super) fn capture_mode_snapshot(
    snapshot: application::ProgrammingCaptureModeSnapshot,
) -> wire::ProgrammingCaptureModeSnapshot {
    wire::ProgrammingCaptureModeSnapshot {
        cursor: EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        projection: capture_mode_projection(&snapshot.projection),
    }
}

pub(in crate::runtime) fn capture_mode_change(
    change: &application::ProgrammingCaptureModeChange,
) -> wire::ProgrammingCaptureModeChange {
    wire::ProgrammingCaptureModeChange {
        projection: capture_mode_projection(&change.projection),
    }
}

fn capture_mode_projection(
    projection: &application::ProgrammingCaptureModeProjection,
) -> wire::ProgrammingCaptureModeProjection {
    wire::ProgrammingCaptureModeProjection {
        user_id: projection.user_id.0,
        revision: projection.revision,
        blind: projection.blind,
        preview: projection.preview,
        preload_capture_programmer: projection.preload_capture_programmer,
    }
}

pub(super) fn values_snapshot(
    snapshot: application::ProgrammingValuesSnapshot,
) -> wire::ProgrammingValuesSnapshot {
    wire::ProgrammingValuesSnapshot {
        cursor: EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        projection: values_projection(&snapshot.projection),
    }
}

pub(in crate::runtime) fn values_change(
    change: &application::ProgrammingValuesChange,
) -> wire::ProgrammingValuesChange {
    wire::ProgrammingValuesChange {
        projection: values_projection(&change.projection),
    }
}

pub(super) fn values_projection(
    projection: &application::ProgrammingValuesProjection,
) -> wire::ProgrammingValuesProjection {
    wire::ProgrammingValuesProjection {
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
    mutation: wire::ProgrammingValueMutation,
) -> application::ProgrammingValueMutation {
    match mutation {
        wire::ProgrammingValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingValueMutation::SetFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => application::ProgrammingValueMutation::ReleaseFixture {
            fixture_id: FixtureId(fixture_id),
            attribute: AttributeKey(attribute),
        },
        wire::ProgrammingValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => application::ProgrammingValueMutation::SetGroup {
            group_id,
            attribute: AttributeKey(attribute),
            value: application_value(value),
            timing: application_timing(timing),
        },
        wire::ProgrammingValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => application::ProgrammingValueMutation::ReleaseGroup {
            group_id,
            attribute: AttributeKey(attribute),
        },
    }
}

const fn application_timing(
    timing: wire::ProgrammingValueTiming,
) -> application::ProgrammingValueTiming {
    application::ProgrammingValueTiming {
        fade: timing.fade,
        fade_millis: timing.fade_millis,
        delay_millis: timing.delay_millis,
    }
}

fn application_value(value: wire::ProgrammingAttributeValue) -> AttributeValue {
    match value {
        wire::ProgrammingAttributeValue::Normalized(value) => AttributeValue::Normalized(value),
        wire::ProgrammingAttributeValue::Spread(values) => AttributeValue::Spread(values),
        wire::ProgrammingAttributeValue::Discrete(value) => AttributeValue::Discrete(value),
        wire::ProgrammingAttributeValue::ColorXyz(value) => AttributeValue::ColorXyz(Xyz {
            x: value.x,
            y: value.y,
            z: value.z,
        }),
        wire::ProgrammingAttributeValue::RawDmx(value) => AttributeValue::RawDmx(value),
        wire::ProgrammingAttributeValue::RawDmxExact(value) => AttributeValue::RawDmxExact(value),
    }
}

fn fixture_value(
    value: &light_programmer::ProgrammerFixtureUpdate,
) -> wire::ProgrammingFixtureValue {
    wire::ProgrammingFixtureValue {
        fixture_id: value.fixture_id.0,
        attribute: value.attribute.0.clone(),
        value: attribute_value(&value.value),
        programmer_order: value.programmer_order,
        fade: value.fade,
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

fn group_value(value: &light_programmer::ProgrammerGroupUpdate) -> wire::ProgrammingGroupValue {
    wire::ProgrammingGroupValue {
        group_id: value.group_id.clone(),
        attribute: value.attribute.0.clone(),
        value: attribute_value(&value.value),
        programmer_order: value.programmer_order,
        fade: value.fade,
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

fn attribute_value(value: &AttributeValue) -> wire::ProgrammingAttributeValue {
    match value {
        AttributeValue::Normalized(value) => wire::ProgrammingAttributeValue::Normalized(*value),
        AttributeValue::Spread(values) => wire::ProgrammingAttributeValue::Spread(values.clone()),
        AttributeValue::Discrete(value) => wire::ProgrammingAttributeValue::Discrete(value.clone()),
        AttributeValue::ColorXyz(value) => {
            wire::ProgrammingAttributeValue::ColorXyz(wire::ProgrammingColorXyz {
                x: value.x,
                y: value.y,
                z: value.z,
            })
        }
        AttributeValue::RawDmx(value) => wire::ProgrammingAttributeValue::RawDmx(*value),
        AttributeValue::RawDmxExact(value) => wire::ProgrammingAttributeValue::RawDmxExact(*value),
    }
}
