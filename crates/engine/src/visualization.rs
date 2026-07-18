use crate::{Engine, EngineError, RenderOptions, resolve_profile_head};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use std::collections::HashMap;

impl Engine {
    /// Returns the same merged abstract attributes that feed DMX rendering. Consumers such as
    /// visualizers can use this without attempting to reverse fixture-specific DMX encoding.
    pub fn resolved_values(&self) -> HashMap<(FixtureId, AttributeKey), AttributeValue> {
        let snapshot = self.snapshot.load_full();
        self.resolved_attributes_at(&snapshot, self.clock.now())
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
        let snapshot = self.snapshot.load_full();
        let mut resolved = self.resolved_attributes_at(&snapshot, self.clock.now());
        for (key, value) in values {
            if resolved.values.get(key) != Some(value) {
                // Visualization-only overrides (notably Preload) do not inherit the sequence
                // master of the source they temporarily replace.
                resolved.sequence_masters.remove(key);
            }
        }
        resolved.values.clone_from(values);
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
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
            for head_index in 0..mode.heads.len() {
                let output = resolve_profile_head(
                    fixture,
                    mode,
                    head_index,
                    &resolved,
                    options,
                    &groups,
                    &group_master_flashes,
                    &highlighted_fixtures,
                )?;
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
