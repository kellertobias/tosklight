use light_core::AttributeValue;
use light_programmer::{Preset, ProgrammerUpdateContent};

use super::error::UpdateError;
use super::incoming::{IncomingValue, incoming_preset_values};
use super::model::{
    ExistingContentMode, UpdateAddress, UpdateIgnoreReason, UpdateItemOutcome, UpdateMode,
    UpdatePreview, UpdatePreviewItem, UpdateTargetFamily, UpdateTargetIdentity,
};
use super::plan::{AtomicUpdatePlan, PlannedUpdateObject, ensure_revision};

pub fn preview_preset_update(
    preset_id: &str,
    preset: &Preset,
    mode: ExistingContentMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<UpdatePreview, UpdateError> {
    if !programmer.has_values() {
        return Err(UpdateError::EmptyProgrammer {
            target_family: UpdateTargetFamily::Preset,
        });
    }
    let items = incoming_preset_values(preset, programmer)
        .into_iter()
        .map(|incoming| preset_preview_item(preset, mode, incoming))
        .collect();
    Ok(UpdatePreview {
        target: UpdateTargetIdentity::preset(preset_id, preset),
        mode: UpdateMode::ExistingContent(mode),
        items,
    })
}

fn preset_preview_item(
    preset: &Preset,
    mode: ExistingContentMode,
    incoming: IncomingValue<'_>,
) -> UpdatePreviewItem {
    let address = incoming.address();
    let existing = stored_value(preset, &address);
    let outcome = match (mode, existing) {
        (_, Some(value)) if value == incoming.value() => {
            UpdateItemOutcome::Unchanged { source: None }
        }
        (_, Some(_)) => UpdateItemOutcome::UpdateExisting,
        (ExistingContentMode::UpdateExisting, None) => UpdateItemOutcome::Ignored {
            reason: UpdateIgnoreReason::NewAddress,
        },
        (ExistingContentMode::AddNew, None) => UpdateItemOutcome::AddNew,
    };
    UpdatePreviewItem { address, outcome }
}

fn stored_value<'a>(preset: &'a Preset, address: &UpdateAddress) -> Option<&'a AttributeValue> {
    match address {
        UpdateAddress::FixtureAttribute {
            fixture_id,
            attribute,
        } => preset
            .values
            .get(fixture_id)
            .and_then(|attributes| attributes.get(attribute)),
        UpdateAddress::GroupAttribute {
            group_id,
            attribute,
        } => preset
            .group_values
            .get(group_id)
            .and_then(|attributes| attributes.get(attribute)),
        UpdateAddress::GroupMembership { .. } => None,
    }
}

fn write_preset_value(preset: &mut Preset, incoming: IncomingValue<'_>) {
    match incoming {
        IncomingValue::Fixture(value) => {
            preset
                .values
                .entry(value.fixture_id)
                .or_default()
                .insert(value.attribute.clone(), value.value.clone());
        }
        IncomingValue::Group(value) => {
            preset
                .group_values
                .entry(value.group_id.clone())
                .or_default()
                .insert(value.attribute.clone(), value.value.clone());
        }
    }
}

pub fn plan_preset_update(
    preset_id: &str,
    preset: &Preset,
    current_revision: u64,
    expected_revision: u64,
    mode: ExistingContentMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<AtomicUpdatePlan, UpdateError> {
    ensure_revision(expected_revision, current_revision)?;
    let preview = preview_preset_update(preset_id, preset, mode, programmer)?;
    if !preview.has_real_change() {
        return Err(UpdateError::NoOp {
            target: preview.target,
        });
    }
    let mut updated = preset.clone();
    for (incoming, item) in incoming_preset_values(preset, programmer)
        .into_iter()
        .zip(&preview.items)
    {
        if item.outcome.changes_data() {
            write_preset_value(&mut updated, incoming);
        }
    }
    Ok(AtomicUpdatePlan {
        target: preview.target.clone(),
        expected_revision,
        preview,
        object: PlannedUpdateObject::Preset(updated),
    })
}
