//! Per-destination, per-head resolve of the Color adapter (TL-592, extended by TL-557 for
//! root-multihead targets and multipatch copies with their own calibration).
use super::*;

/// Outcome of one head before it is folded into the published resolution.
#[derive(Clone)]
pub(super) struct HeadResolution {
    pub achieved: AchievedColor,
    pub quality: ColorQuality,
    pub continuity: ColorHeadContinuity,
}

/// Every destination head's writes, outcomes and continuity for one target and frame.
pub(super) struct HeadsResolution {
    pub writes: Vec<NativeControlWrite>,
    pub outcomes: Vec<ColorHeadOutcome>,
    pub continuity: ColorContinuity,
}

/// The last semantic resolve of one destination head (TL-553). The fit reads only the intent
/// and the head's input channels of the seeded `current` (after the whole-vector validation),
/// so an unchanged key replays the exact writes, achieved output, quality and continuity.
/// The descriptor, and with it this memo, is recompiled with every runtime generation.
pub(super) struct ColorHeadMemo {
    intent: ColorIntent,
    key: Vec<u32>,
    writes: Vec<NativeControlWrite>,
    resolution: HeadResolution,
}

impl ColorHeadMemo {
    fn matches(&self, head: &ColorHeadDescriptor, intent: &ColorIntent, current: &[u32]) -> bool {
        self.intent == *intent
            && head.fitting.accepts_raw(current)
            && head
                .inputs
                .iter()
                .zip(&self.key)
                .all(|(&index, &raw)| current[index] == raw)
    }
}

impl ColorAdapter {
    pub(super) fn fit(
        &self,
        head: &ColorHeadDescriptor,
        intent: &ColorIntent,
        current: &[u32],
        scratch: &mut ColorHeadScratch,
        work: &mut ColorSolveWork,
    ) -> Result<(), TransitionError> {
        let ColorHeadScratch {
            workspace, output, ..
        } = scratch;
        head.fitting
            .fit(head.head, current, intent, workspace, output)
            .map_err(|error| invalid(format!("Color fitting input rejected: {error:?}")))?;
        if output.status != light_fixture::forward::ColorFitStatus::Fitted {
            return Err(invalid(
                "semantic Color intent is invalid or carries unresolved spreads",
            ));
        }
        work.fits += 1;
        work.candidates_ranked += output.candidates_ranked;
        let step = output.work;
        work.fit.visible_solves += step.visible_solves;
        work.fit.fixed_offset_solves += step.fixed_offset_solves;
        work.fit.level_solves += step.level_solves;
        work.fit.forward_evaluations += step.forward_evaluations;
        Ok(())
    }

    /// Overlay this destination head's last accepted writes and park its unmodeled controls.
    /// Only controls the head writes are touched: a claimed shared slot keeps its owner's value.
    pub(super) fn seed_head(
        head: &ColorHeadDescriptor,
        previous: Option<&ColorContinuity>,
        current: &mut [u32],
    ) {
        let owned = |index: u32| {
            head.controls
                .iter()
                .zip(head.writes_control.iter())
                .find(|(control, _)| control.channel_index == index)
                .filter(|(_, writes)| **writes)
                .map(|(control, _)| control)
        };
        if let Some(previous) = previous.and_then(|p| p.head(head.destination, head.head_id)) {
            for &(index, id, raw) in &previous.controls {
                if owned(index).is_some_and(|c| c.channel_id == id && raw <= c.raw_max) {
                    current[index as usize] = raw;
                }
            }
        }
        for (control, writes) in head.controls.iter().zip(head.writes_control.iter()) {
            if *writes && !control.modeled {
                current[control.channel_index as usize] = control.park_raw;
            }
        }
    }

