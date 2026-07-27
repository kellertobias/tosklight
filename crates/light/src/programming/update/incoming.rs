use light_core::AttributeValue;
use light_dynamics::{DynamicAddressValue, DynamicSemanticValue, DynamicValueTiming};
use light_programmer::{
    Preset, ProgrammerFixtureUpdate, ProgrammerGroupUpdate, ProgrammerUpdateContent,
};

use super::model::UpdateAddress;

#[derive(Clone, Copy)]
pub(super) enum IncomingValue<'a> {
    Fixture(&'a ProgrammerFixtureUpdate),
    Group(&'a ProgrammerGroupUpdate),
    Dynamic(&'a DynamicAddressValue),
}

impl IncomingValue<'_> {
    pub(super) fn programmer_order(&self) -> u64 {
        match self {
            Self::Fixture(value) => value.programmer_order,
            Self::Group(value) => value.programmer_order,
            Self::Dynamic(value) => value.programmer_order,
        }
    }

    pub(super) fn address(&self) -> UpdateAddress {
        match self {
            Self::Fixture(value) => UpdateAddress::FixtureAttribute {
                fixture_id: value.fixture_id,
                attribute: value.attribute.clone(),
            },
            Self::Group(value) => UpdateAddress::GroupAttribute {
                group_id: value.group_id.clone(),
                attribute: value.attribute.clone(),
            },
            Self::Dynamic(value) => UpdateAddress::DynamicAttribute {
                fixture_id: value.fixture_id,
                attribute: value.attribute.clone(),
                instance_link: dynamic_instance_link(&value.value),
            },
        }
    }

    pub(super) fn ordinary_value(&self) -> Option<&AttributeValue> {
        match self {
            Self::Fixture(value) => Some(&value.value),
            Self::Group(value) => Some(&value.value),
            Self::Dynamic(_) => None,
        }
    }

    pub(super) fn dynamic_value(&self) -> Option<&DynamicSemanticValue> {
        match self {
            Self::Dynamic(value) => Some(&value.value),
            Self::Fixture(_) | Self::Group(_) => None,
        }
    }

    pub(super) fn fade_millis(&self) -> Option<u64> {
        match self {
            Self::Fixture(value) => value.fade_millis,
            Self::Group(value) => value.fade_millis,
            Self::Dynamic(value) => dynamic_timing(&value.value).fade_millis,
        }
    }

    pub(super) fn delay_millis(&self) -> Option<u64> {
        match self {
            Self::Fixture(value) => value.delay_millis,
            Self::Group(value) => value.delay_millis,
            Self::Dynamic(value) => dynamic_timing(&value.value).delay_millis,
        }
    }
}

pub(super) fn incoming_values(content: &ProgrammerUpdateContent) -> Vec<IncomingValue<'_>> {
    let mut values = content
        .fixture_values
        .iter()
        .map(IncomingValue::Fixture)
        .chain(content.group_values.iter().map(IncomingValue::Group))
        .chain(content.dynamic_values.iter().map(IncomingValue::Dynamic))
        .collect::<Vec<_>>();
    values.sort_by_key(IncomingValue::programmer_order);
    values
}

pub(super) fn incoming_preset_values<'a>(
    preset: &Preset,
    content: &'a ProgrammerUpdateContent,
) -> Vec<IncomingValue<'a>> {
    incoming_values(content)
        .into_iter()
        .filter(|incoming| match incoming.address() {
            UpdateAddress::FixtureAttribute { ref attribute, .. }
            | UpdateAddress::GroupAttribute { ref attribute, .. } => {
                preset.family.accepts(attribute)
            }
            UpdateAddress::DynamicAttribute { .. } | UpdateAddress::GroupMembership { .. } => false,
        })
        .collect()
}

fn dynamic_instance_link(value: &DynamicSemanticValue) -> Option<uuid::Uuid> {
    match value {
        DynamicSemanticValue::DynamicOn { instance_link, .. }
        | DynamicSemanticValue::DynamicOff { instance_link, .. } => Some(*instance_link),
        DynamicSemanticValue::Static { .. }
        | DynamicSemanticValue::FixAt { .. }
        | DynamicSemanticValue::Release => None,
    }
}

fn dynamic_timing(value: &DynamicSemanticValue) -> DynamicValueTiming {
    match value {
        DynamicSemanticValue::Static { timing, .. }
        | DynamicSemanticValue::DynamicOn { timing, .. }
        | DynamicSemanticValue::DynamicOff { timing, .. }
        | DynamicSemanticValue::FixAt { timing, .. } => *timing,
        DynamicSemanticValue::Release => DynamicValueTiming::default(),
    }
}
