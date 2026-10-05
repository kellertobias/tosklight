//! Isolated evaluation of pending Programmer intent from one immutable output capture.

use crate::{
    ContributionBatch, ContributionSourceId, Engine, EngineError, ObservedSourceFrame,
    OutputContinuityState, PreparedOutputFrame, PreparedPreloadSources, PreparedStaticFamilyFrame,
    ProfileVisualizationProjection, programmer_release::ProgrammerReleaseMemo,
    programmer_resolution::ProgrammerAddressMemo, resolution::ProgrammerLaneInputs,
};
use light_core::{AttributeKey, FixtureId};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;

/// One pending lane's source bundle. A hypothetical Playback runtime, when supplied, is
/// projected once at the captured clock. Static Current, both Release branches, Cue Dynamics
/// and the final preview therefore observe the same Playback selection and timing.
pub struct PreparedPreloadFrame<'a> {
    identity: Arc<()>,
    frame: &'a PreparedOutputFrame,
    sources: Arc<PreparedPreloadSources>,
    playback: Option<crate::resolution::PlaybackResolution>,
    playback_scratch: Arc<Mutex<crate::engine::FrameScratch>>,
}

/// The two independent source lanes used by a pending Color Release preview.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreloadBranch {
    BeforeRelease,
    AfterRelease,
}

#[derive(Clone)]
pub(crate) struct PreloadTokenIdentity {
    bundle: Arc<()>,
    state: Arc<()>,
    revision: u64,
    branch: PreloadBranch,
}

impl PreloadTokenIdentity {
    fn new(
        input: &PreparedPreloadFrame<'_>,
        state: &PreloadFrameState,
        branch: PreloadBranch,
    ) -> Self {
        Self {
            bundle: Arc::clone(&input.identity),
            state: Arc::clone(&state.identity),
            revision: state.revision,
            branch,
        }
    }

    fn matches(
        &self,
        input: &PreparedPreloadFrame<'_>,
        state: &PreloadFrameState,
        branch: PreloadBranch,
    ) -> bool {
        Arc::ptr_eq(&self.bundle, &input.identity)
            && Arc::ptr_eq(&self.state, &state.identity)
            && self.revision == state.revision
            && self.branch == branch
    }

    pub(crate) fn same_identity(
        &self,
        bundle: &Arc<()>,
        state: &Arc<()>,
        revision: u64,
        branch: PreloadBranch,
    ) -> bool {
        Arc::ptr_eq(&self.bundle, bundle)
            && Arc::ptr_eq(&self.state, state)
            && self.revision == revision
            && self.branch == branch
    }

    pub(crate) fn release_pair(before: &Self, after: &Self) -> bool {
        Arc::ptr_eq(&before.bundle, &after.bundle)
            && Arc::ptr_eq(&before.state, &after.state)
            && before.revision == after.revision
            && before.branch == PreloadBranch::BeforeRelease
            && after.branch == PreloadBranch::AfterRelease
    }
}

impl Drop for PreparedPreloadFrame<'_> {
    fn drop(&mut self) {
        let Some(playback) = &mut self.playback else {
            return;
        };
        let mut contributions = std::mem::take(&mut playback.contributions);
        contributions.clear();
        let mut scratch = self.playback_scratch.lock();
        if scratch.contributions.capacity() < contributions.capacity() {
            scratch.contributions = contributions;
        }
    }
}

impl PreparedPreloadFrame<'_> {
    pub fn sources(&self) -> &Arc<PreparedPreloadSources> {
        &self.sources
    }

    pub fn frame(&self) -> &PreparedOutputFrame {
        self.frame
    }

    /// Identity handles for [`crate::CapturedFrameToken`]; never the bundle or state itself.
    pub(crate) fn token_identity(&self, state: &PreloadFrameState) -> (Arc<()>, Arc<()>, u64) {
        (
            Arc::clone(&self.identity),
            Arc::clone(&state.identity),
            state.revision,
        )
    }

    fn playback(&self) -> &crate::resolution::PlaybackResolution {
        self.playback.as_ref().unwrap_or(&self.frame.playback)
    }

    pub fn cue_dynamic_values(&self) -> &[light_playback::ActiveCueDynamicValue] {
        &self.playback().cue_dynamic_values
    }

    pub fn dynamic_playbacks(&self) -> &[light_playback::ActiveDynamicPlayback] {
        &self.playback().dynamic_playbacks
    }

    pub fn playback_dynamics_paused(&self) -> bool {
        self.playback().dynamics_paused
    }
}

