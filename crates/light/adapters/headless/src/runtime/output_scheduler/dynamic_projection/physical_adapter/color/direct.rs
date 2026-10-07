//! TL-559 Direct Color replay inside the Color adapter.
//!
//! One tagged Direct value resolves as ONE complete Color result per destination head:
//! 1. The source estimate is forward-evaluated by the pinned ORIGINAL model for the exact recipe
//!    of this frame (a Direct Dynamic sends a new recipe every tick, so every changed recipe is
//!    evaluated before any fallback fitting; a record-time estimate is never replayed while the
//!    original is available). The cache is keyed by the immutable program and model objects, so
//!    an unchanged value is evaluated once. An available original rejecting the recipe is
//!    invalid data. Without the original, the recorded estimate is valid fallback data.
//! 2. Each head is planned by TL-595 identity alone (`plan_direct_replay`): a verified compatible
//!    layout replays the exact recorded controls; any other head fits the estimate through the
//!    existing fitter. Visible `Fit` uses the shared semantic adoption (total XYZ, never
//!    normalized, leakage never added again); visible `Hold` keeps this head's last accepted
//!    visible writes or declared safe defaults, never white. UV is independent: a known amount
//!    (including zero) is applied, unknown UV is parked off. UV never comes from continuity.
//! 3. Every footprint control is written exactly once, as for semantic values.
use super::resolve::HeadResolution;
use super::*;
use light_core::programming::{
    DirectDestination, DirectFallback, DirectReplay, NativeColorEditModel, NativeColorRecipe,
    UvIntent, plan_direct_replay, semantic_color_adoption,
};
use light_dynamics::{DynamicNativeModelResolver, NativeColorModelCapability};
use std::sync::Weak;

type Model = Arc<dyn NativeColorEditModel + Send + Sync>;

/// The last original-model evaluation of one exact Direct program.
pub(super) struct DirectEstimateCache {
    program: Weak<ColorProgram>,
    model: Weak<dyn NativeColorEditModel + Send + Sync>,
    prediction: light_core::programming::NativeColorPrediction,
}

/// The source estimate one frame uses for every head of the target.
struct SourceEstimate {
    estimate: PortableColorEstimate,
    drive_limit: NativeDriveLimit,
    origin: DirectEstimateOrigin,
    limitations: Vec<String>,
}

fn recipe_of(program: &ColorProgram) -> &NativeColorRecipe {
    match program {
        ColorProgram::Direct { recipe, .. } => recipe,
        ColorProgram::Semantic { .. } => unreachable!("Direct replay requires a Direct value"),
    }
}