    /// Park every retained control this head writes; true when `current` changed.
    fn park_retained(
        head: &ColorHeadDescriptor,
        output: &ColorFitResult,
        current: &mut [u32],
    ) -> bool {
        let mut changed = false;
        for retained in &output.retained {
            let Some((control, _)) = head
                .controls
                .iter()
                .zip(head.writes_control.iter())
                .find(|(c, writes)| **writes && c.channel_index == retained.channel_index)
            else {
                continue;
            };
            let slot = &mut current[control.channel_index as usize];
            changed |= *slot != control.park_raw;
            *slot = control.park_raw;
        }
        changed
    }

    /// Emit (or park) every control this head writes, and apply the writes to `current`.
    /// Returns true when the fitter wanted another value for a claimed shared slot.
    fn emit(
        head: &ColorHeadDescriptor,
        output: &ColorFitResult,
        uv_zero: bool,
        current: &mut [u32],
        writes: &mut Vec<NativeControlWrite>,
    ) -> bool {
        let mut conflict = false;
        for (control, owned) in head.controls.iter().zip(head.writes_control.iter()) {
            let fitted = output
                .writes
                .iter()
                .find(|w| w.channel_index == control.channel_index);
            let index = control.channel_index as usize;
            if !*owned {
                conflict |= fitted.is_some_and(|w| w.raw != current[index]);
                continue;
            }
            let native = match fitted {
                Some(write) => {
                    let mut native = NativeControlWrite::from_color(head.destination, write);
                    native.parked |= uv_zero
                        && matches!(
                            write.role,
                            light_fixture::forward::ColorWriteRole::Ultraviolet { .. }
                        );
                    native
                }
                None => NativeControlWrite {
                    slot: NativeControlSlot {
                        destination: head.destination,
                        channel_index: control.channel_index,
                        split: control.split,
                    },
                    channel_id: control.channel_id,
                    function_id: None,
                    raw: control.park_raw,
                    parked: true,
                },
            };
            current[index] = native.raw;
            writes.push(native);
        }
        conflict
    }

    pub(super) fn quality(
        output: &ColorFitResult,
        work: ColorSolveWork,
        actual: Option<&ColorForwardResult>,
    ) -> ColorQuality {
        let shared_conflict = actual.is_some();
        let visible = &output.visible;
        let mut limitations = output.limitations;
        if shared_conflict {
            limitations.0 |= ColorFitLimitations::SHARED_CONTROL.0;
        }
        let mut quality = ColorQuality {
            visible: visible.status,
            color_match: visible.color_match,
            delta_uv: visible.delta_uv,
            luminance_ratio: visible.luminance_ratio,
            luminance_limited: visible.luminance_limited,
            data_quality: visible.data_quality,
            nominal: visible.nominal,
            discrete: false,
            total_quality: output.total_quality,
            uv: output.uv.status,
            uv_clipped: output.uv.clipped,
            uv_appearance_known: output.uv.appearance_known,
            limitations,
            constraints: output.constraints.clone(),
            parked: output
                .retained
                .iter()
                .map(|r| (r.channel_id, r.reason))
                .collect(),
            shared_conflict,
            work,
            direct: None,
            heads: Vec::new(),
        };
        if let Some(forward) = actual {
            // The match figures above still describe the fitter's unshared proposal. Quality
            // and completeness describe the actual shared values evaluated below instead.
            if !forward.visible_complete {
                quality.visible = VisibleFitStatus::PredictionIncomplete;
            }
            quality.data_quality = forward.data_quality;
            quality.nominal = matches!(
                forward.data_quality,
                PhysicalDataQuality::Unknown | PhysicalDataQuality::Estimated
            );
            quality.total_quality = if forward.visible_complete {
                forward.data_quality
            } else {
                PhysicalDataQuality::Unknown
            };
            // The forward result exposes whole-path completeness, not per-contributor
            // appearance. Require that complete prediction for active UV; zero is known.
            quality.uv_appearance_known =
                !forward.uv_emitters.iter().any(|uv| uv.drive > 0.) || forward.visible_complete;
            if quality.uv_appearance_known {
                quality.limitations.0 &= !ColorFitLimitations::UNKNOWN_UV_APPEARANCE.0;
            } else {
                quality.limitations.0 |= ColorFitLimitations::UNKNOWN_UV_APPEARANCE.0;
            }
        }
        quality
    }

