use crate::{
    AxisInversion, ContributionBatch, Engine, EngineError, RenderOptions, resolve_profile_fixture,
};
use light_core::{AttributeKey, AttributeValue};

pub struct ProfileVisualizationProjection {
    pub values: crate::ResolvedValues,
    pub physical: std::sync::Arc<crate::Pooled<crate::PhysicalForwardFrame>>,
    pub points: std::sync::Arc<crate::Pooled<Vec<crate::ResolvedPointPose>>>,
    pub native_ownership: std::collections::HashMap<light_core::FixtureId, Box<[bool]>>,
}

/// An explicitly traced resolution, retaining the generation and dense master/source metadata.
/// It is observational and is never used as persisted programming intent.
pub struct ObservedSourceFrame {
    pub(crate) capture_identity: std::sync::Arc<()>,
    pub(crate) preload: Option<crate::preload_frame::PreloadTokenIdentity>,
    pub(crate) overlays: std::sync::Arc<crate::prepared_frame::CapturedOutputOverlays>,
    pub(crate) generation: std::sync::Arc<crate::RuntimeGeneration>,
    pub(crate) sampled_at: chrono::DateTime<chrono::Utc>,
    pub(crate) values: crate::FrameValues,
    pub(crate) points: std::sync::Arc<crate::Pooled<Vec<crate::ResolvedPointPose>>>,
    pub(crate) mounts: std::sync::Arc<crate::Pooled<crate::FixtureMountFrame>>,
    pub(crate) position_native: crate::native_position_projection::NativePositionProjection,
}

impl ObservedSourceFrame {
    pub fn values(&self) -> &crate::FrameValues {
        &self.values
    }
    pub fn snapshot(&self) -> std::sync::Arc<crate::EngineSnapshot> {
        self.generation.snapshot_arc()
    }
    pub fn sampled_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.sampled_at
    }
    /// Point poses from this same captured generation and value frame. Holding the source frame
    /// keeps both the dense values and the pooled Point projection immutable across later edits.
    pub fn points(&self) -> &[crate::ResolvedPointPose] {
        self.points.as_slice()
    }
    /// Body transforms from these final Point poses. This capture never advances Live history.
    pub fn mounts(&self) -> &crate::FixtureMountFrame {
        &self.mounts
    }
}

impl Engine {
    /// Capture source provenance for an explicit observer. Ordinary output does not allocate
    /// these source records. Coherent before/after preview supplies its pinned inputs separately.
    pub fn observe_source_frame(&self, sampled: &[ContributionBatch]) -> ObservedSourceFrame {
        let frame = self.prepare_observer_frame(RenderOptions::default());
        self.observe_prepared_frame(&frame, sampled)
    }

    pub fn observe_prepared_frame(
        &self,
        frame: &crate::PreparedOutputFrame,
        sampled: &[ContributionBatch],
    ) -> ObservedSourceFrame {
        let mut continuity = frame.continuity.clone();
        self.observe_prepared_lane(
            frame,
            sampled,
            &mut continuity,
            crate::resolution::ProgrammerLaneInputs {
                playback: &frame.playback,
                states: &frame.programmer.output_states,
                releases: &frame.releases,
                addresses: &self.programmer_addresses,
            },
        )
    }

    /// Scalar Current and source diagnostics need captured values, but not preliminary Point
    /// or mount solves. Keep the ordinary scalar Freeze semantics and evidence while leaving
    /// geometry to the final scalar-resolved token. This observation never commits continuity.
    pub fn observe_prepared_values(
        &self,
        frame: &crate::PreparedOutputFrame,
        sampled: &[ContributionBatch],
    ) -> crate::FrameValues {
        let mut continuity = frame.continuity.clone();
        self.observe_prepared_lane_values(
            frame,
            sampled,
            &mut continuity,
            crate::resolution::ProgrammerLaneInputs {
                playback: &frame.playback,
                states: &frame.programmer.output_states,
                releases: &frame.releases,
                addresses: &self.programmer_addresses,
            },
        )
    }

