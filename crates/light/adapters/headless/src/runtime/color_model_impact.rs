//! What switching a programmed show between Direct and Color Intent does to the colour it already
//! stores. The switch itself rewrites nothing; this report says which stored values will play or
//! edit differently afterwards, and which of those cannot come back unchanged.

use super::command_http::{ColorAttributeIndex, ColorTarget};
use light_core::{AttributeKey, AttributeValue, ColorProgrammingModel, FixtureId, Xyz};
use light_wire::v2::attribute_configuration as wire;

#[derive(Default)]
struct Counts {
    native: u32,
    dimmed: u32,
    unresolved: u32,
}

impl Counts {
    fn visit(
        &mut self,
        fixture: Option<FixtureId>,
        attribute: &AttributeKey,
        value: &AttributeValue,
        fixtures: &ColorAttributeIndex,
    ) {
        if attribute.0.starts_with("color.") {
            self.native += 1;
            return;
        }
        let AttributeValue::ColorXyz(color) = value else {
            return;
        };
        if carries_brightness(*color) {
            self.dimmed += 1;
        }
        if fixture
            .and_then(|fixture| fixtures.get(&fixture))
            .is_some_and(|target| matches!(target, ColorTarget::Direct(_)))
        {
            self.unresolved += 1;
        }
    }
}

/// A whole colour whose own level is below full: Direct plays it dimmer, Intent at full.
fn carries_brightness(color: Xyz) -> bool {
    let linear = [
        3.240_454_2 * color.x - 1.537_138_5 * color.y - 0.498_531_4 * color.z,
        -0.969_266 * color.x + 1.876_010_8 * color.y + 0.041_556 * color.z,
        0.055_643_4 * color.x - 0.204_025_9 * color.y + 1.057_225_2 * color.z,
    ];
    linear.into_iter().fold(0.0_f32, f32::max) < 0.98
}

/// Scan every Preset and Cue of the show document.
pub(super) fn color_model_impact(
    document: &light_show::PortableShowDocument,
    fixtures: &ColorAttributeIndex,
    from: ColorProgrammingModel,
    to: ColorProgrammingModel,
) -> wire::ColorModelImpact {
    let mut counts = Counts::default();
    for object in document.objects_of_kind("preset") {
        let Ok(preset) = serde_json::from_value::<light_programmer::Preset>(object.body().clone())
        else {
            continue;
        };
        for (fixture, values) in &preset.values {
            for (attribute, value) in values {
                counts.visit(Some(*fixture), attribute, value, fixtures);
            }
        }
        for values in preset.group_values.values() {
            for (attribute, value) in values {
                counts.visit(None, attribute, value, fixtures);
            }
        }
        // A universal colour reaches whatever is selected, so it is lost wherever Direct cannot
        // resolve a whole colour.
        for (attribute, value) in &preset.universal_values {
            counts.visit(None, attribute, value, fixtures);
            if matches!(value, AttributeValue::ColorXyz(_)) && fixtures.any_direct() {
                counts.unresolved += 1;
            }
        }
    }
    for object in document.objects_of_kind("cue_list") {
        let Ok(cue_list) = serde_json::from_value::<light_playback::CueList>(object.body().clone())
        else {
            continue;
        };
        for cue in &cue_list.cues {
            for change in &cue.changes {
                if let Some(value) = &change.value {
                    counts.visit(Some(change.fixture_id), &change.attribute, value, fixtures);
                }
            }
            for change in &cue.group_changes {
                if let Some(value) = &change.value {
                    counts.visit(None, &change.attribute, value, fixtures);
                }
            }
        }
    }
    let mut items = Vec::new();
    match (from, to) {
        (ColorProgrammingModel::Direct, ColorProgrammingModel::Intent) => {
            if counts.native > 0 {
                items.push(wire::ColorModelImpactItem {
                    kind: wire::ColorModelImpactKind::NativeColorValues,
                    count: counts.native,
                    lossy: false,
                    message: format!(
                        "{} stored fixture-native colour value(s) are kept and still play, but can \
                         no longer be edited from the Color feature.",
                        counts.native
                    ),
                });
            }
            if counts.dimmed > 0 {
                items.push(wire::ColorModelImpactItem {
                    kind: wire::ColorModelImpactKind::DimmedWholeColors,
                    count: counts.dimmed,
                    lossy: true,
                    message: format!(
                        "{} stored colour(s) carry their own brightness. Color Intent shows them \
                         at full brightness; set the level with Intensity instead.",
                        counts.dimmed
                    ),
                });
            }
        }
        (ColorProgrammingModel::Intent, ColorProgrammingModel::Direct) => {
            if counts.unresolved > 0 {
                items.push(wire::ColorModelImpactItem {
                    kind: wire::ColorModelImpactKind::UnresolvedWholeColors,
                    count: counts.unresolved,
                    lossy: true,
                    message: format!(
                        "{} stored colour(s) are on fixtures without an authored colour system. \
                         Direct cannot resolve them, so those fixtures lose that colour.",
                        counts.unresolved
                    ),
                });
            }
        }
        _ => {}
    }
    wire::ColorModelImpact {
        from: wire_model(from),
        to: wire_model(to),
        lossy: items.iter().any(|item| item.lossy),
        items,
    }
}

pub(super) fn wire_model(model: ColorProgrammingModel) -> wire::ColorProgrammingModel {
    match model {
        ColorProgrammingModel::Direct => wire::ColorProgrammingModel::Direct,
        ColorProgrammingModel::Intent => wire::ColorProgrammingModel::Intent,
    }
}

pub(super) fn domain_model(model: wire::ColorProgrammingModel) -> ColorProgrammingModel {
    match model {
        wire::ColorProgrammingModel::Direct => ColorProgrammingModel::Direct,
        wire::ColorProgrammingModel::Intent => ColorProgrammingModel::Intent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_at_its_own_full_level_carries_no_brightness() {
        let red = light_fixture::srgb_to_xyz(1.0, 0.0, 0.0);
        assert!(!carries_brightness(red));
        let half = light_fixture::srgb_to_xyz(0.5, 0.0, 0.0);
        assert!(carries_brightness(half));
        assert!(carries_brightness(Xyz {
            x: 0.0,
            y: 0.0,
            z: 0.0
        }));
    }
}
