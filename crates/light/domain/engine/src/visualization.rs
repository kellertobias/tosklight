use crate::{
    AxisInversion, ContributionBatch, Engine, EngineError, RenderOptions, resolve_profile_fixture,
};
use light_core::{AttributeKey, AttributeValue};

impl Engine {
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
        let generation = self.generation.load_full();
        let snapshot = generation.snapshot();
        let mut resolved = self.resolved_attributes_at(&generation, self.clock.now(), &[]);
        for (key, value) in values {
            if resolved.values.get(key) != Some(value) {
                // Visualization-only overrides (notably Preload) do not inherit the sequence
                // master of the source they temporarily replace.
                resolved.sequence_masters.remove(key);
            }
        }
        resolved.values.clone_from(values);
        // These values were assembled elsewhere and replace the frame wholesale, so the frame is
        // released rather than read: it no longer describes what is being projected.
        resolved.frame = None;
        let sequence_masters = std::mem::take(&mut resolved.sequence_masters);
        let named_values = resolved.named_values();
        let profile_values = crate::ProfileValueIndex::new(
            &named_values,
            &sequence_masters,
            generation.channel_slots(),
        );
        let group_masters = generation.group_masters();
        let group_master_flashes = self.group_master_flashes.read();
        let highlight_layers = self.highlight_layers.read();
        let highlight_look = self.highlight_look.read();
        let mut projected = crate::ResolvedValues::default();
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
            let output = resolve_profile_fixture(
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
                AxisInversion {
                    pan: fixture.invert_pan,
                    tilt: fixture.invert_tilt,
                },
            )?;
            for output in output.heads {
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
        }
        Ok(projected)
    }
}
