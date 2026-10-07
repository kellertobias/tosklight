use super::*;
use crate::*;
use light_core::{
    FixtureId,
    programming::{IntentError, NativeColorEditModel, TransitionError},
};
use std::{cell::Cell, collections::HashMap, sync::Arc};

pub struct ProgrammingEvaluationContext<'a> {
    pub instance_id: uuid::Uuid,
    pub controller_id: uuid::Uuid,
    pub authored_occurrence: Option<DynamicSourceOccurrenceId>,
    pub target: FixtureId,
    pub elapsed_millis: u64,
    pub cycle_duration_millis: u64,
    pub phase_degrees: f32,
    /// The existing instance-owned Random stream supplies one envelope per group/target.
    pub random_envelope: Option<f32>,
    pub sources: &'a dyn DynamicValueSourceResolver,
}

#[derive(Clone)]
enum Configuration {
    Keyframes(KeyframeConfiguration<CompiledDynamicValueSource>),
    MaxMin(MaxMinConfiguration<CompiledDynamicValueSource>),
    MiddleAmplitude(MiddleAmplitudeConfiguration<CompiledDynamicValueSource, DynamicValue>),
    Random {
        low: CompiledDynamicValueSource,
        high: CompiledDynamicValueSource,
    },
}

/// Built on definition/source-model changes, retained while the instance clock continues.
/// Cache only keyframe endpoint compilation, never the moving target or pre-Dynamic Current.
#[derive(Clone)]
pub struct CompiledProgrammingLane {
    unavailable: Option<crate::NativeColorModelUnavailable>,
    address: CompiledDynamicValueAddress,
    expression_address: Arc<DynamicValueAddress>,
    configuration: Configuration,
    speed_multiplier: Rational,
    width: f32,
    transitions: HashMap<(uuid::Uuid, FixtureId, usize), CachedTransition>,
}

/// A keyframe transition compiled by [`CompiledProgrammingLane::sample_with_cache`], for
/// [`CompiledProgrammingLane::keep_transition`].
pub(crate) struct KeyframeTransition {
    key: (uuid::Uuid, FixtureId, usize),
    transition: CachedTransition,
}

#[derive(Clone)]
struct CachedTransition {
    compiled: CompiledDynamicValueTransition,
    from: Arc<DynamicSampleExpression>,
    to: Arc<DynamicSampleExpression>,
    authored_occurrence: Option<DynamicSourceOccurrenceId>,
    from_dependency: Option<crate::DynamicSourceDependency>,
    to_dependency: Option<crate::DynamicSourceDependency>,
}

impl CompiledProgrammingLane {
    pub fn new(
        lane: &DynamicLane,
        groups: &[DynamicRandomGroup],
        native_model: Option<Arc<dyn NativeColorEditModel + Send + Sync>>,
    ) -> Result<Self, IntentError> {
        Self::compile(lane, groups, native_model, None)
    }

    pub(crate) fn suspended(
        lane: &DynamicLane,
        groups: &[DynamicRandomGroup],
        unavailable: crate::NativeColorModelUnavailable,
    ) -> Result<Self, IntentError> {
        Self::compile(lane, groups, None, Some(unavailable))
    }

