//! The Freeze, Highlight and fitted-native state a profile head renders under.

use super::{ProfileHeadInputs, look_for_role, resolved_highlight_layer};
use crate::native_position_projection::NativePositionInput;
use crate::{GroupMasterIndex, RenderOptions};
use light_core::{AttributeKey, FixtureId};
use light_fixture::{
    FixtureChannel, FixtureMode, HighlightLook, HighlightLookCompatibility, PatchedFixture,
    SignalLossPolicy,
};
use light_programmer::HighlightOutputLayer;
use std::collections::HashMap;

/// The Freeze and Highlight state one head renders under, shared by the ordinary head and the
/// head whose input a fitted native position replaces.
pub(super) struct HeadOverlayState {
    pub(super) options: RenderOptions,
    pub(super) layer: Option<HighlightOutputLayer>,
    pub(super) output_highlighted: bool,
    pub(super) selected_look: Option<HighlightLook>,
    pub(super) legacy_raw_highlight: bool,
    pub(super) group_scale: f32,
}

impl HeadOverlayState {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve(
        fixture: &PatchedFixture,
        owner: FixtureId,
        native_full_freeze: bool,
        options: RenderOptions,
        group_masters: &GroupMasterIndex,
        group_master_flashes: &HashMap<String, f32>,
        highlight_layers: &HashMap<FixtureId, HighlightOutputLayer>,
        highlight_look: &HighlightLook,
    ) -> Self {
        // Nothing frozen, nothing highlighted and nothing flashing is the ordinary state of a
        // desk, so each of these asks whether there is anything to look up before hashing this
        // head's identity.
        let full_freeze = native_full_freeze
            || !fixture.freeze.targets.is_empty()
                && fixture
                    .freeze
                    .targets
                    .get(&owner)
                    .is_some_and(|target| target.full);
        let options = if full_freeze {
            RenderOptions {
                grand_master: 1.0,
                blackout: false,
                control_loss_progress: None,
                ..options
            }
        } else {
            options
        };
        let layer = (!full_freeze)
            .then(|| resolved_highlight_layer(fixture.fixture_id, owner, highlight_layers))
            .flatten();
        let output_highlighted =
            layer.is_some() && !(fixture.definition.hazardous && options.blackout);
        let selected_look = layer
            .as_ref()
            .map(|layer| look_for_role(layer.role, highlight_look));
        let legacy_raw_highlight = output_highlighted
            && selected_look
                .as_ref()
                .is_some_and(|look| look.compatibility != HighlightLookCompatibility::Semantic);
        let group_scale = if full_freeze || output_highlighted || !fixture.group_masters_enabled {
            1.0
        } else {
            group_masters.scale(owner, group_master_flashes)
        };
        Self {
            options,
            layer,
            output_highlighted,
            selected_look,
            legacy_raw_highlight,
            group_scale,
        }
    }
}

pub(super) fn channel_matches_attribute(
    channel: &FixtureChannel,
    attribute: &AttributeKey,
) -> bool {
    *attribute == channel.attribute
        || *attribute == channel.fixture_attribute
        || *attribute == FixtureMode::control_action_attribute(channel.id)
        || channel
            .functions
            .iter()
            .any(|function| function.attribute == *attribute)
}

/// Seeds a fitted native position as this channel's complete input and returns the attributes
/// derived from it.
pub(super) fn seed_native_candidate(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    options: RenderOptions,
    inputs: &mut ProfileHeadInputs<'_, '_>,
    index: usize,
    native: &NativePositionInput,
) -> Vec<AttributeKey> {
    let mut derived_attributes = Vec::new();
    let channel = &mode.channels[index];

    inputs.values.remove(&channel.attribute);
    inputs
        .values
        .insert(channel.attribute.clone(), native.value.clone());
    derived_attributes.push(channel.attribute.clone());
    let safe_keys: Vec<_> = fixture
        .definition
        .safe_values
        .keys()
        .filter(|attribute| channel_matches_attribute(channel, attribute))
        .collect();
    let loss_progress = options.control_loss_progress.and_then(|progress| {
        match fixture.definition.effective_signal_loss_policy() {
            SignalLossPolicy::HoldLast => None,
            SignalLossPolicy::ImmediateSafe => Some(1.0),
            SignalLossPolicy::FadeToSafe { .. } => Some(progress.clamp(0.0, 1.0)),
        }
    });
    let replacing = (fixture.definition.hazardous && options.blackout && !safe_keys.is_empty())
        || loss_progress.is_some_and(|progress| {
            safe_keys.iter().any(|attribute| {
                progress >= 1.0 || mode.head_attribute_is_snap(inputs.head_id, attribute)
            })
        });
    if replacing {
        // A safety alias/function must become a real candidate, without the old native
        // canonical candidate defeating it. Other safe candidates keep normal priorities.
        inputs.values.remove(&channel.attribute);
        derived_attributes.clear();
    } else if loss_progress.is_some() {
        // Existing Raw->safe fading holds until completion. Seed every applicable safety
        // alias from the same exact raw base so an absent alias cannot start a new fade.
        for attribute in safe_keys {
            inputs
                .values
                .insert(attribute.clone(), native.value.clone());
            if !derived_attributes.contains(attribute) {
                derived_attributes.push(attribute.clone());
            }
        }
    }
    derived_attributes
}
