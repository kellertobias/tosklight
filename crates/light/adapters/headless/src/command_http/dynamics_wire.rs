use light_dynamics as domain;
use light_wire::v2::dynamics as wire;

pub(super) fn programming_value(
    value: &domain::DynamicAddressValue,
) -> light_wire::v2::programming::ProgrammingDynamicValue {
    light_wire::v2::programming::ProgrammingDynamicValue {
        fixture_id: value.fixture_id.0,
        attribute: value.attribute.0.clone(),
        value: semantic_value(&value.value),
        programmer_order: value.programmer_order,
        changed_at_millis: value.changed_at_millis,
    }
}

fn semantic_value(
    value: &domain::DynamicSemanticValue,
) -> light_wire::v2::programming::ProgrammingDynamicSemanticValue {
    use light_wire::v2::programming::ProgrammingDynamicSemanticValue as Wire;
    match value {
        domain::DynamicSemanticValue::Static { value, timing } => Wire::Static {
            value: super::values_wire::attribute_value(value),
            timing: timing_projection(*timing),
        },
        domain::DynamicSemanticValue::DynamicOn {
            instance_link,
            dynamic,
            lane_id,
            overrides,
            timing,
        } => Wire::DynamicOn {
            instance_link: *instance_link,
            dynamic: reference(dynamic),
            lane_id: *lane_id,
            overrides: overrides_projection(overrides),
            timing: timing_projection(*timing),
        },
        domain::DynamicSemanticValue::DynamicOff {
            instance_link,
            timing,
        } => Wire::DynamicOff {
            instance_link: *instance_link,
            timing: timing_projection(*timing),
        },
        domain::DynamicSemanticValue::FixAt { value, timing } => Wire::FixAt {
            value: *value,
            timing: timing_projection(*timing),
        },
        domain::DynamicSemanticValue::Release => Wire::Release,
    }
}

pub(super) fn definition(value: &domain::DynamicDefinition) -> wire::DynamicDefinitionProjection {
    wire::DynamicDefinitionProjection {
        id: value.id,
        pool_number: value.pool_number,
        revision: value.revision,
        name: value.name.clone(),
        color: value.color.clone(),
        icon: value.icon.clone(),
        target_binding: target_binding(&value.target_binding),
        lanes: value.lanes.iter().map(lane).collect(),
        random_groups: value.random_groups.iter().map(random_group).collect(),
        phase_mode: match value.phase_spread_mode {
            domain::DynamicPhaseSpreadMode::Uniform => {
                wire::DynamicPhaseSpreadModeProjection::Uniform
            }
            domain::DynamicPhaseSpreadMode::PerLane => {
                wire::DynamicPhaseSpreadModeProjection::PerLane
            }
        },
        phase: phase(&value.phase),
        speed: speed(&value.speed),
        overall_speed_multiplier: rational(value.overall_speed_multiplier),
        run_mode: match value.run_mode {
            domain::DynamicRunMode::Loop => wire::DynamicRunModeProjection::Loop,
            domain::DynamicRunMode::OneShot => wire::DynamicRunModeProjection::OneShot,
        },
        default_activation: activation(value.default_activation),
        activation_boundary: match value.activation_boundary {
            domain::ActivationBoundary::Beat => wire::DynamicActivationBoundaryProjection::Beat,
            domain::ActivationBoundary::Bar => wire::DynamicActivationBoundaryProjection::Bar,
        },
    }
}

fn target_binding(value: &domain::DynamicTargetBinding) -> wire::DynamicTargetBindingProjection {
    match value {
        domain::DynamicTargetBinding::LiveGroup { group_id } => {
            wire::DynamicTargetBindingProjection::LiveGroup {
                group_id: group_id.clone(),
            }
        }
        domain::DynamicTargetBinding::FrozenTargets { targets } => {
            wire::DynamicTargetBindingProjection::FrozenTargets {
                targets: targets.iter().map(|target| target.0).collect(),
            }
        }
        domain::DynamicTargetBinding::Targetless => {
            wire::DynamicTargetBindingProjection::Targetless
        }
    }
}