/// One desk's Preload evaluation caches. They cannot evict Live's address/release caches.
/// Discard on show activation; captured sources still provide normal patch-generation invalidation.
#[derive(Default)]
pub struct PreloadFrameState {
    identity: Arc<()>,
    revision: u64,
    before_addresses: Mutex<ProgrammerAddressMemo>,
    after_addresses: Mutex<ProgrammerAddressMemo>,
    before_releases: Mutex<ProgrammerReleaseMemo>,
    after_releases: Mutex<ProgrammerReleaseMemo>,
    before_mounts: crate::mount_projection::MountTransformWorkspace,
    after_mounts: crate::mount_projection::MountTransformWorkspace,
}

pub struct RenderedPreloadFrame {
    pub source: ObservedSourceFrame,
    /// The exact original branch already used for Color Release ownership, when required.
    pub before_release: Option<ObservedSourceFrame>,
    pub projection: ProfileVisualizationProjection,
}

impl Engine {
    /// `playback` must be the isolated hypothetical runtime derived from this capture. This
    /// reads it at the captured clock, never ticks it, and never installs it in Live. Its
    /// automatic Cue actions/events deliberately have no dispatch path through this type.
    pub fn prepare_preload_frame<'a>(
        &self,
        frame: &'a PreparedOutputFrame,
        playback: Option<&light_playback::PlaybackEngine>,
    ) -> PreparedPreloadFrame<'a> {
        PreparedPreloadFrame {
            identity: Arc::new(()),
            frame,
            sources: self.prepare_preload_sources(frame.programmer()),
            playback: playback.map(|playback| {
                Self::playback_resolution(
                    playback,
                    frame.sampled_at,
                    Vec::new(),
                    &[],
                    false,
                    &self.preload_playback_scratch,
                )
            }),
            playback_scratch: Arc::clone(&self.preload_playback_scratch),
        }
    }

    /// Static Current for a pending Dynamic is evaluated in the pending lane, without advancing
    /// either lane's history. The final preview supplies its own Dynamic batches from this clock.
    pub fn observe_prepared_preload(
        &self,
        input: &PreparedPreloadFrame<'_>,
        sampled: &[ContributionBatch],
        state: &PreloadFrameState,
        before_release: bool,
    ) -> ObservedSourceFrame {
        self.evaluate_prepared_preload(
            input,
            sampled,
            state,
            before_release,
            &mut input.frame.continuity.clone(),
        )
    }

    /// Scalar Current for one pending branch, retaining Freeze and source evidence without
    /// resolving Points or mounts. Final scalar output owns the branch's geometry solve.
    /// This consumes only captured sources and never advances Live or pending continuity.
    pub fn observe_prepared_preload_values(
        &self,
        input: &PreparedPreloadFrame<'_>,
        sampled: &[ContributionBatch],
        state: &PreloadFrameState,
        branch: PreloadBranch,
    ) -> crate::FrameValues {
        let (states, releases, addresses) = match branch {
            PreloadBranch::BeforeRelease => (
                &input.sources.before,
                &state.before_releases,
                &state.before_addresses,
            ),
            PreloadBranch::AfterRelease => (
                &input.sources.after,
                &state.after_releases,
                &state.after_addresses,
            ),
        };
        let releases = releases.lock().compile(states, &input.frame.generation);
        self.observe_prepared_lane_values(
            input.frame,
            sampled,
            &mut input.frame.continuity.clone(),
            ProgrammerLaneInputs {
                playback: input.playback(),
                states,
                releases: &releases,
                addresses,
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_prepared_preload(
        &self,
        input: &PreparedPreloadFrame<'_>,
        sampled: &[ContributionBatch],
        state: &PreloadFrameState,
        before_release: bool,
        continuity: &mut OutputContinuityState,
    ) -> ObservedSourceFrame {
        let frame = input.frame;
        let sources = &input.sources;
        let (states, releases, addresses) = if before_release {
            (
                &sources.before,
                &state.before_releases,
                &state.before_addresses,
            )
        } else {
            (
                &sources.after,
                &state.after_releases,
                &state.after_addresses,
            )
        };
        let releases = releases.lock().compile(states, &frame.generation);
        let mut observed = self.observe_prepared_lane(
            frame,
            sampled,
            continuity,
            ProgrammerLaneInputs {
                playback: input.playback(),
                states,
                releases: &releases,
                addresses,
            },
        );
        observed.preload = Some(PreloadTokenIdentity::new(
            input,
            state,
            if before_release {
                PreloadBranch::BeforeRelease
            } else {
                PreloadBranch::AfterRelease
            },
        ));
        observed
    }

    /// Resolve one pending static lane before Freeze. The returned token shares the ordinary
    /// family query/write and cached geometry APIs, but only Preload finalization may consume it.
    pub fn prepare_preload_static_family_frame(
        &self,
        input: &PreparedPreloadFrame<'_>,
        sampled: &[ContributionBatch],
        state: &PreloadFrameState,
        branch: PreloadBranch,
    ) -> PreparedStaticFamilyFrame {
        let (states, releases, addresses, mounts) = match branch {
            PreloadBranch::BeforeRelease => (
                &input.sources.before,
                &state.before_releases,
                &state.before_addresses,
                &state.before_mounts,
            ),
            PreloadBranch::AfterRelease => (
                &input.sources.after,
                &state.after_releases,
                &state.after_addresses,
                &state.after_mounts,
            ),
        };
        let releases = releases.lock().compile(states, &input.frame.generation);
        // Pending targets never seed the Live Programmer fade history. Only the branch-local
        // mount cache persists between successful Preload frames.
        let mut continuity = input.frame.continuity.clone();
        continuity.mounts = mounts.clone();
        let mut resolved = self.resolve_prepared_lane_attributes(
            input.frame,
            sampled,
            &mut continuity,
            true,
            ProgrammerLaneInputs {
                playback: input.playback(),
                states,
                releases: &releases,
                addresses,
            },
        );
        input.frame.finalize_output_parameters(&mut resolved);
        PreparedStaticFamilyFrame {
            capture_identity: Arc::clone(&input.frame.identity),
            preload: Some(PreloadTokenIdentity::new(input, state, branch)),
            resolved,
            continuity,
            geometry: None,
            projections: Default::default(),
            position_native: Default::default(),
            native_raw: Default::default(),
        }
    }

    /// Query the compiled Release plan without resolving values or physical output.
    pub fn preload_requires_before_release(
        &self,
        input: &PreparedPreloadFrame<'_>,
        state: &PreloadFrameState,
    ) -> bool {
        state
            .after_releases
            .lock()
            .compile(&input.sources.after, &input.frame.generation)
            .excluded_addresses()
            .any(|(_, key)| key.0.as_ref() == "color")
    }

    /// Consume completed pending branches without another static solve or Live continuity commit.
    /// All token identities are checked before projection. A failure leaves both pending mount caches
    /// and their revision unchanged; observations keep their original capture and branch identity.
    pub fn render_prepared_preload_families(
        &self,
        input: &PreparedPreloadFrame<'_>,
        before: Option<PreparedStaticFamilyFrame>,
        after: PreparedStaticFamilyFrame,
        state: &mut PreloadFrameState,
    ) -> Result<RenderedPreloadFrame, EngineError> {
        let valid = |token: &PreparedStaticFamilyFrame, branch| {
            Arc::ptr_eq(&token.capture_identity, &input.frame.identity)
                && token
                    .preload
                    .as_ref()
                    .is_some_and(|identity| identity.matches(input, state, branch))
        };
        if !valid(&after, PreloadBranch::AfterRelease)
            || before
                .as_ref()
                .is_some_and(|token| !valid(token, PreloadBranch::BeforeRelease))
        {
            return Err(EngineError::StalePreparedFrame);
        }
        let needs_before = self.preload_requires_before_release(input, state);
        if needs_before && before.is_none() {
            return Err(EngineError::Invalid(
                "Color Release requires a completed before-Release family token".into(),
            ));
        }
        let revision = state
            .revision
            .checked_add(1)
            .ok_or_else(|| EngineError::Invalid("Preload continuity revision exhausted".into()))?;
        let observe = |token: PreparedStaticFamilyFrame| -> Result<_, EngineError> {
            let identity = token.preload.clone();
            let (resolved, mut continuity, geometry, native) = token.into_projected(input.frame)?;
            let mut observed = self.observe_resolved_prepared_frame(
                input.frame,
                resolved,
                &mut continuity,
                geometry,
                native,
            );
            observed.preload = identity;
            Ok((observed, continuity.mounts))
        };
        let (after, after_mounts) = observe(after)?;
        let before = if needs_before {
            Some(observe(before.expect("validated before token"))?)
        } else {
            None
        };
        let previewed = pending_winning_addresses(input.frame, &after);
        let projection = if let Some((before, _)) = &before {
            let releases = state
                .after_releases
                .lock()
                .compile(&input.sources.after, &input.frame.generation);
            self.profile_color_release_frames(
                before,
                &after,
                &releases,
                input.frame.overlays.options,
                &previewed,
            )?
        } else {
            self.profile_observed_source_frame(&after, input.frame.overlays.options, &previewed)?
        };
        state.after_mounts = after_mounts;
        let before_release = before.map(|(observed, mounts)| {
            state.before_mounts = mounts;
            observed
        });
        state.revision = revision;
        Ok(RenderedPreloadFrame {
            source: after,
            before_release,
            projection,
        })
    }

    /// Evaluate and project once at the scheduler boundary. Neither branch reads mutable engine
    /// sources, ticks Playback, nor commits Live continuity. Callers publish only with the accepted
    /// Live capture and keep this result for all consumers until the next accepted publication.
    pub fn render_prepared_preload(
        &self,
        input: &PreparedPreloadFrame<'_>,
        before_samples: &[ContributionBatch],
        after_samples: &[ContributionBatch],
        state: &mut PreloadFrameState,
    ) -> Result<RenderedPreloadFrame, EngineError> {
        let before = self.preload_requires_before_release(input, state).then(|| {
            self.prepare_preload_static_family_frame(
                input,
                before_samples,
                state,
                PreloadBranch::BeforeRelease,
            )
        });
        let after = self.prepare_preload_static_family_frame(
            input,
            after_samples,
            state,
            PreloadBranch::AfterRelease,
        );
        self.render_prepared_preload_families(input, before, after, state)
    }
}

fn pending_winning_addresses(
    frame: &PreparedOutputFrame,
    after: &ObservedSourceFrame,
) -> HashSet<(FixtureId, AttributeKey)> {
    let mut result = HashSet::new();
    let (Some(id), Some(preload)) = (frame.programmer.identity, frame.programmer.preload.as_ref())
    else {
        return result;
    };
    let pending = &preload.pending;
    let source = ContributionSourceId::preload(id);
    for value in pending.fixture_values.iter() {
        if after
            .values()
            .contribution_origin(value.fixture_id, &value.attribute)
            .is_some_and(|origin| {
                origin.source() == &source
                    && origin.stamp().changed_at == value.changed_at
                    && origin.stamp().programmer_order == value.programmer_order
            })
        {
            result.insert((value.fixture_id, value.attribute.clone()));
        }
    }
    let rankings = frame.generation.group_rankings_arc();
    for (group, values) in pending.group_values.iter() {
        let Some(ranking) = rankings.get(group) else {
            continue;
        };
        let source = ContributionSourceId::preload_group(id, group.as_str());
        for fixture in &ranking.ordered_fixture_ids {
            for (attribute, value) in values {
                if after
                    .values()
                    .contribution_origin(*fixture, attribute)
                    .is_some_and(|origin| {
                        origin.source() == &source
                            && origin.stamp().changed_at == value.changed_at
                            && origin.stamp().programmer_order == value.programmer_order
                    })
                {
                    result.insert((*fixture, attribute.clone()));
                }
            }
        }
    }
    // Every Dynamic-sourced Pending address is previewed (TL-610), as the read-time route does:
    // its address is authoritative, so ownership is never inferred by comparing samples. The
    // native mask still requires each channel to win or to be a fitted write of that owner.
    // Scalar Off previews reveal the underlay but still address a channel. Whole Color Release
    // is handled separately through the excluded-winner trace, including consumed native controls.
    for value in pending.dynamic_values.iter() {
        let removal = matches!(
            value.value,
            light_dynamics::DynamicSemanticValue::DynamicOff { .. }
                | light_dynamics::DynamicSemanticValue::Release
        );
        if !removal || value.attribute.0.as_ref() != "color" {
            result.insert((value.fixture_id, value.attribute.clone()));
        }
    }
    result
}
