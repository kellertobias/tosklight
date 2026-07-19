use crate::{EngineError, GroupMasterIndex, RenderOptions, apply_safe_values};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_fixture::{PatchedFixture, SignalLossPolicy, encode_parameter, mix_color};
use light_output::DmxFrame;
use std::collections::HashMap;

pub(crate) fn render_fixture(
    frame: &mut DmxFrame,
    fixture: &PatchedFixture,
    resolved: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
    options: RenderOptions,
    group_masters: &GroupMasterIndex,
    group_master_flashes: &HashMap<String, f32>,
) -> Result<(), EngineError> {
    let Some(address) = fixture.address else {
        return Ok(());
    };
    for head in &fixture.definition.heads {
        let owner = if head.shared {
            fixture.fixture_id
        } else {
            fixture
                .logical_heads
                .iter()
                .find(|patched| patched.head_index == head.index)
                .map(|patched| patched.fixture_id)
                .unwrap_or(fixture.fixture_id)
        };
        let group_scale = group_masters.scale(owner, group_master_flashes);
        let mut abstract_values: HashMap<AttributeKey, AttributeValue> = resolved
            .iter()
            .filter(|((fixture_id, _), _)| *fixture_id == owner)
            .map(|((_, attribute), value)| (attribute.clone(), value.clone()))
            .collect();
        if let Some(progress) = options.control_loss_progress {
            match fixture.definition.effective_signal_loss_policy() {
                SignalLossPolicy::HoldLast => {}
                SignalLossPolicy::ImmediateSafe => {
                    apply_safe_values(&mut abstract_values, &fixture.definition.safe_values, 1.0)
                }
                SignalLossPolicy::FadeToSafe { .. } => apply_safe_values(
                    &mut abstract_values,
                    &fixture.definition.safe_values,
                    progress.clamp(0.0, 1.0),
                ),
            }
        }
        if fixture.definition.hazardous && options.blackout {
            for (attribute, value) in &fixture.definition.safe_values {
                abstract_values.insert(attribute.clone(), value.clone());
            }
        }
        let intensity_key = AttributeKey::intensity();
        let intensity = if options.blackout {
            0.0
        } else {
            abstract_values
                .get(&intensity_key)
                .and_then(AttributeValue::normalized)
                .unwrap_or(1.0)
                * group_scale
                * options.grand_master.clamp(0.0, 1.0)
        };
        let has_physical_dimmer = head
            .parameters
            .iter()
            .any(|parameter| parameter.attribute.is_intensity() && !parameter.virtual_dimmer);
        if let (Some(AttributeValue::ColorXyz(color)), Some(calibration)) = (
            abstract_values.get(&AttributeKey("color".into())),
            &fixture.definition.color_calibration,
        ) {
            let mut levels = mix_color(*color, calibration)?;
            if !has_physical_dimmer {
                for level in &mut levels {
                    *level *= intensity;
                }
            }
            for (emitter, level) in calibration.emitters.iter().zip(levels) {
                abstract_values
                    .entry(AttributeKey(format!(
                        "color.emitter.{}",
                        emitter.name.to_lowercase()
                    )))
                    .or_insert(AttributeValue::Normalized(level));
            }
        }
        for parameter in &head.parameters {
            let mut level = abstract_values
                .get(&parameter.attribute)
                .and_then(AttributeValue::normalized)
                .unwrap_or(parameter.default);
            if parameter.attribute.is_intensity() {
                level *= group_scale;
                level *= options.grand_master.clamp(0.0, 1.0);
                if options.blackout {
                    level = 0.0;
                }
            }
            if parameter.virtual_dimmer {
                level *= intensity;
            }
            if parameter.components.is_empty() {
                continue;
            }
            encode_parameter(frame, address, parameter, level)?;
        }
        for (attribute, value) in &abstract_values {
            if let (Some(offset), AttributeValue::RawDmx(raw)) = (
                attribute
                    .0
                    .strip_prefix("dmx.")
                    .and_then(|offset| offset.parse::<u16>().ok()),
                value,
            ) && offset < fixture.definition.footprint
            {
                frame[usize::from(address - 1 + offset)] = *raw;
            }
        }
    }
    Ok(())
}