    fn compile(
        lane: &DynamicLane,
        groups: &[DynamicRandomGroup],
        native_model: Option<Arc<dyn NativeColorEditModel + Send + Sync>>,
        unavailable: Option<crate::NativeColorModelUnavailable>,
    ) -> Result<Self, IntentError> {
        let DynamicLaneBody::Programming(body) = &lane.body else {
            return Err(IntentError(
                "typed compiler requires a programming lane".into(),
            ));
        };
        body.validate().map_err(|e| IntentError(e.to_string()))?;
        address::ensure(
            lane.speed_multiplier.numerator > 0
                && lane.speed_multiplier.denominator > 0
                && lane.width.is_finite()
                && lane.width >= 0.0,
            "Dynamic timing is invalid",
        )?;
        let address = if unavailable.is_some() {
            CompiledDynamicValueAddress::suspended_native(body.address.clone())?
        } else {
            CompiledDynamicValueAddress::new(body.address.clone(), native_model)?
        };
        let source = |s: &DynamicValueSource, slot| {
            s.compile(&address).map(|source| {
                source.with_occurrence(DynamicPresetSourceOccurrence {
                    lane_id: lane.id,
                    slot,
                })
            })
        };
        use DynamicPresetSourceSlot as Slot;
        let configuration = match &body.configuration {
            ProgrammingLaneConfiguration::Keyframes(c) => {
                Configuration::Keyframes(KeyframeConfiguration {
                    points: c
                        .points
                        .iter()
                        .enumerate()
                        .map(|(index, p)| {
                            Ok(DynamicKeyframe {
                                position: p.position,
                                source: source(
                                    &p.source,
                                    Slot::Keyframe {
                                        index: index as u32,
                                        position_bits: p.position.to_bits(),
                                    },
                                )?,
                                interpolation: p.interpolation,
                            })
                        })
                        .collect::<Result<_, IntentError>>()?,
                    size: c.size,
                })
            }
            ProgrammingLaneConfiguration::MaxMin(c) => Configuration::MaxMin(MaxMinConfiguration {
                minimum: source(&c.minimum, Slot::Minimum)?,
                maximum: source(&c.maximum, Slot::Maximum)?,
                function: c.function,
                size: c.size,
                pwm: c.pwm,
            }),
            ProgrammingLaneConfiguration::MiddleAmplitude(c) => {
                Configuration::MiddleAmplitude(MiddleAmplitudeConfiguration {
                    middle: source(&c.middle, Slot::Middle)?,
                    amplitude: c.amplitude.clone(),
                    function: c.function,
                    size: c.size,
                    pwm: c.pwm,
                    invert_waveform: c.invert_waveform,
                })
            }
            ProgrammingLaneConfiguration::Random => {
                let range = groups
                    .iter()
                    .find(|g| Some(g.id) == lane.random_group_id)
                    .map(|g| &g.range);
                let Some(DynamicRandomRange::Programming { low, high }) = range else {
                    return Err(IntentError(
                        "typed Random lane requires a typed shared group".into(),
                    ));
                };
                Configuration::Random {
                    low: source(low, Slot::RandomLow)?,
                    high: source(high, Slot::RandomHigh)?,
                }
            }
        };
        Ok(Self {
            unavailable,
            expression_address: Arc::new(body.address.clone()),
            address,
            configuration,
            speed_multiplier: lane.speed_multiplier,
            width: lane.width,
            transitions: HashMap::new(),
        })
    }

    pub fn address(&self) -> &CompiledDynamicValueAddress {
        &self.address
    }

    pub fn unavailable_native_source(&self) -> Option<&crate::NativeColorModelUnavailable> {
        self.unavailable.as_ref()
    }

    /// Collapse component resumes so repeated interruptions do not grow expression trees.
    /// Whole-family/live conversions are captured by the coherent frame compositor.
    pub(crate) fn blend_components(
        &self,
        from: &DynamicSampleExpression,
        to: &DynamicSampleExpression,
        progress: f32,
    ) -> Option<DynamicSampleExpression> {
        if self.address.address().component.is_none() {
            return None;
        }
        let (a, from) = from.programming_leaf()?;
        let (b, to) = to.programming_leaf()?;
        if a != b || a != self.address.address() {
            return None;
        }
        self.address
            .transition(from.clone(), to.clone())
            .ok()?
            .sample(progress)
            .ok()
            .map(|value| DynamicSampleExpression::Programming {
                address: Arc::new(a.clone()),
                value,
                occurrence: None,
                dependency_occurrence: None,
            })
    }

    pub fn retain_targets(&mut self, targets: &std::collections::HashSet<FixtureId>) {
        self.transitions
            .retain(|(_, target, _), _| targets.contains(target));
    }

    pub(crate) fn visit_preset_sources(
        &self,
        mut visitor: impl FnMut(&Arc<DynamicPresetSourceBinding>),
    ) {
        let mut visit = |source: &CompiledDynamicValueSource| {
            if let Some(binding) = source.preset_binding() {
                visitor(binding);
            }
        };
        match &self.configuration {
            Configuration::Keyframes(config) => {
                for point in &config.points {
                    visit(&point.source);
                }
            }
            Configuration::MaxMin(config) => {
                visit(&config.minimum);
                visit(&config.maximum);
            }
            Configuration::MiddleAmplitude(config) => visit(&config.middle),
            Configuration::Random { low, high } => {
                visit(low);
                visit(high);
            }
        }
    }

