use super::{DynamicFamilyRepresentation, DynamicValueAddress, address::ensure};
use light_core::{AttributeValue, programming::*};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DynamicValue {
    Scalar(f32),
    Native(u32),
    Family(AttributeValue),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Domain {
    Scalar {
        domain: ScalarDomain,
        interpolation: ScalarInterpolation,
    },
    Native(NativeColorComponentDescriptor),
    /// A verified discrete FixAT value. Only complete-recipe composition may step this
    /// channel/function; waveform arithmetic and numeric interpolation remain forbidden.
    FixedNative(NativeColorComponentDescriptor),
    /// Storage shape only; there are no verified bounds and numeric evaluation is suspended.
    UnverifiedNative,
    Family,
}

/// Cold-compiled lane address. Native function bounds are derived from the verified pinned
/// source, never accepted as arbitrary serialized per-lane limits.
#[derive(Clone)]
pub struct CompiledDynamicValueAddress {
    address: DynamicValueAddress,
    domain: Domain,
    native_model: Option<Arc<dyn NativeColorEditModel + Send + Sync>>,
    unverified_native: bool,
}

impl CompiledDynamicValueAddress {
    pub fn new(
        address: DynamicValueAddress,
        native_model: Option<Arc<dyn NativeColorEditModel + Send + Sync>>,
    ) -> Result<Self, IntentError> {
        address.validate()?;
        if let DynamicFamilyRepresentation::DirectColor { source } = &address.representation {
            ensure(
                native_model
                    .as_ref()
                    .is_some_and(|model| model.source() == source),
                "Dynamic Direct source requires its exact verified native model",
            )?;
        }
        let domain = match address.component {
            None => Domain::Family,
            Some(ProgrammingComponent::NativeColor(binding)) => {
                let descriptor = native_model
                    .as_ref()
                    .and_then(|model| model.descriptor(binding))
                    .ok_or_else(|| {
                        IntentError(
                            "native Dynamic function is absent from the pinned source".into(),
                        )
                    })?;
                ensure(
                    descriptor.binding == binding && descriptor.continuous,
                    "native Dynamic requires a verified continuous function",
                )?;
                Domain::Native(descriptor)
            }
            Some(component) => {
                let descriptor = component.descriptor();
                ensure(
                    descriptor.dynamics,
                    "discrete controls are not continuous Dynamic lanes",
                )?;
                Domain::Scalar {
                    domain: descriptor
                        .domain
                        .expect("continuous semantic component has a domain"),
                    interpolation: descriptor.interpolation,
                }
            }
        };
        Ok(Self {
            address,
            domain,
            native_model,
            unverified_native: false,
        })
    }

    /// FixAT can own a discrete native channel. Keep a distinct domain so accepting that
    /// stored value cannot accidentally enable continuous Dynamic arithmetic for wheels.
    pub(super) fn new_fixed_component(
        address: DynamicValueAddress,
        native_model: Option<Arc<dyn NativeColorEditModel + Send + Sync>>,
    ) -> Result<Self, IntentError> {
        let Some(ProgrammingComponent::NativeColor(binding)) = address.component else {
            return Self::new(address, native_model);
        };
        let Some(model) = native_model.as_ref() else {
            return Self::new(address, native_model);
        };
        let Some(descriptor) = model.descriptor(binding) else {
            return Self::new(address, native_model);
        };
        if descriptor.continuous {
            return Self::new(address, native_model);
        }
        address.validate()?;
        ensure(
            descriptor.binding == binding,
            "fixed native descriptor differs from its binding",
        )?;
        ensure(
            matches!(&address.representation, DynamicFamilyRepresentation::DirectColor { source } if model.source() == source),
            "fixed native source requires its exact verified model",
        )?;
        Ok(Self {
            address,
            domain: Domain::FixedNative(descriptor),
            native_model,
            unverified_native: false,
        })
    }

    pub(crate) fn suspended_native(address: DynamicValueAddress) -> Result<Self, IntentError> {
        address.validate()?;
        ensure(
            matches!(
                address.representation,
                DynamicFamilyRepresentation::DirectColor { .. }
            ),
            "only an unavailable original native source can suspend native validation",
        )?;
        let domain = if address.component.is_some() {
            Domain::UnverifiedNative
        } else {
            Domain::Family
        };
        Ok(Self {
            address,
            domain,
            native_model: None,
            unverified_native: true,
        })
    }

    pub fn address(&self) -> &DynamicValueAddress {
        &self.address
    }

    pub(super) fn native_model(&self) -> Option<Arc<dyn NativeColorEditModel + Send + Sync>> {
        self.native_model.clone()
    }

    pub fn compatible_random_domain(&self, other: &Self) -> bool {
        match (self.domain, other.domain) {
            (Domain::Scalar { domain: a, .. }, Domain::Scalar { domain: b, .. }) => {
                a == b
                    && self.address.component.map(|c| c.descriptor().unit)
                        == other.address.component.map(|c| c.descriptor().unit)
            }
            (Domain::Native(a), Domain::Native(b)) => {
                self.address.representation == other.address.representation
                    && a.raw_from.min(a.raw_to) == b.raw_from.min(b.raw_to)
                    && a.raw_from.max(a.raw_to) == b.raw_from.max(b.raw_to)
            }
            _ => false,
        }
    }

    /// A waveform's range/size is numeric arithmetic in the declared units. It intentionally
    /// differs from keyframe shortest-arc Hue and reciprocal-temperature interpolation.
    pub fn wave_between(
        &self,
        low: &DynamicValue,
        high: &DynamicValue,
        amount: f32,
        size: f32,
    ) -> Result<DynamicValue, IntentError> {
        ensure(
            amount.is_finite() && size.is_finite() && size >= 0.0,
            "Dynamic waveform and size must be finite",
        )?;
        self.validate_value(low)?;
        self.validate_value(high)?;
        let factor = 0.5 + (f64::from(amount) - 0.5) * f64::from(size);
        match (self.domain, low, high) {
            (
                Domain::Scalar { domain, .. },
                DynamicValue::Scalar(low),
                DynamicValue::Scalar(high),
            ) => Ok(DynamicValue::Scalar(constrain_wide(
                domain,
                f64::from(*low) + (f64::from(*high) - f64::from(*low)) * factor,
            )?)),
            (Domain::Native(descriptor), DynamicValue::Native(low), DynamicValue::Native(high)) => {
                Ok(DynamicValue::Native(scale_native_delta_wide(
                    *low,
                    i64::from(*high) - i64::from(*low),
                    factor,
                    descriptor.raw_from.min(descriptor.raw_to),
                    descriptor.raw_from.max(descriptor.raw_to),
                )?))
            }
            _ => Err(IntentError(
                "whole-family keyframes do not have a numeric waveform range".into(),
            )),
        }
    }

    pub fn validate_value(&self, value: &DynamicValue) -> Result<(), IntentError> {
        match (self.domain, value) {
            (Domain::Scalar { domain, .. }, DynamicValue::Scalar(value)) => ensure(
                domain.contains(*value),
                "Dynamic scalar is outside the declared component domain",
            ),
            (
                Domain::Native(descriptor) | Domain::FixedNative(descriptor),
                DynamicValue::Native(value),
            ) => ensure(
                (descriptor.raw_from.min(descriptor.raw_to)
                    ..=descriptor.raw_from.max(descriptor.raw_to))
                    .contains(value),
                "Dynamic native value is outside its pinned function",
            ),
            (Domain::UnverifiedNative, DynamicValue::Native(_)) => Ok(()),
            (Domain::Family, DynamicValue::Family(value)) => self.address.validate_family(value),
            _ => Err(IntentError(
                "Dynamic source has the wrong scalar/native/family value type".into(),
            )),
        }
    }

    /// Definition/fallback/recovery boundary only. A whole Direct recipe must own
    /// exactly the source model's controls, including any discrete wheel functions.
    pub fn validate_source_value(&self, value: &DynamicValue) -> Result<(), IntentError> {
        self.validate_value(value)?;
        if self.unverified_native {
            // Structural whole-family/identity validation above remains mandatory. Native
            // completeness/function bounds are deferred until the exact original is available.
            return Ok(());
        }
        if let DynamicValue::Family(AttributeValue::ColorProgram(color)) = value
            && let ColorProgram::Direct { recipe, .. } = color.as_ref()
        {
            self.native_model
                .as_ref()
                .ok_or_else(|| IntentError("Direct source model is unavailable".into()))?
                .predict(recipe)?
                .validate()?;
        }
        Ok(())
    }

    /// Numeric waveform/size arithmetic is in the authored component's units. Unlike keyframe
    /// interpolation, this can span a whole Hue revolution or amplify beyond endpoint values.
    /// Only an explicitly bounded component/function clamps; signed degrees/metres stay signed.
    pub fn scale_from(
        &self,
        pivot: &DynamicValue,
        value: &DynamicValue,
        size: f32,
    ) -> Result<DynamicValue, IntentError> {
        ensure(
            size.is_finite() && size >= 0.0,
            "Dynamic size must be finite and nonnegative",
        )?;
        self.validate_value(pivot)?;
        self.validate_value(value)?;
        match (self.domain, pivot, value) {
            (
                Domain::Scalar { domain, .. },
                DynamicValue::Scalar(pivot),
                DynamicValue::Scalar(value),
            ) => {
                let value =
                    f64::from(*pivot) + (f64::from(*value) - f64::from(*pivot)) * f64::from(size);
                Ok(DynamicValue::Scalar(constrain_wide(domain, value)?))
            }
            (
                Domain::Native(descriptor),
                DynamicValue::Native(pivot),
                DynamicValue::Native(value),
            ) => Ok(DynamicValue::Native(scale_native_delta(
                *pivot,
                i64::from(*value) - i64::from(*pivot),
                size,
                descriptor.raw_from.min(descriptor.raw_to),
                descriptor.raw_from.max(descriptor.raw_to),
            )?)),
            _ => Err(IntentError(
                "whole-family Dynamic size requires its base transition".into(),
            )),
        }
    }

    pub fn around(
        &self,
        middle: &DynamicValue,
        amplitude: &DynamicValue,
        amount: f32,
    ) -> Result<DynamicValue, IntentError> {
        self.around_wide(middle, amplitude, f64::from(amount))
    }

    pub fn around_wide(
        &self,
        middle: &DynamicValue,
        amplitude: &DynamicValue,
        amount: f64,
    ) -> Result<DynamicValue, IntentError> {
        ensure(amount.is_finite(), "Dynamic waveform amount must be finite")?;
        self.validate_value(middle)?;
        match (self.domain, middle, amplitude) {
            (
                Domain::Scalar { domain, .. },
                DynamicValue::Scalar(middle),
                DynamicValue::Scalar(amplitude),
            ) => {
                ensure(
                    amplitude.is_finite() && *amplitude >= 0.0,
                    "Dynamic amplitude must be finite and nonnegative",
                )?;
                Ok(DynamicValue::Scalar(constrain_wide(
                    domain,
                    f64::from(*middle) + f64::from(*amplitude) * amount,
                )?))
            }
            (
                Domain::Native(descriptor),
                DynamicValue::Native(middle),
                DynamicValue::Native(amplitude),
            ) => Ok(DynamicValue::Native(scale_native_delta_wide(
                *middle,
                i64::from(*amplitude),
                amount,
                descriptor.raw_from.min(descriptor.raw_to),
                descriptor.raw_from.max(descriptor.raw_to),
            )?)),
            _ => Err(IntentError(
                "Dynamic amplitude must match its scalar/native component".into(),
            )),
        }
    }

    pub fn transition(
        &self,
        from: DynamicValue,
        to: DynamicValue,
    ) -> Result<CompiledDynamicValueTransition, TransitionError> {
        self.validate_value(&from)?;
        self.validate_value(&to)?;
        ensure(
            !matches!(self.domain, Domain::FixedNative(_)),
            "discrete Fixed channels require complete-recipe step composition",
        )?;
        let family = match (&from, &to) {
            (DynamicValue::Family(from), DynamicValue::Family(to)) => {
                Some(CompiledProgrammingTransition::new(
                    from.clone(),
                    to.clone(),
                    self.native_model.clone(),
                )?)
            }
            _ => None,
        };
        Ok(CompiledDynamicValueTransition {
            domain: self.domain,
            from,
            to,
            family,
        })
    }
}

fn constrain_wide(domain: ScalarDomain, value: f64) -> Result<f32, IntentError> {
    let value = match domain {
        ScalarDomain::Finite => value,
        ScalarDomain::Bounded { bounds } => {
            value.clamp(f64::from(bounds.min), f64::from(bounds.max))
        }
        ScalarDomain::Cyclic { bounds } => {
            f64::from(bounds.min)
                + (value - f64::from(bounds.min))
                    .rem_euclid(f64::from(bounds.max) - f64::from(bounds.min))
        }
    };
    let result = value as f32;
    ensure(
        result.is_finite(),
        "Dynamic arithmetic exceeds its finite physical domain",
    )?;
    Ok(result)
}

#[derive(Clone)]
pub struct CompiledDynamicValueTransition {
    domain: Domain,
    from: DynamicValue,
    to: DynamicValue,
    family: Option<CompiledProgrammingTransition>,
}

impl CompiledDynamicValueTransition {
    pub fn endpoints(&self) -> (&DynamicValue, &DynamicValue) {
        (&self.from, &self.to)
    }

    pub fn sample(&self, progress: f32) -> Result<DynamicValue, TransitionError> {
        ensure(progress.is_finite(), "Dynamic progress must be finite")?;
        if progress <= 0.0 {
            return Ok(self.from.clone());
        }
        if progress >= 1.0 {
            return Ok(self.to.clone());
        }
        match (self.domain, &self.from, &self.to) {
            (
                Domain::Scalar {
                    domain,
                    interpolation,
                },
                DynamicValue::Scalar(from),
                DynamicValue::Scalar(to),
            ) => Ok(DynamicValue::Scalar(domain.constrain(
                interpolate_scalar(*from, *to, progress, interpolation),
            )?)),
            (Domain::Native(descriptor), DynamicValue::Native(from), DynamicValue::Native(to)) => {
                Ok(DynamicValue::Native(scale_native_delta(
                    *from,
                    i64::from(*to) - i64::from(*from),
                    progress,
                    descriptor.raw_from.min(descriptor.raw_to),
                    descriptor.raw_from.max(descriptor.raw_to),
                )?))
            }
            (Domain::Family, ..) => self
                .family
                .as_ref()
                .expect("compiled family transition")
                .sample(progress)
                .map(DynamicValue::Family),
            _ => unreachable!("validated Dynamic endpoints"),
        }
    }
}