    /// Achieved output of the written values: the fitter's forward evaluation, or a forward
    /// re-evaluation of the destination's final raw values after a shared-slot conflict.
    fn achieved(
        head: &ColorHeadDescriptor,
        scratch: &mut ColorHeadScratch,
        current: &[u32],
        conflict: bool,
    ) -> Result<AchievedColor, TransitionError> {
        let output = &scratch.output;
        if !conflict {
            return Ok(AchievedColor {
                visible: output.visible.achieved,
                known_xyz: output.visible.known_xyz,
                uv_drive: output.uv.achieved_drive,
            });
        }
        let model = head.fitting.forward();
        if scratch.forward.is_empty() {
            scratch.forward = model.create_output();
        }
        model
            .evaluate(current, &mut scratch.forward)
            .map_err(|error| invalid(format!("Color forward evaluation rejected: {error:?}")))?;
        let forward = &scratch.forward[head.head];
        Ok(AchievedColor {
            visible: forward.visible_complete.then_some(forward.known_xyz),
            known_xyz: forward.known_xyz,
            uv_drive: forward.portable_uv.map(|uv| uv.amount),
        })
    }

    pub(super) fn resolve_head(
        &self,
        head: &ColorHeadDescriptor,
        intent: &ColorIntent,
        previous: Option<&ColorContinuity>,
        current: &mut [u32],
        writes: &mut Vec<NativeControlWrite>,
    ) -> Result<HeadResolution, TransitionError> {
        let mut scratch = head.scratch.lock();
        Self::seed_head(head, previous, current);
        if let Some(memo) = scratch
            .memo
            .as_ref()
            .filter(|memo| memo.matches(head, intent, current))
        {
            // TL-553: identical fit inputs give the identical result; replay it exactly.
            for write in &memo.writes {
                current[write.slot.channel_index as usize] = write.raw;
            }
            writes.extend_from_slice(&memo.writes);
            self.count(|c| c.result_reuses += 1);
            let mut resolution = memo.resolution.clone();
            // The published work describes this resolve, which fitted nothing.
            resolution.quality.work = ColorSolveWork::default();
            return Ok(resolution);
        }
        let key = head.inputs.iter().map(|&index| current[index]).collect();
        let mut work = ColorSolveWork::default();
        self.fit(head, intent, current, &mut scratch, &mut work)?;
        if Self::park_retained(head, &scratch.output, current) {
            // The forward prediction must describe the parked output, not the stale value.
            self.fit(head, intent, current, &mut scratch, &mut work)?;
            if Self::park_retained(head, &scratch.output, current) {
                return Err(invalid("retained Color control could not be parked"));
            }
        }
        let first = writes.len();
        let conflict = Self::emit(
            head,
            &scratch.output,
            intent.uv.amount == 0.,
            current,
            writes,
        );
        let achieved = Self::achieved(head, &mut scratch, current, conflict)?;
        self.record(&work, conflict);
        let resolution = HeadResolution {
            achieved,
            quality: ColorQuality {
                discrete: !head.fitting.has_visible_emitters(head.head),
                ..Self::quality(
                    &scratch.output,
                    work,
                    conflict.then(|| &scratch.forward[head.head]),
                )
            },
            continuity: ColorHeadContinuity {
                destination: head.destination,
                head_id: head.head_id,
                controls: writes[first..]
                    .iter()
                    .map(|w| (w.slot.channel_index, w.channel_id, w.raw))
                    .collect(),
            },
        };
        scratch.memo = Some(ColorHeadMemo {
            intent: intent.clone(),
            key,
            writes: writes[first..].to_vec(),
            resolution: resolution.clone(),
        });
        Ok(resolution)
    }

