use light_core::AttributeValue;
use light_programmer::{
    Preset, ProgrammerFixtureUpdate, ProgrammerGroupUpdate, ProgrammerUpdateContent,
};

use super::model::UpdateAddress;

#[derive(Clone, Copy)]
pub(super) enum IncomingValue<'a> {
    Fixture(&'a ProgrammerFixtureUpdate),
    Group(&'a ProgrammerGroupUpdate),
}

impl IncomingValue<'_> {
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
        }
    }

    pub(super) fn value(&self) -> &AttributeValue {
        match self {
            Self::Fixture(value) => &value.value,
            Self::Group(value) => &value.value,
        }
    }

    pub(super) fn fade_millis(&self) -> Option<u64> {
        match self {
            Self::Fixture(value) => value.fade_millis,
            Self::Group(value) => value.fade_millis,
        }
    }

    pub(super) fn delay_millis(&self) -> Option<u64> {
        match self {
            Self::Fixture(value) => value.delay_millis,
            Self::Group(value) => value.delay_millis,
        }
    }
}

pub(super) fn incoming_values(content: &ProgrammerUpdateContent) -> Vec<IncomingValue<'_>> {
    content
        .fixture_values
        .iter()
        .map(IncomingValue::Fixture)
        .chain(content.group_values.iter().map(IncomingValue::Group))
        .collect()
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
            UpdateAddress::GroupMembership { .. } => false,
        })
        .collect()
}