    pub(crate) fn observe_prepared_lane_values(
        &self,
        frame: &crate::PreparedOutputFrame,
        sampled: &[ContributionBatch],
        continuity: &mut crate::OutputContinuityState,
        lane: crate::resolution::ProgrammerLaneInputs<'_>,
    ) -> crate::FrameValues {
        let mut resolved =
            self.resolve_prepared_lane_attributes(frame, sampled, continuity, true, lane);
        crate::render::apply_fixture_freezes(&frame.generation.snapshot().fixtures, &mut resolved);
        resolved.named_values()
    }

    pub(crate) fn observe_prepared_lane(
        &self,
        frame: &crate::PreparedOutputFrame,
        sampled: &[ContributionBatch],
        continuity: &mut crate::OutputContinuityState,
        lane: crate::resolution::ProgrammerLaneInputs<'_>,
    ) -> ObservedSourceFrame {
        let resolved =
            self.resolve_prepared_lane_attributes(frame, sampled, continuity, true, lane);
        self.observe_resolved_prepared_frame(frame, resolved, continuity, None, Default::default())
    }

    /// Freeze an already composed static token and retain its geometry without another source
    /// evaluation. Typed-family writes cannot change Point axes, so cached geometry stays exact.
    pub(crate) fn observe_resolved_prepared_frame(
        &self,
        frame: &crate::PreparedOutputFrame,
        mut resolved: crate::ResolvedAttributes,
        continuity: &mut crate::OutputContinuityState,
        geometry: Option<crate::PreparedFrameGeometry>,
        position_native: crate::native_position_projection::NativePositionProjection,
    ) -> ObservedSourceFrame {
        crate::render::apply_fixture_freezes(&frame.generation.snapshot().fixtures, &mut resolved);
        let values = resolved.named_values();
        let (points, mounts) = match geometry {
            Some(geometry) => (geometry.points, geometry.mounts),
            None => {
                let points =
                    std::sync::Arc::new(frame.generation.point_projection().resolve(&values));
                let mounts = frame
                    .generation
                    .mount_projection()
                    .resolve(&points, &mut continuity.mounts);
                (points, mounts)
            }
        };
        ObservedSourceFrame {
            position_native,
            capture_identity: std::sync::Arc::clone(&frame.identity),
            preload: None,
            overlays: std::sync::Arc::clone(&frame.overlays),
            values,
            points,
            mounts,
            generation: std::sync::Arc::clone(&frame.generation),
            sampled_at: frame.sampled_at,
        }
    }

    /// Project the captured frame directly: Cue master provenance never depends on comparing
    /// the value with a separately sampled live value.
    pub fn profile_observed_source_frame(
        &self,
        frame: &ObservedSourceFrame,
        options: RenderOptions,
        previewed: &std::collections::HashSet<(light_core::FixtureId, AttributeKey)>,
    ) -> Result<ProfileVisualizationProjection, EngineError> {
        self.profile_frozen_frame_projection(
            &frame.generation,
            &frame.values,
            &Default::default(),
            options,
            previewed,
            Some(&frame.overlays),
            Some(std::sync::Arc::clone(&frame.points)),
            &frame.position_native,
        )
    }

