use crate::{ContributionBatch, Engine, EngineError, RenderOptions, resolve_profile_fixture};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use std::collections::HashMap;

impl Engine {
    /// Returns the same merged abstract attributes that feed DMX rendering. Consumers such as
    /// visualizers can use this without attempting to reverse fixture-specific DMX encoding.
    pub fn resolved_values(&self) -> HashMap<(FixtureId, AttributeKey), AttributeValue> {
        self.resolved_values_with_contribution_batches(&[])
    }

    /// Resolve externally sampled values through ordinary semantic arbitration without rendering.
    pub fn resolved_values_with_contribution_batches(
        &self,
        sampled: &[ContributionBatch],
    ) -> HashMap<(FixtureId, AttributeKey), AttributeValue> {
        let generation = self.generation.load_full();
        self.resolved_attributes_at(&generation, self.clock.now(), sampled)
            .values
    }

    /// Project schema-v2 profile heads through the same channel-resolution path used for DMX.
    /// The returned intensity and XYZ color therefore include Highlight/Blackout, calibrated
    /// gamut clipping, response curves, virtual intensity, and applicable masters exactly once.
    /// `values` may include temporary visualization-only overrides such as Preload.
    pub fn profile_visualization_values(
        &self,
        values: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
        options: RenderOptions,
    ) -> Result<HashMap<(FixtureId, AttributeKey), AttributeValue>, EngineError> {
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
        let profile_values = crate::ProfileValueIndex::new(&resolved);
        let group_masters = generation.group_masters();
        let group_master_flashes = self.group_master_flashes.read();
        let highlighted_fixtures = self.highlighted_fixtures.read();
        let mut projected = HashMap::new();
        for fixture in &snapshot.fixtures {
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
                &highlighted_fixtures,
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