    pub fn apply_controller_size(
        &self,
        value: DynamicSampleExpression,
        size: f32,
        target: FixtureId,
        sources: &dyn DynamicValueSourceResolver,
    ) -> Result<DynamicSampleExpression, IntentError> {
        self.apply_controller_size_with_operations(value, size, target, sources, None)
    }

    /// The sampler's Size site. A whole-family `Scale` receives its genuine emission origin;
    /// a component Size is normalized into a value and has no operation node to attribute.
    pub(crate) fn apply_controller_size_with_operations(
        &self,
        value: DynamicSampleExpression,
        size: f32,
        target: FixtureId,
        sources: &dyn DynamicValueSourceResolver,
        operation: Option<DynamicOperationContext<'_>>,
    ) -> Result<DynamicSampleExpression, IntentError> {
        address::ensure(
            size.is_finite() && size >= 0.0,
            "Dynamic Size must be finite and nonnegative",
        )?;
        if size == 1.0 {
            return Ok(value);
        }
        if self.address.address().component.is_none() {
            let Some(base) = sources.current_family_base(target, self.address.address()) else {
                return Ok(value);
            };
            let scale = DynamicSampleExpression::Scale {
                address: Arc::clone(&self.expression_address),
                base: DynamicValue::Family(base),
                value: Arc::new(value),
                factor: size,
                baseline_occurrence: sources
                    .current_family_occurrence(target, self.address.address()),
            };
            Ok(annotate(
                scale,
                operation,
                DynamicOperationSite::ControllerSize {
                    role: DynamicControllerSizeRole::FamilyScale,
                },
            ))
        } else if let DynamicSampleExpression::Programming {
            address,
            value,
            occurrence,
            dependency_occurrence,
        } = value
        {
            // The coherent frame supplies any representation adoption before a
            // component Current is read; None must mean genuinely unavailable.
            let Some(base) = sources.current(target, self.address.address()) else {
                return Ok(DynamicSampleExpression::Programming {
                    address,
                    value,
                    occurrence,
                    dependency_occurrence,
                });
            };
            Ok(DynamicSampleExpression::Programming {
                address,
                value: self.address.scale_from(&base, &value, size)?,
                occurrence,
                dependency_occurrence: dependency_occurrence
                    .or_else(|| Some(sources.current_dependency(target, self.address.address()))),
            })
        } else {
            Err(IntentError(
                "component Dynamic produced a whole-family expression".into(),
            ))
        }
    }

    fn evaluation_position(&self, context: &ProgrammingEvaluationContext<'_>) -> f32 {
        let function = match &self.configuration {
            Configuration::MaxMin(c) => Some(c.function),
            Configuration::MiddleAmplitude(c) => Some(c.function),
            _ => None,
        };
        crate::evaluate::lane_position(
            self.speed_multiplier,
            self.width,
            function == Some(PeriodicFunction::Pwm),
            context.elapsed_millis,
            context.cycle_duration_millis,
            context.phase_degrees,
        )
    }

    /// Pin scalar Angle arithmetic without reading an adopted root-fixture Current. Only the
    /// selected sources and Size pivots participate; legacy scalar-only readers keep `sample`.
    pub fn pin_angle_numeric(
        &self,
        context: &ProgrammingEvaluationContext<'_>,
        controller_size: f32,
    ) -> Result<AngleNumericSample, TransitionError> {
        self.pin_angle_numeric_with_operations(context, controller_size, None)
    }

