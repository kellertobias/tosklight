//! One static resolution followed by complete-family composition. The immutable baseline and
//! candidate continuity belong to the same capture; projection never re-enters LTP arbitration.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, programming::*};

use crate::{
    ContributionBatch, ContributionFamilyEvidence, ContributionOrigin, ContributionSequenceMaster,
    Engine, EngineError, OutputContinuityState, PreparedOutputFrame, RenderResult,
};

/// Source metadata is independent of the family value and its output master. Mixed provenance
/// which the adapter retains separately must use an explicit unknown replacement here.
#[derive(Clone, Debug)]
pub enum FamilyProjectionEvidence {
    PreserveBaseline,
    Replace {
        origin: Option<Arc<ContributionOrigin>>,
        family_evidence: Option<Arc<ContributionFamilyEvidence>>,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum FamilyProjectionMaster {
    PreserveBaseline,
    Replace(ContributionSequenceMaster),
    Remove,
}

#[derive(Clone, Debug)]
pub struct FamilyProjectionMetadata {
    /// The producer's actual output change time, if known. None reports an unknown timestamp;
    /// it does not borrow the static source's edit time or manufacture an arbitration vote.
    pub changed_at: Option<DateTime<Utc>>,
    pub evidence: FamilyProjectionEvidence,
    pub master: FamilyProjectionMaster,
}

impl FamilyProjectionMetadata {
    pub(crate) fn apply(self, winner: &mut crate::SlotWinner, value: AttributeValue) {
        winner.value = value;
        winner.projected_changed_at = Some(self.changed_at);
        if let FamilyProjectionEvidence::Replace {
            origin,
            family_evidence,
        } = self.evidence
        {
            winner.origin = origin;
            winner.family_evidence = family_evidence;
        }
        match self.master {
            FamilyProjectionMaster::PreserveBaseline => {}
            FamilyProjectionMaster::Replace(master) => winner.sequence_master = Some(master),
            FamilyProjectionMaster::Remove => winner.sequence_master = None,
        }
    }
}

/// Resolved pre-Freeze static values and speculative continuity from one exact capture.
///
/// Attribute queries always read the immutable static baseline, including after `project_family`.
/// Queued writes become visible only when this token is consumed for rendering or preview.
/// Dropping the token commits nothing. No named frame map is constructed by this API.
pub struct PreparedStaticFamilyFrame {
    pub(crate) capture_identity: Arc<()>,
    pub(crate) preload: Option<crate::preload_frame::PreloadTokenIdentity>,
    pub(crate) resolved: crate::ResolvedAttributes,
    pub(crate) continuity: OutputContinuityState,
    pub(crate) geometry: Option<crate::PreparedFrameGeometry>,
    pub(crate) position_native: crate::native_position_projection::NativePositionProjection,
    pub(crate) projections: rustc_hash::FxHashMap<
        (FixtureId, ProgrammingOwner),
        (AttributeValue, FamilyProjectionMetadata),
    >,
    /// TL-553: native raw values already captured from this token's immutable resolution, by
    /// physical root and Position instance. Every head of a multi-head root reads the same root
    /// vector; capturing it once per frame instead of once per head yields identical values.
    pub(crate) native_raw: crate::native_raw::NativeRawCache,
}

/// One resolved static winner, answering the same queries as [`PreparedStaticFamilyFrame`].
#[derive(Clone, Copy)]
pub struct StaticWinner<'a>(&'a crate::SlotWinner);

impl<'a> StaticWinner<'a> {
    pub fn value(self) -> &'a AttributeValue {
        &self.0.value
    }

    pub fn changed_at(self) -> Option<DateTime<Utc>> {
        self.0.output_changed_at()
    }

    pub fn contribution_origin(self) -> Option<&'a ContributionOrigin> {
        self.0.origin.as_deref()
    }

    pub fn contribution_family_evidence(self) -> Option<&'a Arc<ContributionFamilyEvidence>> {
        self.0.family_evidence.as_ref()
    }

    pub fn sequence_master(self) -> Option<ContributionSequenceMaster> {
        self.0.sequence_master
    }

    /// The live Position crossing behind this held static value (TL-544 G1). Runtime only.
    pub fn pending_transition(self) -> Option<&'a Arc<PendingFamilyTransition>> {
        self.0.pending_transition.as_ref()
    }
}