impl ColorAdapter {
    pub(super) fn resolve_direct(
        &self,
        request: &PhysicalRequest<'_, Self>,
        program: &Arc<ColorProgram>,
    ) -> Result<PhysicalResolution<Self>, TransitionError> {
        program.validate()?;
        if !recipe_of(program).spreads.is_empty() {
            // Spreads resolve per member before the physical adapter.
            return Err(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ));
        }
        let models = request.frame.native_models;
        let source = {
            let mut scratch = request.descriptor.scratch.lock();
            self.source_estimate(program, models, &mut scratch.direct)?
        };
        let resolved = self.resolve_each_head(request, |adapter, head, current, writes| {
            adapter.resolve_direct_head(head, program, &source, models, request, current, writes)
        })?;
        Ok(Self::publish(
            resolved,
            ColorRequest::Direct(Arc::clone(program)),
        ))
    }

    fn source_estimate(
        &self,
        program: &Arc<ColorProgram>,
        models: &dyn DynamicNativeModelResolver,
        cache: &mut Option<DirectEstimateCache>,
    ) -> Result<SourceEstimate, TransitionError> {
        let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
            unreachable!("Direct replay requires a Direct value")
        };
        let model: Model = match models.resolve_capability(&recipe.source)? {
            NativeColorModelCapability::Available(model) => model,
            NativeColorModelCapability::Unavailable(unavailable) => {
                let mut limitations = portable.limitations.clone();
                limitations.push(format!(
                    "Original native Color model is unavailable ({}); the recorded estimate is used.",
                    unavailable.detail.chars().take(200).collect::<String>()
                ));
                return Ok(SourceEstimate {
                    estimate: portable.clone(),
                    drive_limit: NativeDriveLimit::Unknown,
                    origin: DirectEstimateOrigin::Recorded,
                    limitations,
                });
            }
        };
        if model.source() != &recipe.source {
            return Err(invalid(
                "captured Direct model differs from its original source",
            ));
        }
        let hit = cache.as_ref().filter(|entry| {
            entry.program.ptr_eq(&Arc::downgrade(program))
                && entry.model.ptr_eq(&Arc::downgrade(&model))
        });
        let prediction = match hit {
            Some(entry) => entry.prediction.clone(),
            None => {
                let prediction = model.predict_with_status(recipe)?;
                if prediction.portable.model_revision != recipe.source.model_revision {
                    return Err(invalid(
                        "Direct prediction differs from its pinned source model revision",
                    ));
                }
                prediction.portable.validate()?;
                self.count(|c| c.direct_forward_evaluations += 1);
                *cache = Some(DirectEstimateCache {
                    program: Arc::downgrade(program),
                    model: Arc::downgrade(&model),
                    prediction: prediction.clone(),
                });
                prediction
            }
        };
        Ok(SourceEstimate {
            limitations: prediction.portable.limitations.clone(),
            estimate: prediction.portable,
            drive_limit: prediction.drive_limit,
            origin: DirectEstimateOrigin::Forward,
        })
    }

    /// TL-595 identity-only plan for one destination head. Capability is resolved per frame.
    fn plan_head(
        head: &ColorHeadDescriptor,
        program: &ColorProgram,
        models: &dyn DynamicNativeModelResolver,
    ) -> Result<DirectReplay, TransitionError> {
        let Some(identity) = head.native.as_ref() else {
            let reason = "destination head has no verified native Color identity".to_string();
            return Ok(plan_direct_replay(
                program,
                &DirectDestination::Unverified(reason),
            )?);
        };
        Ok(match models.resolve_capability(identity)? {
            NativeColorModelCapability::Available(model) => {
                plan_direct_replay(program, &DirectDestination::Verified(model.as_ref()))?
            }
            NativeColorModelCapability::Unavailable(unavailable) => {
                plan_direct_replay(program, &DirectDestination::Unverified(unavailable.detail))?
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_direct_head(
        &self,
        head: &ColorHeadDescriptor,
        program: &ColorProgram,
        source: &SourceEstimate,
        models: &dyn DynamicNativeModelResolver,
        request: &PhysicalRequest<'_, Self>,
        current: &mut [u32],
        writes: &mut Vec<NativeControlWrite>,
    ) -> Result<HeadResolution, TransitionError> {
        let mut limitations = source.limitations.clone();
        let (mut resolved, replay) = match Self::plan_head(head, program, models)? {
            DirectReplay::Exact { recipe } => {
                self.count(|c| c.direct_exact += 1);
                (
                    self.replay_exact(
                        head,
                        &recipe,
                        Self::source_visible(&source.estimate),
                        current,
                        writes,
                    )?,
                    DirectReplayOutcome::Exact,
                )
            }
            DirectReplay::Fallback { compatibility, .. } => {
                self.count(|c| c.direct_fallbacks += 1);
                // The fallback derives from THIS frame's forward estimate, not the recorded one.
                let fallback = DirectFallback::from_portable(&source.estimate);
                limitations = fallback.limitations.clone();
                if source.origin == DirectEstimateOrigin::Recorded {
                    limitations.extend(source.limitations.last().cloned());
                }
                let resolved = match fallback.visible {
                    VisibleFallback::Fit(_) => {
                        let adopted = semantic_color_adoption(&source.estimate, None)?;
                        self.resolve_head(head, &adopted.intent, request.previous, current, writes)?
                    }
                    VisibleFallback::Hold => {
                        self.count(|c| c.direct_visible_holds += 1);
                        let uv = match fallback.uv {
                            UvFallback::Apply(uv) => uv.amount,
                            UvFallback::ParkOff => 0.0,
                        };
                        self.hold_visible(head, uv, request.previous, current, writes)?
                    }
                };
                (
                    resolved,
                    DirectReplayOutcome::Fallback {
                        compatibility,
                        visible: fallback.visible,
                        uv: fallback.uv,
                    },
                )
            }
        };
        resolved.quality.direct = Some(DirectColorStatus {
            replay,
            origin: source.origin,
            estimate: source.estimate.clone(),
            drive_limit: source.drive_limit,
            limitations,
        });
        Ok(resolved)
    }

    /// Write the verified recipe unchanged on every control this head writes; achieved output
    /// is the forward evaluation of exactly those values.
    fn replay_exact(
        &self,
        head: &ColorHeadDescriptor,
        recipe: &NativeColorRecipe,
        source_visible: Option<Xyz>,
        current: &mut [u32],
        writes: &mut Vec<NativeControlWrite>,
    ) -> Result<HeadResolution, TransitionError> {
        if let Some(foreign) = recipe
            .channels
            .iter()
            .find(|v| !head.controls.iter().any(|c| c.channel_id == v.channel_id))
        {
            return Err(invalid(format!(
                "verified Direct recipe control {} is not part of the destination head",
                foreign.channel_id
            )));
        }
        let first = writes.len();
        let mut conflict = false;
        for (control, owned) in head.controls.iter().zip(head.writes_control.iter()) {
            let value = recipe
                .channels
                .iter()
                .find(|v| v.channel_id == control.channel_id);
            let index = control.channel_index as usize;
            if !*owned {
                conflict |= value.is_some_and(|v| v.raw != current[index]);
                continue;
            }
            let write = NativeControlWrite {
                slot: NativeControlSlot {
                    destination: head.destination,
                    channel_index: control.channel_index,
                    split: control.split,
                },
                channel_id: control.channel_id,
                function_id: value.map(|v| v.function_id),
                raw: value.map_or(control.park_raw, |v| v.raw),
                parked: value.is_none(),
            };
            current[index] = write.raw;
            writes.push(write);
        }
        let mut scratch = head.scratch.lock();
        let forward = Self::evaluate_forward(head, &mut scratch, current)?;
        self.record(&ColorSolveWork::default(), conflict);
        let mut quality = Self::forward_quality(forward, conflict);
        quality.visible = if forward.visible_complete {
            VisibleFitStatus::Fitted
        } else {
            VisibleFitStatus::PredictionIncomplete
        };
        // Native identity replay is not a chromaticity match: compare the destination's actual
        // forward output with the source estimate, or report Unknown when either is unknown.
        Self::compare_with_source(&mut quality, forward, source_visible);
        let achieved = Self::forward_achieved(forward);
        Self::settle_uv_appearance(head, &mut scratch, current, &[], &mut quality)?;
        Ok(HeadResolution {
            achieved,
            quality,
            continuity: Self::continuity(head, &writes[first..]),
        })
    }

    /// Total source visible output (already includes brightness and leakage), if known.
    fn source_visible(estimate: &PortableColorEstimate) -> Option<Xyz> {
        estimate.visible.map(|v| Xyz {
            x: v.xyz.x * v.relative_output,
            y: v.xyz.y * v.relative_output,
            z: v.xyz.z * v.relative_output,
        })
    }

    /// Compare the destination's actual forward output with the source estimate.
    fn compare_with_source(
        quality: &mut ColorQuality,
        forward: &ColorForwardResult,
        source: Option<Xyz>,
    ) {
        let measured = measure_against_source(forward.known_xyz, forward.visible_complete, source);
        quality.color_match = measured.color_match;
        quality.delta_uv = measured.delta_uv;
        quality.luminance_ratio = measured.luminance_ratio;
        quality.luminance_limited = measured.luminance_limited;
    }

    /// Active UV appearance of the FINAL written values: a zero drive is known; otherwise the
    /// complete total proves it, and an incomplete total is probed with every non-UV control of
    /// this head closed so unknown held visible controls do not hide known UV leakage.
    /// `fitted_uv` lists controls a fitter wrote with the Ultraviolet role.
    fn settle_uv_appearance(
        head: &ColorHeadDescriptor,
        scratch: &mut ColorHeadScratch,
        current: &[u32],
        fitted_uv: &[u32],
        quality: &mut ColorQuality,
    ) -> Result<(), TransitionError> {
        let forward = &scratch.forward[head.head];
        let active = forward.uv_emitters.iter().any(|uv| uv.drive > 0.);
        let known = if !active || forward.visible_complete {
            true
        } else {
            let is_uv = |control: &ColorFitControl| {
                fitted_uv.contains(&control.channel_index)
                    || head
                        .native_controls
                        .iter()
                        .any(|c| c.channel_id == control.channel_id && c.ultraviolet)
            };
            let mut probe = current.to_vec();
            for control in head.controls.iter().filter(|c| !is_uv(c)) {
                probe[control.channel_index as usize] = control.park_raw;
            }
            Self::evaluate_forward(head, scratch, &probe)?.visible_complete
        };
        quality.uv_appearance_known = known;
        if !known {
            quality.limitations.0 |= ColorFitLimitations::UNKNOWN_UV_APPEARANCE.0;
            quality.total_quality = PhysicalDataQuality::Unknown;
        }
        Ok(())
    }

    /// Unknown visible appearance: keep this head's last accepted visible writes (or declared
    /// safe defaults before a first valid solution) and resolve UV independently.
    fn hold_visible(
        &self,
        head: &ColorHeadDescriptor,
        uv: f32,
        previous: Option<&ColorContinuity>,
        current: &mut [u32],
        writes: &mut Vec<NativeControlWrite>,
    ) -> Result<HeadResolution, TransitionError> {
        let mut scratch = head.scratch.lock();
        // UV-only request: the fitter freezes UV first; only its UV writes are used.
        let request = ColorIntent {
            base_xyz: Xyz {
                x: 0.,
                y: 0.,
                z: 0.,
            },
            recipe: light_core::programming::VirtualColorRecipe {
                rgb: [0.; 3],
                ..Default::default()
            },
            relative_output: 0.,
            uv: UvIntent { amount: uv },
            ..ColorIntent::default()
        };
        let mut work = ColorSolveWork::default();
        self.fit(head, &request, current, &mut scratch, &mut work)?;
        let held = previous.and_then(|p| p.head(head.destination, head.head_id));
        let first = writes.len();
        let mut conflict = false;
        let fitted_uv: Vec<u32> = scratch
            .output
            .writes
            .iter()
            .filter(|w| {
                matches!(
                    w.role,
                    light_fixture::forward::ColorWriteRole::Ultraviolet { .. }
                )
            })
            .map(|w| w.channel_index)
            .collect();
        for (control, owned) in head.controls.iter().zip(head.writes_control.iter()) {
            if !*owned {
                // A master-shared UV slot keeps an earlier head's value; a different proposal
                // for it is a shared-control conflict (held visible controls propose nothing).
                conflict |= scratch.output.writes.iter().any(|w| {
                    w.channel_index == control.channel_index
                        && fitted_uv.contains(&w.channel_index)
                        && w.raw != current[control.channel_index as usize]
                });
                continue;
            }
            let uv_write = scratch.output.writes.iter().find(|w| {
                w.channel_index == control.channel_index
                    && matches!(
                        w.role,
                        light_fixture::forward::ColorWriteRole::Ultraviolet { .. }
                    )
            });
            let write = match uv_write {
                Some(write) => {
                    let mut native = NativeControlWrite::from_color(head.destination, write);
                    native.parked |= uv == 0.;
                    native
                }
                None => {
                    // Never UV from continuity: an unwritten UV control is parked off.
                    let ultraviolet = head
                        .native_controls
                        .iter()
                        .any(|c| c.channel_id == control.channel_id && c.ultraviolet);
                    let kept = held.filter(|_| !ultraviolet).and_then(|h| {
                        h.controls.iter().find(|(index, id, raw)| {
                            *index == control.channel_index
                                && *id == control.channel_id
                                && *raw <= control.raw_max
                        })
                    });
                    NativeControlWrite {
                        slot: NativeControlSlot {
                            destination: head.destination,
                            channel_index: control.channel_index,
                            split: control.split,
                        },
                        channel_id: control.channel_id,
                        function_id: None,
                        raw: kept.map_or(control.park_raw, |(_, _, raw)| *raw),
                        parked: kept.is_none(),
                    }
                }
            };
            current[control.channel_index as usize] = write.raw;
            writes.push(write);
        }
        let uv_fit = scratch.output.uv;
        let fit_limits = scratch.output.limitations.0
            & (ColorFitLimitations::UV_CLIPPED.0 | ColorFitLimitations::UV_UNSUPPORTED.0);
        let forward = Self::evaluate_forward(head, &mut scratch, current)?;
        self.record(&work, conflict);
        let mut quality = Self::forward_quality(forward, conflict);
        quality.visible = VisibleFitStatus::UnknownAppearance;
        quality.uv = uv_fit.status;
        quality.uv_clipped = uv_fit.clipped;
        quality.limitations.0 |= fit_limits;
        quality.work = work;
        let mut achieved = Self::forward_achieved(forward);
        achieved.uv_drive = achieved
            .uv_drive
            .filter(|_| uv_fit.status == UvFitStatus::Applied);
        // UV knowledge comes from the final written values, not the provisional UV-only fit.
        Self::settle_uv_appearance(head, &mut scratch, current, &fitted_uv, &mut quality)?;
        Ok(HeadResolution {
            achieved,
            quality,
            continuity: Self::continuity(head, &writes[first..]),
        })
    }

    fn evaluate_forward<'s>(
        head: &ColorHeadDescriptor,
        scratch: &'s mut ColorHeadScratch,
        current: &[u32],
    ) -> Result<&'s ColorForwardResult, TransitionError> {
        let model = head.fitting.forward();
        if scratch.forward.is_empty() {
            scratch.forward = model.create_output();
        }
        model
            .evaluate(current, &mut scratch.forward)
            .map_err(|error| invalid(format!("Color forward evaluation rejected: {error:?}")))?;
        Ok(&scratch.forward[head.head])
    }

    fn forward_achieved(forward: &ColorForwardResult) -> AchievedColor {
        AchievedColor {
            visible: forward.visible_complete.then_some(forward.known_xyz),
            known_xyz: forward.known_xyz,
            uv_drive: forward.portable_uv.map(|uv| uv.amount),
        }
    }

    /// Passive quality of written native values that were not fitted to a visible request.
    fn forward_quality(forward: &ColorForwardResult, shared_conflict: bool) -> ColorQuality {
        let mut limitations = ColorFitLimitations::default();
        if shared_conflict {
            limitations.0 |= ColorFitLimitations::SHARED_CONTROL.0;
        }
        ColorQuality {
            visible: VisibleFitStatus::NotEvaluated,
            color_match: ColorMatch::Unknown,
            delta_uv: None,
            luminance_ratio: None,
            luminance_limited: false,
            data_quality: forward.data_quality,
            nominal: !matches!(forward.data_quality, PhysicalDataQuality::Measured),
            discrete: false,
            total_quality: if forward.visible_complete {
                forward.data_quality
            } else {
                PhysicalDataQuality::Unknown
            },
            uv: if forward.uv_emitters.is_empty() {
                UvFitStatus::NotRequested
            } else {
                UvFitStatus::Applied
            },
            uv_clipped: false,
            uv_appearance_known: forward.visible_complete,
            limitations,
            constraints: Vec::new(),
            parked: Vec::new(),
            shared_conflict,
            work: ColorSolveWork::default(),
            direct: None,
            heads: Vec::new(),
        }
    }

    fn continuity(
        head: &ColorHeadDescriptor,
        written: &[NativeControlWrite],
    ) -> ColorHeadContinuity {
        ColorHeadContinuity {
            destination: head.destination,
            head_id: head.head_id,
            controls: written
                .iter()
                .map(|w| (w.slot.channel_index, w.channel_id, w.raw))
                .collect(),
        }
    }
}

/// Measured quality of a replayed native output against its source estimate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SourceMatch {
    pub color_match: ColorMatch,
    /// Finite Δu′v′, `None` when either side is black or unknown.
    pub delta_uv: Option<f64>,
    /// Finite actual Y / source Y, `None` when the source Y cannot support a meaningful ratio.
    pub luminance_ratio: Option<f64>,
    pub luminance_limited: bool,
}