    /// Compute explicit Color Release ownership from two resolutions of the same captured
    /// generation/time. The caller must supply the same source-state capture to both branches;
    /// this primitive never reconstructs a source by comparing values. `other_previewed` is for
    /// independent pending Set operations, not a list of all addressed Release instructions.
    pub fn profile_color_release_frames(
        &self,
        before: &ObservedSourceFrame,
        after: &ObservedSourceFrame,
        releases: &ContributionBatch,
        options: RenderOptions,
        other_previewed: &std::collections::HashSet<(light_core::FixtureId, AttributeKey)>,
    ) -> Result<ProfileVisualizationProjection, EngineError> {
        if !std::sync::Arc::ptr_eq(&before.capture_identity, &after.capture_identity)
            || !std::sync::Arc::ptr_eq(&before.generation, &after.generation)
            || before.sampled_at != after.sampled_at
            || !match (&before.preload, &after.preload) {
                (None, None) => true,
                (Some(before), Some(after)) => {
                    crate::preload_frame::PreloadTokenIdentity::release_pair(before, after)
                }
                _ => false,
            }
        {
            return Err(EngineError::Invalid(
                "Color Release preview requires one captured generation and time".into(),
            ));
        }
        let released = releases
            .excluded_addresses()
            .filter(|(_, attribute)| attribute.0.as_ref() == "color")
            .filter_map(|(fixture, attribute)| {
                let origin = before.values.contribution_origin(fixture, attribute)?;
                releases
                    .excludes_origin(
                        origin.source(),
                        fixture,
                        attribute,
                        origin.stamp().changed_at,
                        origin.stamp().programmer_order,
                    )
                    .then(|| (fixture, attribute.clone()))
            })
            .collect::<std::collections::HashSet<_>>();
        let mut destination_keys = other_previewed.clone();
        destination_keys.extend(released.iter().cloned());
        let mut destination =
            self.profile_observed_source_frame(after, options, &destination_keys)?;
        if released.is_empty() {
            return Ok(destination);
        }
        let original = self.profile_observed_source_frame(before, options, &released)?;
        for (fixture, old) in original.native_ownership {
            match destination.native_ownership.entry(fixture) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(old);
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    let current = entry.get_mut();
                    if current.len() != old.len() {
                        return Err(EngineError::Invalid(
                            "Color Release native layout changed during projection".into(),
                        ));
                    }
                    for (current, old) in current.iter_mut().zip(old.iter()) {
                        *current |= old;
                    }
                }
            }
        }
        Ok(destination)
    }
    /// Returns the same merged abstract attributes that feed DMX rendering. Consumers such as
    /// visualizers can use this without attempting to reverse fixture-specific DMX encoding.
    pub fn resolved_values(&self) -> crate::ResolvedValues {
        self.resolved_values_with_contribution_batches(&[])
    }

    /// Resolve externally sampled values through ordinary semantic arbitration without rendering.
    pub fn resolved_values_with_contribution_batches(
        &self,
        sampled: &[ContributionBatch],
    ) -> crate::ResolvedValues {
        let generation = self.generation.load_full();
        // This caller wants every value by name, so the frame is materialised here rather than
        // left dense. Nothing on the output path takes this route.
        self.resolved_attributes_at(&generation, self.clock.now(), sampled)
            .named_values()
            .values()
            .clone()
    }

    /// Project schema-v2 profile heads through the same channel-resolution path used for DMX.
    /// The returned intensity and XYZ color therefore include Highlight/Blackout, calibrated
    /// gamut clipping, response curves, virtual intensity, and applicable masters exactly once.
    /// `values` may include temporary visualization-only overrides such as Preload.
    pub fn profile_visualization_values(
        &self,
        values: &crate::ResolvedValues,
        options: RenderOptions,
    ) -> Result<crate::ResolvedValues, EngineError> {
        Ok(self
            .profile_visualization_projection(values, options)?
            .values)
    }

    /// Normal and Preload use the same per-instance final-native forward calculation.
    pub fn profile_visualization_projection(
        &self,
        values: &crate::ResolvedValues,
        options: RenderOptions,
    ) -> Result<ProfileVisualizationProjection, EngineError> {
        self.profile_visualization_projection_at(
            values,
            options,
            None,
            &std::collections::HashSet::new(),
        )
    }

    /// The expected snapshot must be the exact generation used to compose observational values.
    pub fn profile_visualization_projection_at(
        &self,
        values: &crate::ResolvedValues,
        options: RenderOptions,
        expected: Option<&std::sync::Arc<crate::EngineSnapshot>>,
        overridden: &std::collections::HashSet<(light_core::FixtureId, light_core::AttributeKey)>,
    ) -> Result<ProfileVisualizationProjection, EngineError> {
        self.profile_preload_projection_at(
            values,
            options,
            expected,
            overridden,
            &Default::default(),
        )
    }

    /// Preview ownership includes Off/Release, while `overridden` only removes active masters.
    /// Channel aliases are compiled; Color ownership records actual resolver writes, so Direct
    /// never claims unrelated wheels or UV and same-value Intent edits retain their ownership.
    pub fn profile_preload_projection_at(
        &self,
        values: &crate::ResolvedValues,
        options: RenderOptions,
        expected: Option<&std::sync::Arc<crate::EngineSnapshot>>,
        overridden: &std::collections::HashSet<(light_core::FixtureId, AttributeKey)>,
        previewed: &std::collections::HashSet<(light_core::FixtureId, AttributeKey)>,
    ) -> Result<ProfileVisualizationProjection, EngineError> {
        let generation = self.generation.load_full();
        if expected
            .is_some_and(|snapshot| !std::sync::Arc::ptr_eq(snapshot, &generation.snapshot_arc()))
        {
            return Err(EngineError::Invalid(
                "visualization generation changed during projection".into(),
            ));
        }
        let snapshot = generation.snapshot();
        let mut effective_preview = previewed.clone();
        for fixture in snapshot.fixtures.iter() {
            for (owner, frozen) in &fixture.freeze.targets {
                for key in frozen.values.keys() {
                    effective_preview.remove(&(*owner, key.clone()));
                }
            }
        }
        let previewed = &effective_preview;
        let mut resolved = self.resolved_attributes_at(&generation, self.clock.now(), &[]);
        // A dense resolution intentionally leaves the named map empty. Read the actual frame
        // before replacing it, and carry its masters explicitly into the map-based projection.
        let original = resolved.named_values();
        for (key, value) in values {
            if !overridden.contains(key) && original.value(key.0, &key.1) == Some(value) {
                if let Some(master) = original.frame().and_then(|frame| {
                    frame
                        .slots()
                        .slot(key.0, &key.1)
                        .and_then(|slot| frame.sequence_master(slot))
                }) {
                    resolved.sequence_masters.insert(key.clone(), master);
                }
            } else {
                resolved.sequence_masters.remove(key);
            }
        }
        resolved.values.clone_from(values);
        // These values were assembled elsewhere and replace the frame wholesale, so the frame is
        // released rather than read: it no longer describes what is being projected.
        resolved.frame = None;
        crate::render::apply_fixture_freezes(&snapshot.fixtures, &mut resolved);
        let sequence_masters = std::mem::take(&mut resolved.sequence_masters);
        let named_values = resolved.named_values();
        self.profile_frozen_frame_projection(
            &generation,
            &named_values,
            &sequence_masters,
            options,
            previewed,
            None,
            None,
            &Default::default(),
        )
    }

    fn profile_frozen_frame_projection(
        &self,
        generation: &crate::RuntimeGeneration,
        named_values: &crate::FrameValues,
        sequence_masters: &rustc_hash::FxHashMap<
            (light_core::FixtureId, AttributeKey),
            crate::contribution::ApplicableSequenceMaster,
        >,
        options: RenderOptions,
        previewed: &std::collections::HashSet<(light_core::FixtureId, AttributeKey)>,
        captured: Option<&crate::prepared_frame::CapturedOutputOverlays>,
        points: Option<std::sync::Arc<crate::Pooled<Vec<crate::ResolvedPointPose>>>>,
        position_native: &crate::native_position_projection::NativePositionProjection,
    ) -> Result<ProfileVisualizationProjection, EngineError> {
        let snapshot = generation.snapshot();
        let points = points.unwrap_or_else(|| {
            std::sync::Arc::new(generation.point_projection().resolve(named_values))
        });
        let mut effective_preview = previewed.clone();
        for fixture in snapshot.fixtures.iter() {
            for (owner, frozen) in &fixture.freeze.targets {
                for key in frozen.values.keys() {
                    effective_preview.remove(&(*owner, key.clone()));
                }
            }
        }
        let previewed = &effective_preview;
        let profile_values = crate::ProfileValueIndex::new(
            named_values,
            sequence_masters,
            generation.channel_slots(),
        );
        let group_masters = generation.group_masters();
        let fresh;
        let overlays = match captured {
            Some(overlays) => overlays,
            None => {
                fresh = self.capture_output_overlays(options);
                &fresh
            }
        };
        let group_master_flashes = &overlays.flashes;
        let highlight_layers = &overlays.highlights;
        let highlight_look = &overlays.highlight_look;
        let options = RenderOptions {
            color_model: overlays.options.color_model,
            ..options
        };
        let mut projected = crate::ResolvedValues::default();
        let mut output = self.profile_scratch_pool.take();
        output.track_color_writes = !previewed.is_empty();
        let mut native_ownership = std::collections::HashMap::new();
        let mut physical = generation.physical_projection().take_frame();
        physical.bind_generation(generation.identity());
        for fixture in snapshot.fixtures.iter() {
            let Some(profile) = fixture.definition.profile_snapshot.as_deref() else {
                continue;
            };
            let mode_id = fixture.definition.mode_id.ok_or_else(|| {
                EngineError::Invalid("schema-v2 fixture is missing its mode identity".into())
            })?;
            let mode = profile
                .mode(mode_id)
                .ok_or_else(|| EngineError::Invalid("schema-v2 fixture mode is missing".into()))?;
            let projection = generation
                .profile_projection(fixture.fixture_id)
                .ok_or_else(|| {
                    EngineError::Invalid("schema-v2 fixture projection plan is missing".into())
                })?;
            resolve_profile_fixture(
                fixture,
                mode,
                projection,
                None,
                &profile_values,
                options,
                group_masters,
                &group_master_flashes,
                &highlight_layers,
                &highlight_look,
                if profile.patch_policy == light_fixture::PatchPolicy::Dmx {
                    AxisInversion {
                        pan: fixture.invert_pan,
                        tilt: fixture.invert_tilt,
                    }
                } else {
                    AxisInversion::default()
                },
                fixture.fixture_id.0,
                position_native.instance(fixture.fixture_id, fixture.fixture_id.0),
                &mut output,
            )?;
            if let Some(mask) = position_native.preview_ownership(
                projection,
                fixture.fixture_id,
                previewed,
                &output,
                named_values.values(),
            ) {
                native_ownership.insert(fixture.fixture_id, mask);
            }
            generation.physical_projection().evaluate(
                fixture.fixture_id,
                0,
                &output.channels,
                &mut physical,
            )?;
            for output in &output.heads {
                projected.insert(
                    (output.owner, AttributeKey::intensity()),
                    AttributeValue::Normalized(output.intensity),
                );
                if let Some(color) = output.color {
                    projected.insert(
                        (output.owner, AttributeKey("color".into())),
                        AttributeValue::ColorXyz(color),
                    );
                }
            }
            for (index, copy) in fixture.multipatch.iter().enumerate() {
                resolve_profile_fixture(
                    fixture,
                    mode,
                    projection,
                    None,
                    &profile_values,
                    options,
                    group_masters,
                    &group_master_flashes,
                    &highlight_layers,
                    &highlight_look,
                    if profile.patch_policy == light_fixture::PatchPolicy::Dmx {
                        AxisInversion {
                            pan: copy.invert_pan,
                            tilt: copy.invert_tilt,
                        }
                    } else {
                        AxisInversion::default()
                    },
                    copy.id,
                    position_native.instance(fixture.fixture_id, copy.id),
                    &mut output,
                )?;
                generation.physical_projection().evaluate(
                    fixture.fixture_id,
                    index + 1,
                    &output.channels,
                    &mut physical,
                )?;
            }
        }
        Ok(ProfileVisualizationProjection {
            values: projected,
            native_ownership,
            physical: std::sync::Arc::new(physical),
            points,
        })
    }
}