fn lane(value: &domain::DynamicLane) -> wire::DynamicLaneProjection {
    wire::DynamicLaneProjection {
        id: value.id,
        attribute: value.attribute.0.clone(),
        mode: match value.mode {
            domain::DynamicLaneMode::Keyframes => wire::DynamicLaneModeProjection::Keyframes,
            domain::DynamicLaneMode::MaxMin => wire::DynamicLaneModeProjection::MaxMin,
            domain::DynamicLaneMode::MiddleAmplitude => {
                wire::DynamicLaneModeProjection::MiddleAmplitude
            }
            domain::DynamicLaneMode::Random => wire::DynamicLaneModeProjection::Random,
        },
        keyframes: wire::DynamicKeyframeConfigurationProjection {
            points: value
                .keyframes
                .points
                .iter()
                .map(|point| wire::DynamicKeyframeProjection {
                    position: point.position,
                    source: scalar_source(&point.source),
                    interpolation: interpolation(point.interpolation),
                })
                .collect(),
            size: value.keyframes.size,
        },
        max_min: wire::DynamicMaxMinConfigurationProjection {
            minimum: scalar_source(&value.max_min.minimum),
            maximum: scalar_source(&value.max_min.maximum),
            function: periodic_function(value.max_min.function),
            size: value.max_min.size,
            pwm: pwm(value.max_min.pwm),
        },
        middle_amplitude: wire::DynamicMiddleAmplitudeConfigurationProjection {
            middle: scalar_source(&value.middle_amplitude.middle),
            amplitude: value.middle_amplitude.amplitude,
            function: periodic_function(value.middle_amplitude.function),
            size: value.middle_amplitude.size,
            pwm: pwm(value.middle_amplitude.pwm),
        },
        speed_multiplier: rational(value.speed_multiplier),
        width: value.width,
        random_group_id: value.random_group_id,
        phase: value.phase.as_ref().map(phase),
    }
}

fn scalar_source(value: &domain::ScalarSource) -> wire::DynamicScalarSourceProjection {
    match value {
        domain::ScalarSource::Current => wire::DynamicScalarSourceProjection::Current,
        domain::ScalarSource::Value { value } => {
            wire::DynamicScalarSourceProjection::Value { value: *value }
        }
        domain::ScalarSource::Preset {
            preset_id,
            attribute,
            last_valid_by_target,
        } => wire::DynamicScalarSourceProjection::Preset {
            preset_id: preset_id.clone(),
            attribute: attribute.0.clone(),
            last_valid_by_target: last_valid_by_target
                .iter()
                .map(|fallback| wire::DynamicTargetScalarFallbackProjection {
                    target: fallback.target.0,
                    value: fallback.value,
                })
                .collect(),
        },
    }
}

fn interpolation(value: domain::ScalarInterpolation) -> wire::DynamicScalarInterpolationProjection {
    match value {
        domain::ScalarInterpolation::Linear => wire::DynamicScalarInterpolationProjection::Linear,
        domain::ScalarInterpolation::EaseIn => wire::DynamicScalarInterpolationProjection::EaseIn,
        domain::ScalarInterpolation::EaseOut => wire::DynamicScalarInterpolationProjection::EaseOut,
        domain::ScalarInterpolation::EaseInOut => {
            wire::DynamicScalarInterpolationProjection::EaseInOut
        }
        domain::ScalarInterpolation::Hold => wire::DynamicScalarInterpolationProjection::Hold,
        domain::ScalarInterpolation::Drop => wire::DynamicScalarInterpolationProjection::Drop,
    }
}

fn periodic_function(value: domain::PeriodicFunction) -> wire::DynamicPeriodicFunctionProjection {
    match value {
        domain::PeriodicFunction::Sinus => wire::DynamicPeriodicFunctionProjection::Sinus,
        domain::PeriodicFunction::Cosinus => wire::DynamicPeriodicFunctionProjection::Cosinus,
        domain::PeriodicFunction::LinearUp => wire::DynamicPeriodicFunctionProjection::LinearUp,
        domain::PeriodicFunction::LinearDown => wire::DynamicPeriodicFunctionProjection::LinearDown,
        domain::PeriodicFunction::Pwm => wire::DynamicPeriodicFunctionProjection::Pwm,
    }
}

fn pwm(value: domain::PwmShape) -> wire::DynamicPwmShapeProjection {
    wire::DynamicPwmShapeProjection {
        attack: value.attack,
        on: value.on,
        decay: value.decay,
        off: value.off,
        attack_interpolation: interpolation(value.attack_interpolation),
        decay_interpolation: interpolation(value.decay_interpolation),
    }
}

