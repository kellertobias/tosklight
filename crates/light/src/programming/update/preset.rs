use std::collections::HashMap;

use light_core::{AttributeKey, AttributeValue};
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
    let existing = stored_value(preset, &address).map(StoredValue::value);
    let outcome = match (mode, existing) {
        (_, Some(value))
            if Some(value) == incoming.ordinary_value()
                && projection_matches(preset, &address, incoming) =>
        {
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

fn projection_matches(
    preset: &Preset,
    address: &UpdateAddress,
    incoming: IncomingValue<'_>,
) -> bool {
    match address {
        UpdateAddress::FixtureAttribute {
            fixture_id,
            attribute,
        } => {
            preset
                .fixture_replacement_projections
                .get(fixture_id)
                .and_then(|values| values.get(attribute))
                == incoming.replacement_projection()
        }
        UpdateAddress::GroupAttribute {
            group_id,
            attribute,
        } => {
            let stored = preset
                .group_replacement_projections
                .get(group_id)
                .and_then(|values| values.get(attribute));
            match (stored, incoming.replacement_projections()) {
                (None, Some(incoming)) => incoming.is_empty(),
                (Some(stored), Some(incoming)) => stored == incoming,
                _ => false,
            }
        }
        _ => true,
    }
}

/// Where an address's existing Preset content comes from.
///
/// An explicit fixture or Group entry wins. Otherwise a universal value — one shared intent that
/// recall applies to every selected fixture and live Group, named or not — is that address's
/// existing content, so Update can change it like any other stored value.
#[derive(Clone, Copy)]
enum StoredValue<'a> {
    Explicit(&'a AttributeValue),
    Universal(&'a AttributeValue),
}

impl<'a> StoredValue<'a> {
    fn value(self) -> &'a AttributeValue {
        match self {
            Self::Explicit(value) | Self::Universal(value) => value,
        }
    }
}

fn stored_value<'a>(preset: &'a Preset, address: &UpdateAddress) -> Option<StoredValue<'a>> {
    let (explicit, attribute) = match address {
        UpdateAddress::FixtureAttribute {
            fixture_id,
            attribute,
        } => (
            preset
                .values
                .get(fixture_id)
                .and_then(|attributes| attributes.get(attribute)),
            attribute,
        ),
        UpdateAddress::GroupAttribute {
            group_id,
            attribute,
        } => (
            preset
                .group_values
                .get(group_id)
                .and_then(|attributes| attributes.get(attribute)),
            attribute,
        ),
        UpdateAddress::DynamicAttribute { .. } | UpdateAddress::GroupMembership { .. } => {
            return None;
        }
    };
    explicit.map(StoredValue::Explicit).or_else(|| {
        preset
            .universal_values
            .get(attribute)
            .map(StoredValue::Universal)
    })
}

/// The new universal value per attribute: the one intent every Programmer address that reads
/// that universal value now shares. Addresses that disagree keep the universal value and store
/// their own explicit value instead, so deliberately different values never auto-extend.
fn shared_universal_updates(
    preset: &Preset,
    incoming: &[IncomingValue<'_>],
) -> HashMap<AttributeKey, AttributeValue> {
    let mut shared = HashMap::<AttributeKey, Option<&AttributeValue>>::new();
    for value in incoming {
        let address = value.address();
        let (Some(StoredValue::Universal(_)), Some(requested)) =
            (stored_value(preset, &address), value.ordinary_value())
        else {
            continue;
        };
        let Some(attribute) = ordinary_attribute(&address) else {
            continue;
        };
        // A live Group family assignment (template plus member exceptions) is valid only on its
        // Group owner, never as a universal value; it stays the Group's explicit value.
        let candidate = (!matches!(requested, AttributeValue::GroupFamily(_))
            && value.replacement_projection().is_none()
            && value
                .replacement_projections()
                .is_none_or(|map| map.is_empty()))
        .then_some(requested);
        shared
            .entry(attribute.clone())
            .and_modify(|current| {
                if *current != candidate {
                    *current = None;
                }
            })
            .or_insert(candidate);
    }
    shared
        .into_iter()
        .filter_map(|(attribute, value)| value.map(|value| (attribute, value.clone())))
        .collect()
}

fn ordinary_attribute(address: &UpdateAddress) -> Option<&AttributeKey> {
    match address {
        UpdateAddress::FixtureAttribute { attribute, .. }
        | UpdateAddress::GroupAttribute { attribute, .. } => Some(attribute),
        UpdateAddress::DynamicAttribute { .. } | UpdateAddress::GroupMembership { .. } => None,
    }
}

fn write_preset_value(preset: &mut Preset, incoming: IncomingValue<'_>) {
    match incoming {
        IncomingValue::Fixture(value) => {
            let metadata = preset
                .fixture_replacement_projections
                .entry(value.fixture_id)
                .or_default();
            if let Some(projection) = &value.replacement_projection {
                metadata.insert(value.attribute.clone(), projection.clone());
            } else {
                metadata.remove(&value.attribute);
            }
            preset
                .values
                .entry(value.fixture_id)
                .or_default()
                .insert(value.attribute.clone(), value.value.clone());
        }
        IncomingValue::Group(value) => {
            let metadata = preset
                .group_replacement_projections
                .entry(value.group_id.clone())
                .or_default();
            if value.replacement_projections.is_empty() {
                metadata.remove(&value.attribute);
            } else {
                metadata.insert(
                    value.attribute.clone(),
                    value.replacement_projections.clone(),
                );
            }
            preset
                .group_values
                .entry(value.group_id.clone())
                .or_default()
                .insert(value.attribute.clone(), value.value.clone());
        }
        IncomingValue::Dynamic(_) => {}
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
    let incoming = incoming_preset_values(preset, programmer);
    let universal = shared_universal_updates(preset, &incoming);
    for (incoming, item) in incoming.into_iter().zip(&preview.items) {
        if !item.outcome.changes_data() {
            continue;
        }
        let replaces_universal = matches!(
            stored_value(preset, &item.address),
            Some(StoredValue::Universal(_))
        ) && ordinary_attribute(&item.address)
            .is_some_and(|attribute| universal.contains_key(attribute));
        if !replaces_universal {
            write_preset_value(&mut updated, incoming);
        }
    }
    // A universal value read by every Programmer address with one shared new intent stays
    // universal and takes that intent; disagreeing addresses were stored explicitly above.
    for (attribute, value) in universal {
        updated.universal_values.insert(attribute, value);
    }
    // Universal Color Presets come from recording one shared colour. Updating one with a single
    // shared colour keeps it universal; a different colour stays specific to its fixtures.
    if preset.is_universal() {
        updated.consolidate_universal_color();
    }
    updated.retain_family_attributes();
    Ok(AtomicUpdatePlan {
        target: preview.target.clone(),
        expected_revision,
        preview,
        object: PlannedUpdateObject::Preset(updated),
    })
}