    /// The optimized Position producer. Its keyframe `Transition` and controller `ScaleFrom`
    /// receive the same genuine emission origins as the ordinary Required/Size sites.
    pub(crate) fn pin_angle_numeric_with_operations(
        &self,
        context: &ProgrammingEvaluationContext<'_>,
        controller_size: f32,
        operation: Option<DynamicOperationContext<'_>>,
    ) -> Result<AngleNumericSample, TransitionError> {
        let address = self.address.address();
        if address.representation != DynamicFamilyRepresentation::Angles
            || !matches!(
                address.component,
                Some(
                    light_core::programming::ProgrammingComponent::Pan
                        | light_core::programming::ProgrammingComponent::Tilt
                )
            )
        {
            return Ok(AngleNumericSample::NotApplicable);
        }
        address::ensure(
            controller_size.is_finite() && controller_size >= 0.,
            "Dynamic Size must be finite and nonnegative",
        )?;
        let position = self.evaluation_position(context);
        if !self.angle_numeric_needs_current(position, controller_size) {
            return Ok(AngleNumericSample::NotApplicable);
        }
        let Some(original) = context
            .sources
            .try_position_current_family(context.target, address)?
        else {
            return Ok(AngleNumericSample::NotApplicable);
        };
        original.validate_programming_address(
            light_core::programming::ProgrammingOwner::Position.key_ref(),
        )?;
        address::ensure(
            matches!(original, light_core::AttributeValue::Position(_)),
            "numeric Angle Current must be a complete Position family",
        )?;
        let mut build = AngleNumericBuilder::default();
        let mut operations = AngleNumericOperationOrigins::default();
        let Some(mut root) = self.build_angle_numeric_root(
            context,
            position,
            operation,
            &mut build,
            &mut operations,
        ) else {
            return Ok(AngleNumericSample::Absent);
        };
        if controller_size != 1. {
            let pivot = build.current();
            root = build.push(AngleNumericNode::ScaleFrom {
                pivot,
                value: root,
                factor: controller_size,
            });
            if let Some(operation) = operation {
                operations.0[1] = Some((
                    root,
                    operation.origin(DynamicOperationSite::ControllerSize {
                        role: DynamicControllerSizeRole::AngleNumericScaleFrom,
                    }),
                ));
            }
        }
        let program = AngleNumericProgram {
            address: address.clone(),
            occurrence: context.authored_occurrence,
            nodes: build.nodes,
            root,
            operations,
        };
        program.validate()?;
        Ok(AngleNumericSample::Program(Arc::new(program)))
    }

    /// Whether numeric Angle arithmetic must read the adopted Position Current at `position`.
    fn angle_numeric_needs_current(&self, position: f32, controller_size: f32) -> bool {
        controller_size != 1.
            || match &self.configuration {
                Configuration::Keyframes(c) => {
                    let (index, _) = keyframe_segment(&c.points, position);
                    c.points[index].source.is_current()
                        || c.points
                            .get(index + 1)
                            .unwrap_or(&c.points[0])
                            .source
                            .is_current()
                        || (c.size != 1. && c.points[0].source.is_current())
                }
                Configuration::MaxMin(c) => c.minimum.is_current() || c.maximum.is_current(),
                Configuration::MiddleAmplitude(c) => c.middle.is_current(),
                Configuration::Random { low, high } => low.is_current() || high.is_current(),
            }
    }

    /// Build the configuration's numeric Angle root, or `None` when a source is absent. The
    /// keyframe transition receives the emission origin in `operations` slot 0.
    fn build_angle_numeric_root(
        &self,
        context: &ProgrammingEvaluationContext<'_>,
        position: f32,
        operation: Option<DynamicOperationContext<'_>>,
        build: &mut AngleNumericBuilder,
        operations: &mut AngleNumericOperationOrigins,
    ) -> Option<u32> {
        Some(match &self.configuration {
            Configuration::Keyframes(c) => {
                let (index, progress) = keyframe_segment(&c.points, position);
                let left = &c.points[index];
                let right = c.points.get(index + 1).unwrap_or(&c.points[0]);
                let (Some(from), Some(to)) = (
                    build.source(&left.source, context),
                    build.source(&right.source, context),
                ) else {
                    return None;
                };
                let value = build.push(AngleNumericNode::Transition { from, to, progress });
                if let Some(operation) = operation {
                    operations.0[0] = Some((
                        value,
                        operation.origin(DynamicOperationSite::KeyframeTransition {
                            segment_index: index as u32,
                        }),
                    ));
                }
                if c.size == 1. {
                    value
                } else {
                    let Some(pivot) = build.source(&c.points[0].source, context) else {
                        return None;
                    };
                    build.push(AngleNumericNode::ScaleFrom {
                        pivot,
                        value,
                        factor: c.size,
                    })
                }
            }
            Configuration::MaxMin(c) => {
                let (Some(low), Some(high)) = (
                    build.source(&c.minimum, context),
                    build.source(&c.maximum, context),
                ) else {
                    return None;
                };
                build.push(AngleNumericNode::WaveBetween {
                    low,
                    high,
                    amount: crate::evaluate::periodic(c.function, position, c.pwm),
                    size: c.size,
                })
            }
            Configuration::MiddleAmplitude(c) => {
                let Some(middle) = build.source(&c.middle, context) else {
                    return None;
                };
                let amount =
                    (f64::from(crate::evaluate::periodic(c.function, position, c.pwm)) * 2. - 1.)
                        * f64::from(c.size);
                build.push(AngleNumericNode::Around {
                    middle,
                    amplitude: c.amplitude.clone(),
                    amount: if c.invert_waveform { -amount } else { amount },
                })
            }
            Configuration::Random { low, high } => {
                let Some(amount) = context.random_envelope else {
                    return None;
                };
                let (Some(low), Some(high)) =
                    (build.source(low, context), build.source(high, context))
                else {
                    return None;
                };
                build.push(AngleNumericNode::WaveBetween {
                    low,
                    high,
                    amount: amount.clamp(0., 1.),
                    size: 1.,
                })
            }
        })
    }