impl PreparedStaticFamilyFrame {
    fn winner(&self, target: FixtureId, attribute: &AttributeKey) -> Option<&crate::SlotWinner> {
        self.resolved.frame.as_ref()?.winner(target, attribute)
    }

    pub fn value(&self, target: FixtureId, attribute: &AttributeKey) -> Option<&AttributeValue> {
        self.winner(target, attribute).map(|winner| &winner.value)
    }

    pub fn changed_at(&self, target: FixtureId, attribute: &AttributeKey) -> Option<DateTime<Utc>> {
        self.winner(target, attribute)
            .and_then(crate::SlotWinner::output_changed_at)
    }

    pub fn contribution_origin(
        &self,
        target: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&ContributionOrigin> {
        self.winner(target, attribute)?.origin.as_deref()
    }

    pub fn contribution_family_evidence(
        &self,
        target: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&Arc<ContributionFamilyEvidence>> {
        self.winner(target, attribute)?.family_evidence.as_ref()
    }

    pub fn sequence_master(
        &self,
        target: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<ContributionSequenceMaster> {
        self.winner(target, attribute)?.sequence_master
    }

    /// The live Position crossing behind the held static value (TL-544 G1). Runtime only.
    pub fn pending_transition(
        &self,
        target: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&Arc<PendingFamilyTransition>> {
        self.winner(target, attribute)?.pending_transition.as_ref()
    }

    /// Every query above for one target and attribute, from one lookup (TL-639 round 2).
    pub fn static_winner(
        &self,
        target: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<StaticWinner<'_>> {
        self.winner(target, attribute).map(StaticWinner)
    }

    /// Queue a complete materialized family. At most one result may address each target/owner;
    /// compose all its writers first. A missing static base stays an upstream frame requirement.
    /// Room for `additional` more [`Self::project_family`] rows (TL-639 round 4). Projections
    /// are applied per row, so their storage order is not observable.
    pub fn reserve_family_projections(&mut self, additional: usize) {
        self.projections.reserve(additional);
    }

    pub fn project_family(
        &mut self,
        target: FixtureId,
        owner: ProgrammingOwner,
        value: AttributeValue,
        metadata: FamilyProjectionMetadata,
    ) -> Result<(), EngineError> {
        let invalid = |message: &str| EngineError::Invalid(message.into());
        if self.value(target, owner.key_ref()).is_none() {
            return Err(invalid(
                "family projection requires a captured static baseline",
            ));
        }
        ProgrammingFieldScope::for_value(owner, &value)
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        if let FamilyProjectionMaster::Replace(master) = metadata.master
            && (!master.scale().is_finite() || !(0.0..=1.0).contains(&master.scale()))
        {
            return Err(invalid(
                "family projection master must be finite and within 0-1",
            ));
        }
        if let FamilyProjectionEvidence::Replace {
            family_evidence: Some(evidence),
            ..
        } = &metadata.evidence
        {
            for entry in evidence.entries() {
                if let crate::ContributionFamilyFootprint::Component(component) = entry.footprint()
                    && component.owner() != owner
                {
                    return Err(invalid("family projection evidence has a different owner"));
                }
                if entry.fields_for_value(owner, &value).is_none() {
                    return Err(invalid(
                        "family projection evidence has an unproven field scope",
                    ));
                }
            }
        }
        if self.projections.contains_key(&(target, owner)) {
            return Err(invalid(
                "family projection already contains this target and owner",
            ));
        }
        self.projections.insert((target, owner), (value, metadata));
        Ok(())
    }

    pub(crate) fn into_projected(
        mut self,
        capture: &PreparedOutputFrame,
    ) -> Result<
        (
            crate::ResolvedAttributes,
            OutputContinuityState,
            Option<crate::PreparedFrameGeometry>,
            crate::native_position_projection::NativePositionProjection,
        ),
        EngineError,
    > {
        if !Arc::ptr_eq(&self.capture_identity, &capture.identity) {
            return Err(EngineError::StalePreparedFrame);
        }
        let frame = self
            .resolved
            .frame
            .as_mut()
            .expect("prepared static resolution retains its dense frame");
        for ((target, owner), (value, metadata)) in self.projections {
            let applied = frame.project_family(target, owner.key_ref(), value, metadata);
            debug_assert!(applied, "validated immutable baseline remains present");
        }
        Ok((
            self.resolved,
            self.continuity,
            self.geometry,
            self.position_native,
        ))
    }
}

impl Engine {
    /// Resolve ordinary static inputs and legacy batches exactly once. This is a speculative
    /// pre-Freeze lane; neither source inspection nor later family writes mutate Live history.
    pub fn prepare_static_family_frame(
        &self,
        capture: &PreparedOutputFrame,
        sampled: &[ContributionBatch],
    ) -> PreparedStaticFamilyFrame {
        self.prepare_static_family_frame_with_trace(capture, sampled, true)
    }

    pub(crate) fn prepare_static_family_frame_with_trace(
        &self,
        capture: &PreparedOutputFrame,
        sampled: &[ContributionBatch],
        trace_sources: bool,
    ) -> PreparedStaticFamilyFrame {
        let mut continuity = capture.continuity.clone();
        let mut resolved = crate::timed(crate::RenderPhase::ResolveTotal, || {
            self.resolve_prepared_attributes(capture, sampled, &mut continuity, trace_sources)
        });
        // Freeze holds the lamp's parameters before they become DMX (2026-10-05): family adapters
        // (Color, Zoom, Focus) read this baseline, so a frozen owner's adapter renders its frozen
        // value. Reapplying the Freeze in the final projection is idempotent.
        if !capture.freezes_nothing() {
            crate::timed(crate::RenderPhase::FixtureFreezes, || {
                crate::render::apply_fixture_freezes(
                    &capture.generation.snapshot().fixtures,
                    &mut resolved,
                )
            });
        }
        PreparedStaticFamilyFrame {
            capture_identity: Arc::clone(&capture.identity),
            preload: None,
            resolved,
            continuity,
            geometry: None,
            projections: Default::default(),
            position_native: Default::default(),
            native_raw: Default::default(),
        }
    }

    /// Consume a composed lane, then commit its saved continuity only after final projection.
    /// Freeze and all captured output overlays still run in their ordinary final position.
    pub fn render_static_family_frame(
        &self,
        capture: &PreparedOutputFrame,
        frame: PreparedStaticFamilyFrame,
    ) -> Result<RenderResult, EngineError> {
        crate::timed(crate::RenderPhase::RenderTotal, || {
            self.finish_static_family_frame(capture, frame)
        })
    }

    pub(crate) fn finish_static_family_frame(
        &self,
        capture: &PreparedOutputFrame,
        frame: PreparedStaticFamilyFrame,
    ) -> Result<RenderResult, EngineError> {
        if frame.preload.is_some() {
            return Err(EngineError::Invalid(
                "a Preload family token requires isolated Preload finalization".into(),
            ));
        }
        let (resolved, mut continuity, geometry, native) = frame.into_projected(capture)?;
        let result = self.project_resolved_frame(
            &capture.generation,
            capture.sampled_at,
            resolved,
            &capture.overlays,
            Arc::clone(&capture.tracked),
            &mut continuity.mounts,
            geometry,
            &native,
        )?;
        if !self.commit_output_continuity_if_unchanged(capture.continuity_revision, continuity) {
            return Err(EngineError::StalePreparedFrame);
        }
        Ok(result)
    }

    /// Consume and project an isolated lane with no Live continuity commit or source resampling.
    pub fn preview_static_family_frame(
        &self,
        capture: &PreparedOutputFrame,
        frame: PreparedStaticFamilyFrame,
    ) -> Result<RenderResult, EngineError> {
        if frame.preload.is_some() {
            return Err(EngineError::Invalid(
                "a Preload family token requires isolated Preload finalization".into(),
            ));
        }
        let (resolved, mut continuity, geometry, native) = frame.into_projected(capture)?;
        self.project_resolved_frame(
            &capture.generation,
            capture.sampled_at,
            resolved,
            &capture.overlays,
            Arc::clone(&capture.tracked),
            &mut continuity.mounts,
            geometry,
            &native,
        )
    }
}