/// The fixture fitter's metric applied to replayed output: f64 chromaticity (denominator above
/// 1e-12), black only below [`light_fixture::forward::COLOR_MATCH_BLACK_Y`] and only for a black source,
/// Δu′v′ within EXACT/APPROXIMATE. A dim chromatic source stays chromatic and is never normalized
/// to D65; a nonblack source with Y = 0 is compared by chromaticity with no luminance ratio.
/// Unknown or non-finite appearance on either side is `Unknown`, never `Exact`.
pub(super) fn measure_against_source(
    actual: Xyz,
    complete: bool,
    source: Option<Xyz>,
) -> SourceMatch {
    let unknown = SourceMatch {
        color_match: ColorMatch::Unknown,
        delta_uv: None,
        luminance_ratio: None,
        luminance_limited: false,
    };
    let finite = |v: Xyz| [v.x, v.y, v.z].into_iter().all(f32::is_finite);
    let Some(source) = source.filter(|s| complete && finite(*s) && finite(actual)) else {
        return unknown;
    };
    let black = light_fixture::forward::COLOR_MATCH_BLACK_Y;
    let (actual_y, source_y) = (f64::from(actual.y), f64::from(source.y));
    let black_source = light_fixture::forward::measured_chromaticity(source).is_none();
    let delta_uv =
        light_fixture::forward::measured_delta_uv(actual, source).filter(|d| d.is_finite());
    let luminance_ratio = (!black_source && source_y > black)
        .then(|| actual_y / source_y)
        .filter(|r| r.is_finite());
    let luminance_limited = match luminance_ratio {
        Some(ratio) => (ratio - 1.).abs() > 0.01,
        // Black or zero-Y source: any visible luminance beyond the black threshold is a
        // luminance mismatch; matching near-zero luminance is not.
        None => (actual_y - source_y).abs() > black,
    };
    SourceMatch {
        color_match: light_fixture::forward::measured_color_match(actual, source),
        delta_uv,
        luminance_ratio,
        luminance_limited,
    }
}
