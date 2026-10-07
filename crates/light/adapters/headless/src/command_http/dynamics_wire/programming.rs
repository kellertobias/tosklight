use super::*;
use crate::runtime::command_http::intent_wire::ToIntentWire;

pub(super) fn address(value: &domain::DynamicValueAddress) -> wire::DynamicValueAddressProjection {
    use domain::DynamicFamilyRepresentation as D;
    use wire::DynamicFamilyRepresentationProjection as W;
    wire::DynamicValueAddressProjection {
        representation: match &value.representation {
            D::Angles => W::Angles,
            D::Target { reference } => W::Target {
                reference: reference.as_ref().map(ToIntentWire::to_intent_wire),
            },
            D::SemanticColor { basis } => W::SemanticColor {
                basis: match basis {
                    domain::DynamicSemanticColorBasis::Retain => {
                        wire::DynamicSemanticColorBasisProjection::Retain
                    }
                    domain::DynamicSemanticColorBasis::Recipe => {
                        wire::DynamicSemanticColorBasisProjection::Recipe
                    }
                    domain::DynamicSemanticColorBasis::HueSaturation => {
                        wire::DynamicSemanticColorBasisProjection::HueSaturation
                    }
                    domain::DynamicSemanticColorBasis::Whole => {
                        wire::DynamicSemanticColorBasisProjection::Whole
                    }
                },
            },
            D::DirectColor { source } => W::DirectColor {
                source: source.to_intent_wire(),
            },
            D::Focus => W::Focus,
            D::Zoom { convention } => W::Zoom {
                convention: convention.to_intent_wire(),
            },
        },
        component: value.component.as_ref().map(ToIntentWire::to_intent_wire),
    }
}

pub(super) fn value(value: &domain::DynamicValue) -> wire::DynamicValueProjection {
    match value {
        domain::DynamicValue::Scalar(value) => wire::DynamicValueProjection::Scalar(*value),
        domain::DynamicValue::Native(value) => wire::DynamicValueProjection::Native(*value),
        domain::DynamicValue::Family(value) => {
            wire::DynamicValueProjection::Family(super::super::values_wire::attribute_value(value))
        }
    }
}

pub(super) fn source(source: &domain::DynamicValueSource) -> wire::DynamicValueSourceProjection {
    match source {
        domain::DynamicValueSource::Current => wire::DynamicValueSourceProjection::Current,
        domain::DynamicValueSource::Value { value: v } => {
            wire::DynamicValueSourceProjection::Value { value: value(v) }
        }
        domain::DynamicValueSource::Preset {
            preset_id,
            address: a,
            last_valid_by_target,
            retained,
        } => wire::DynamicValueSourceProjection::Preset {
            retained: retained.as_ref().map(|template| preset_template(template)),
            preset_id: preset_id.clone(),
            address: address(a),
            last_valid_by_target: last_valid_by_target
                .iter()
                .map(|fallback| wire::DynamicValueFallbackProjection {
                    target: fallback.target.0,
                    value: value(&fallback.value),
                })
                .collect(),
        },
    }
}

fn preset_template(
    template: &domain::DynamicPresetTemplate,
) -> wire::DynamicPresetTemplateProjection {
    wire::DynamicPresetTemplateProjection {
        universal: template
            .universal
            .as_ref()
            .map(super::super::values_wire::attribute_value),
        groups: template
            .groups
            .iter()
            .map(|group| wire::DynamicPresetGroupTemplateProjection {
                group_id: group.group_id.clone(),
                value: super::super::values_wire::attribute_value(&group.value),
            })
            .collect(),
        fixtures: template
            .fixtures
            .iter()
            .map(|fixture| wire::DynamicPresetFixtureTemplateProjection {
                fixture_id: fixture.fixture_id.0,
                value: super::super::values_wire::attribute_value(&fixture.value),
            })
            .collect(),
        fallback: template
            .fallback
            .as_ref()
            .map(|fallback| Box::new(preset_template(fallback))),
    }
}

pub(super) fn lane(body: &domain::ProgrammingLaneBody) -> wire::DynamicProgrammingLaneProjection {
    use domain::ProgrammingLaneConfiguration as D;
    use wire::DynamicProgrammingLaneConfigurationProjection as W;
    wire::DynamicProgrammingLaneProjection {
        address: address(&body.address),
        configuration: match &body.configuration {
            D::Keyframes(config) => W::Keyframes(wire::DynamicProgrammingKeyframesProjection {
                size: config.size,
                points: config
                    .points
                    .iter()
                    .map(|point| wire::DynamicProgrammingKeyframeProjection {
                        position: point.position,
                        source: source(&point.source),
                        interpolation: interpolation(point.interpolation),
                    })
                    .collect(),
            }),
            D::MaxMin(config) => W::MaxMin(wire::DynamicProgrammingMaxMinProjection {
                minimum: source(&config.minimum),
                maximum: source(&config.maximum),
                function: periodic_function(config.function),
                size: config.size,
                pwm: pwm(config.pwm),
            }),
            D::MiddleAmplitude(config) => {
                W::MiddleAmplitude(wire::DynamicProgrammingMiddleAmplitudeProjection {
                    middle: source(&config.middle),
                    amplitude: value(&config.amplitude),
                    function: periodic_function(config.function),
                    size: config.size,
                    pwm: pwm(config.pwm),
                    invert_waveform: config.invert_waveform,
                })
            }
            D::Random => W::Random,
        },
    }
}