    pub fn sample(
        &mut self,
        context: ProgrammingEvaluationContext<'_>,
    ) -> Result<Option<DynamicSampleExpression>, TransitionError> {
        self.sample_with_operations(context, None)
    }

    /// The sampler's Required keyframe site. The cached endpoints remain shared across frames;
    /// only the emitted transition receives this emission's origin.
    pub(crate) fn sample_with_operations(
        &mut self,
        context: ProgrammingEvaluationContext<'_>,
        operation: Option<DynamicOperationContext<'_>>,
    ) -> Result<Option<DynamicSampleExpression>, TransitionError> {
        let mut compiled = None;
        let sampled = self.sample_with_cache(context, operation, &mut compiled);
        self.keep_transition(compiled);
        sampled
    }

    /// Keep a keyframe transition [`Self::sample_with_cache`] compiled.
    pub(crate) fn keep_transition(&mut self, compiled: Option<KeyframeTransition>) {
        if let Some(KeyframeTransition { key, transition }) = compiled {
            self.transitions.insert(key, transition);
        }
    }

    /// [`Self::sample_with_operations`] without touching the lane (TL-639 round 6): a keyframe
    /// transition the cache lacks, or holds for other endpoints, is compiled into `compiled`
    /// for the caller to keep (in frame order) instead of being inserted here.
    pub(crate) fn sample_with_cache(
        &self,
        context: ProgrammingEvaluationContext<'_>,
        operation: Option<DynamicOperationContext<'_>>,
        compiled: &mut Option<KeyframeTransition>,
    ) -> Result<Option<DynamicSampleExpression>, TransitionError> {
        let position = self.evaluation_position(&context);
        let current_used = Cell::new(false);
        let resolve = |source: &CompiledDynamicValueSource| {
            if source.is_current() {
                current_used.set(true);
            }
            source.resolve(context.instance_id, context.target, context.sources)
        };
        let value = match &self.configuration {
            Configuration::Keyframes(c) => {
                let (index, progress) = keyframe_segment(&c.points, position);
                let left = &c.points[index];
                let right = c.points.get(index + 1).unwrap_or(&c.points[0]);
                let (Some(from), Some(to)) = (resolve(&left.source), resolve(&right.source)) else {
                    return Ok(None);
                };
                let dependency =
                    (left.source.is_current() || right.source.is_current()).then(|| {
                        context
                            .sources
                            .current_dependency(context.target, self.address.address())
                    });
                let from_dependency = left
                    .source
                    .is_current()
                    .then(|| dependency.clone())
                    .flatten();
                let to_dependency = right.source.is_current().then_some(dependency).flatten();
                let key = (context.controller_id, context.target, index);
                let cached = self.transitions.get(&key).filter(|t| {
                    t.compiled.endpoints() == (&from, &to)
                        && t.authored_occurrence == context.authored_occurrence
                        && t.from_dependency == from_dependency
                        && t.to_dependency == to_dependency
                });
                if cached.is_none() {
                    *compiled = Some(KeyframeTransition {
                        key,
                        transition: CachedTransition {
                            from: Arc::new(DynamicSampleExpression::Programming {
                                address: Arc::clone(&self.expression_address),
                                value: from.clone(),
                                occurrence: context.authored_occurrence,
                                dependency_occurrence: from_dependency.clone(),
                            }),
                            to: Arc::new(DynamicSampleExpression::Programming {
                                address: Arc::clone(&self.expression_address),
                                value: to.clone(),
                                occurrence: context.authored_occurrence,
                                dependency_occurrence: to_dependency.clone(),
                            }),
                            compiled: self.address.transition(from, to)?,
                            authored_occurrence: context.authored_occurrence,
                            from_dependency,
                            to_dependency,
                        },
                    });
                }
                let transition = match cached {
                    Some(transition) => transition,
                    None => &compiled.as_ref().expect("compiled above").transition,
                };
                let value = match transition.compiled.sample(progress) {
                    Ok(value) => value,
                    Err(TransitionError::Requires(requirement)) => {
                        return Ok(Some(annotate(
                            DynamicSampleExpression::Transition {
                                from: Some(Arc::clone(&transition.from)),
                                to: Some(Arc::clone(&transition.to)),
                                progress,
                                reason: DynamicTransitionReason::Required { requirement },
                            },
                            operation,
                            DynamicOperationSite::KeyframeTransition {
                                segment_index: index as u32,
                            },
                        )));
                    }
                    Err(error) => return Err(error),
                };
                if c.size == 1.0 {
                    value
                } else {
                    let Some(pivot) = resolve(&c.points[0].source) else {
                        return Ok(None);
                    };
                    self.address.scale_from(&pivot, &value, c.size)?
                }
            }
            Configuration::MaxMin(c) => {
                let (Some(low), Some(high)) = (resolve(&c.minimum), resolve(&c.maximum)) else {
                    return Ok(None);
                };
                self.address.wave_between(
                    &low,
                    &high,
                    crate::evaluate::periodic(c.function, position, c.pwm),
                    c.size,
                )?
            }
            Configuration::MiddleAmplitude(c) => {
                let Some(middle) = resolve(&c.middle) else {
                    return Ok(None);
                };
                let amount =
                    (f64::from(crate::evaluate::periodic(c.function, position, c.pwm)) * 2.0 - 1.0)
                        * f64::from(c.size);
                self.address.around_wide(
                    &middle,
                    &c.amplitude,
                    if c.invert_waveform { -amount } else { amount },
                )?
            }
            Configuration::Random { low, high } => {
                let (Some(low), Some(high), Some(amount)) =
                    (resolve(low), resolve(high), context.random_envelope)
                else {
                    return Ok(None);
                };
                self.address
                    .wave_between(&low, &high, amount.clamp(0.0, 1.0), 1.0)?
            }
        };
        Ok(Some(DynamicSampleExpression::Programming {
            address: Arc::clone(&self.expression_address),
            value,
            occurrence: context.authored_occurrence,
            dependency_occurrence: current_used.get().then(|| {
                context
                    .sources
                    .current_dependency(context.target, self.address.address())
            }),
        }))
    }
}

