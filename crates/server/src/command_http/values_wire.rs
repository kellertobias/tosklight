use light_application as application;
use light_core::AttributeValue;
use light_wire::v2::{events::EventSnapshotCursor, programming as wire};

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

fn values_projection(
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