    pub(super) fn record(&self, work: &ColorSolveWork, conflict: bool) {
        self.count(|c| {
            c.fits += u64::from(work.fits);
            c.refits += u64::from(work.fits.saturating_sub(1));
            c.shared_conflicts += u64::from(conflict);
            c.candidates_ranked += u64::from(work.candidates_ranked);
            c.visible_solves += u64::from(work.fit.visible_solves);
            c.fixed_offset_solves += u64::from(work.fit.fixed_offset_solves);
            c.level_solves += u64::from(work.fit.level_solves);
            c.forward_evaluations += u64::from(work.fit.forward_evaluations);
        });
    }

    /// Resolve every destination head of the target under one captured token. Direct values
    /// take the replay path (`direct.rs`); everything else must be a semantic intent.
    pub(super) fn resolve_heads(
        &self,
        request: PhysicalRequest<'_, Self>,
    ) -> Result<PhysicalResolution<Self>, TransitionError> {
        if let AttributeValue::ColorProgram(program) = request.value
            && matches!(program.as_ref(), ColorProgram::Direct { .. })
        {
            return self.resolve_direct(&request, program);
        }
        let intent = semantic_intent(request.value)?;
        let resolved = self.resolve_each_head(&request, |adapter, head, current, writes| {
            adapter.resolve_head(head, intent, request.previous, current, writes)
        })?;
        Ok(Self::publish(
            resolved,
            ColorRequest::Semantic(intent.clone()),
        ))
    }

    /// Read the root's pre-master native raw values once under the request's token, then run
    /// `per_head` for every destination head. Every destination starts from the root's
    /// resolved channels (copies duplicate them); heads of one destination share its values.
    pub(super) fn resolve_each_head(
        &self,
        request: &PhysicalRequest<'_, Self>,
        mut per_head: impl FnMut(
            &Self,
            &ColorHeadDescriptor,
            &mut [u32],
            &mut Vec<NativeControlWrite>,
        ) -> Result<HeadResolution, TransitionError>,
    ) -> Result<HeadsResolution, TransitionError> {
        let descriptor = request.descriptor;
        let mut scratch = descriptor.scratch.lock();
        let ColorScratch {
            native, current, ..
        } = &mut *scratch;
        request.frame.native_raw_into(request.target, native)?;
        if native.destination() != Some(descriptor.root)
            || native.token() != Some(request.frame.token)
        {
            return Err(invalid(
                "Color native raw values belong to another destination or frame",
            ));
        }
        let mut writes = Vec::with_capacity(descriptor.footprint.len());
        let mut outcomes = Vec::with_capacity(descriptor.heads.len());
        let mut continuity = ColorContinuity::default();
        let mut destination = None;
        for head in descriptor.heads.iter() {
            if destination != Some(head.destination) {
                destination = Some(head.destination);
                current.clear();
                current.extend_from_slice(native.raw());
            }
            let resolved = per_head(self, head, current, &mut writes)?;
            continuity.heads.push(resolved.continuity);
            outcomes.push(ColorHeadOutcome {
                destination: head.destination,
                head_id: head.head_id,
                achieved: resolved.achieved,
                quality: resolved.quality,
            });
        }
        self.count(|c| c.resolves += 1);
        Ok(HeadsResolution {
            writes,
            outcomes,
            continuity,
        })
    }

    /// The primary head's achieved/quality plus the complete per-head breakdown.
    pub(super) fn publish(
        resolved: HeadsResolution,
        requested: ColorRequest,
    ) -> PhysicalResolution<Self> {
        let HeadsResolution {
            writes,
            outcomes,
            continuity,
        } = resolved;
        let primary = &outcomes[0];
        let achieved = primary.achieved;
        let mut quality = primary.quality.clone();
        quality.heads = outcomes;
        PhysicalResolution {
            writes,
            requested,
            achieved,
            quality,
            continuity,
        }
    }
}