fn annotate(
    expression: DynamicSampleExpression,
    operation: Option<DynamicOperationContext<'_>>,
    site: DynamicOperationSite,
) -> DynamicSampleExpression {
    match operation {
        Some(operation) => DynamicSampleExpression::Operation {
            origin: Some(operation.origin(site)),
            value: Arc::new(expression),
        },
        None => expression,
    }
}

/// Shared with legacy sampling so pinning uses exactly the same keyframe/easing coefficients.
fn keyframe_segment<T>(points: &[DynamicKeyframe<T>], position: f32) -> (usize, f32) {
    let index = points
        .iter()
        .rposition(|p| p.position <= position)
        .unwrap_or(0);
    let left = &points[index];
    let right_position = points.get(index + 1).map_or(1., |p| p.position);
    let progress = crate::evaluate::interpolate(
        ((position - left.position) / (right_position - left.position).max(f32::EPSILON))
            .clamp(0., 1.),
        left.interpolation,
    );
    (index, progress)
}

#[derive(Default)]
struct AngleNumericBuilder {
    nodes: Vec<AngleNumericNode>,
    current: Option<u32>,
}
impl AngleNumericBuilder {
    fn push(&mut self, node: AngleNumericNode) -> u32 {
        let index = self.nodes.len() as u32;
        self.nodes.push(node);
        index
    }
    fn current(&mut self) -> u32 {
        if let Some(index) = self.current {
            return index;
        }
        let index = self.push(AngleNumericNode::Current);
        self.current = Some(index);
        index
    }
    fn source(
        &mut self,
        source: &CompiledDynamicValueSource,
        context: &ProgrammingEvaluationContext<'_>,
    ) -> Option<u32> {
        if source.is_current() {
            return Some(self.current());
        }
        source
            .resolve(context.instance_id, context.target, context.sources)
            .map(|value| {
                self.push(AngleNumericNode::Materialized {
                    value,
                    dependency_occurrence: None,
                })
            })
    }
}