fn random_group(value: &domain::DynamicRandomGroup) -> wire::DynamicRandomGroupProjection {
    wire::DynamicRandomGroupProjection {
        id: value.id,
        seed: value.seed,
        low: scalar_source(&value.low),
        high: scalar_source(&value.high),
        decision_interval_millis: value.decision_interval_millis,
        start_probability: value.start_probability,
        mean_duration_millis: value.mean_duration_millis,
        duration_spread_millis: value.duration_spread_millis,
        attack_ratio: value.attack_ratio,
        decay_ratio: value.decay_ratio,
    }
}

fn phase(value: &domain::PhaseDistribution) -> wire::DynamicPhaseDistributionProjection {
    wire::DynamicPhaseDistributionProjection {
        ordering: match value.ordering {
            domain::PhaseOrdering::Selection => wire::DynamicPhaseOrderingProjection::Selection,
            domain::PhaseOrdering::GridLinear { angle_degrees } => {
                wire::DynamicPhaseOrderingProjection::GridLinear { angle_degrees }
            }
            domain::PhaseOrdering::RadialOut { center_x, center_z } => {
                wire::DynamicPhaseOrderingProjection::RadialOut { center_x, center_z }
            }
            domain::PhaseOrdering::RadialIn { center_x, center_z } => {
                wire::DynamicPhaseOrderingProjection::RadialIn { center_x, center_z }
            }
            domain::PhaseOrdering::Axial { center_x, center_z } => {
                wire::DynamicPhaseOrderingProjection::Axial { center_x, center_z }
            }
            domain::PhaseOrdering::RandomEachLoop { seed } => {
                wire::DynamicPhaseOrderingProjection::RandomEachLoop { seed }
            }
        },
        offset_degrees: value.offset_degrees,
        span_degrees: value.span_degrees,
        block_size: value.block_size,
        repeats: value.repeats,
        wings: value.wings,
        anchors_degrees: value.anchors_degrees.clone(),
    }
}

fn speed(value: &domain::DynamicSpeed) -> wire::DynamicSpeedProjection {
    match value {
        domain::DynamicSpeed::Fixed { duration_millis } => wire::DynamicSpeedProjection::Fixed {
            duration_millis: *duration_millis,
        },
        domain::DynamicSpeed::SpeedGroup {
            group,
            beats_per_cycle,
        } => wire::DynamicSpeedProjection::SpeedGroup {
            group: match group {
                domain::SpeedGroup::A => wire::DynamicSpeedGroupProjection::A,
                domain::SpeedGroup::B => wire::DynamicSpeedGroupProjection::B,
                domain::SpeedGroup::C => wire::DynamicSpeedGroupProjection::C,
                domain::SpeedGroup::D => wire::DynamicSpeedGroupProjection::D,
                domain::SpeedGroup::E => wire::DynamicSpeedGroupProjection::E,
            },
            beats_per_cycle: rational(*beats_per_cycle),
        },
    }
}

fn activation(value: domain::ActivationPolicy) -> wire::DynamicActivationPolicyProjection {
    match value {
        domain::ActivationPolicy::StartNow => wire::DynamicActivationPolicyProjection::StartNow,
        domain::ActivationPolicy::JoinSyncNow => {
            wire::DynamicActivationPolicyProjection::JoinSyncNow
        }
        domain::ActivationPolicy::NextBoundary => {
            wire::DynamicActivationPolicyProjection::NextBoundary
        }
    }
}

fn reference(value: &domain::DynamicReference) -> wire::DynamicReferenceProjection {
    wire::DynamicReferenceProjection {
        dynamic_id: value.dynamic_id,
        last_known_pool_number: value.last_known_pool_number,
        embedded_fallback_id: value.embedded_fallback.definition.id,
        embedded_fallback_revision: value.embedded_fallback.definition.revision,
        embedded_fallback: None,
    }
}

fn timing_projection(value: domain::DynamicValueTiming) -> wire::DynamicValueTimingProjection {
    wire::DynamicValueTimingProjection {
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

fn overrides_projection(
    value: &domain::DynamicInstanceOverrides,
) -> wire::DynamicInstanceOverridesProjection {
    wire::DynamicInstanceOverridesProjection {
        size: value.size,
        speed_multiplier: rational(value.speed_multiplier),
        phase_offset_degrees: value.phase_offset_degrees,
    }
}

fn rational(value: domain::Rational) -> wire::DynamicRationalProjection {
    wire::DynamicRationalProjection {
        numerator: value.numerator,
        denominator: value.denominator,
    }
}
