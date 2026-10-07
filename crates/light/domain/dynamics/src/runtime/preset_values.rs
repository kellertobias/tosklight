use super::*;
use crate::{
    DynamicPresetSourceBinding, DynamicPresetSourceValues, DynamicValue, DynamicValueAddress,
    DynamicValueSourceResolver,
};
use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub(super) struct PresetValues {
    pub retained: Vec<DynamicPresetSourceValues>,
    pub by_binding: Arc<HashMap<(Uuid, FixtureId), DynamicValue>>,
    // Runtime-only freshness. Serialized last-valid values are recovery input, not proof
    // that the current Group/native dependencies have been compiled.
    pub prepared_generation: Option<Uuid>,
}

/// Compiled per-instance Preset tables awaiting one atomic manifest check and publication.
///
/// This token retains only expected instance/dependency/source identities, ordered targets
/// and the compiled tables. It carries no clocks, controllers, history or native provider.
/// Install on the runtime whose manifests were captured; intervening dependency changes make
/// the whole token stale. Advancing or pausing that runtime does not invalidate the tables.
#[derive(Debug)]
#[must_use = "prepared Preset tables must be installed to affect the runtime"]
pub struct PreparedDynamicPresetSources {
    entries: Vec<PreparedPresetValues>,
}

#[derive(Debug)]
struct PreparedPresetValues {
    instance_id: Uuid,
    dependency_generation: Uuid,
    ordered_targets: Vec<FixtureId>,
    source_ids: Vec<Uuid>,
    values: PresetValues,
}

impl PresetValues {
    pub fn compile(
        retained: Vec<DynamicPresetSourceValues>,
        lanes: &ProgrammingLanes,
        targets: &[FixtureId],
        strict: bool,
    ) -> Result<Self, DynamicRuntimeError> {
        let fail = |message: &str| DynamicRuntimeError::InvalidSnapshot(message.into());
        let targets = targets.iter().copied().collect::<HashSet<_>>();
        let mut seen = HashSet::new();
        let mut values = HashMap::new();
        let mut kept = Vec::new();
        for mut record in retained {
            if !seen.insert(record.occurrence) {
                return Err(fail("duplicate retained Dynamic Preset source occurrence"));
            }
            let Some(lane) = lanes.get(&record.occurrence.lane_id) else {
                if strict {
                    return Err(fail("retained Dynamic Preset lane is absent"));
                }
                continue;
            };
            let mut binding = None;
            lane.visit_preset_sources(|source| {
                if record.matches(source) {
                    binding = Some(Arc::clone(source));
                }
            });
            let Some(binding) = binding else {
                if strict {
                    return Err(fail("retained Dynamic Preset source identity changed"));
                }
                continue;
            };
            let mut seen_targets = HashSet::new();
            for fallback in &record.values {
                if !seen_targets.insert(fallback.target) {
                    return Err(fail("duplicate retained Dynamic Preset target"));
                }
                if strict && !targets.contains(&fallback.target) {
                    return Err(fail(
                        "retained Dynamic Preset target exceeds instance scope",
                    ));
                }
                lane.address()
                    .validate_source_value(&fallback.value)
                    .map_err(|error| fail(&error.to_string()))?;
            }
            record
                .values
                .retain(|fallback| targets.contains(&fallback.target));
            for fallback in &record.values {
                values.insert((binding.id, fallback.target), fallback.value.clone());
            }
            kept.push(record);
        }
        Ok(Self {
            retained: kept,
            by_binding: Arc::new(values),
            prepared_generation: None,
        })
    }
}

impl DynamicInstance {
    fn matches_preset_source_manifest(
        &self,
        dependency_generation: Uuid,
        ordered_targets: &[FixtureId],
        mut source_ids: impl Iterator<Item = Uuid>,
    ) -> bool {
        if self.preset_dependency_generation != dependency_generation
            || self.targets != ordered_targets
        {
            return false;
        }
        let mut matches = true;
        for lane in &self.definition.lanes {
            if let Some(compiled) = self.programming_lanes.get(&lane.id) {
                compiled.visit_preset_sources(|source| {
                    matches &= source_ids.next() == Some(source.id);
                });
            }
        }
        matches && source_ids.next().is_none()
    }

    pub(super) fn rebind_preset_values(&mut self) {
        // Previously validated records remain valid only for the same exact address/source.
        // Native source identities are immutable. A replacement address drops its old record.
        self.preset_values = PresetValues::compile(
            std::mem::take(&mut self.preset_values.retained),
            &self.programming_lanes,
            &self.targets,
            false,
        )
        .expect("retained source records were validated against their immutable source");
    }
}

impl DynamicRuntime {
    /// Compile every table without mutating the runtime. Duplicate instance entries are
    /// invalid even if a manifest is stale. Otherwise any stale manifest returns `Ok(None)`
    /// before payload validation; only a wholly current batch is compiled. This precedence
    /// is independent of input order and never validates values against replacement lanes.
    /// An empty batch prepares successfully. External dependency edits must invalidate their
    /// manifests before preparation; installing tables alone does not change generations.
    pub fn prepare_preset_source_values(
        &self,
        values: impl IntoIterator<Item = (DynamicInstancePresetSources, Vec<DynamicPresetSourceValues>)>,
    ) -> Result<Option<PreparedDynamicPresetSources>, DynamicRuntimeError> {
        let values = values.into_iter().collect::<Vec<_>>();
        let mut seen = HashSet::with_capacity(values.len());
        for (expected, _) in &values {
            if !seen.insert(expected.instance_id) {
                return Err(DynamicRuntimeError::InvalidSnapshot(
                    "duplicate instance in Dynamic Preset source batch".into(),
                ));
            }
        }
        if !values.iter().all(|(expected, _)| {
            self.instances
                .get(&expected.instance_id)
                .is_some_and(|instance| {
                    instance.matches_preset_source_manifest(
                        expected.dependency_generation,
                        &expected.ordered_targets,
                        expected.sources.iter().map(|source| source.id),
                    )
                })
        }) {
            return Ok(None);
        }
        let mut entries = Vec::with_capacity(values.len());
        for (expected, values) in values {
            let instance = &self.instances[&expected.instance_id];
            let values = PresetValues::compile(
                values,
                &instance.programming_lanes,
                &instance.targets,
                true,
            )?;
            entries.push(PreparedPresetValues {
                instance_id: expected.instance_id,
                dependency_generation: expected.dependency_generation,
                ordered_targets: expected.ordered_targets,
                source_ids: expected.sources.iter().map(|source| source.id).collect(),
                values,
            });
        }
        Ok(Some(PreparedDynamicPresetSources { entries }))
    }

    /// Publish all compiled tables, or none if any manifest changed since preparation. The
    /// complete preflight precedes journaling and mutation; successful publication participates
    /// in an enclosing output transaction and preserves its latest clocks and held history.
    pub fn install_prepared_preset_source_values(
        &mut self,
        prepared: PreparedDynamicPresetSources,
    ) -> bool {
        if !prepared.entries.iter().all(|entry| {
            self.instances
                .get(&entry.instance_id)
                .is_some_and(|instance| {
                    instance.matches_preset_source_manifest(
                        entry.dependency_generation,
                        &entry.ordered_targets,
                        entry.source_ids.iter().copied(),
                    )
                })
        }) {
            return false;
        }
        for entry in prepared.entries {
            let instance = self
                .instances
                .get_mut(&entry.instance_id)
                .expect("validated Preset instance remains present during publication");
            if let Some(undo) = &mut self.output_frame_undo {
                undo.record_instance(instance.id, Some(instance));
            }
            instance.preset_values = entry.values;
            instance.preset_values.prepared_generation = Some(entry.dependency_generation);
        }
        true
    }

    /// Call when an input outside the definition/target list changes (Group membership,
    /// spatial ranking or pinned source availability), before capturing a new cold manifest.
    /// The adapter supplies only affected instances; unrelated sources keep their generation.
    pub fn invalidate_preset_source_dependencies(&mut self, instance_id: Uuid) -> bool {
        let Some(instance) = self.instances.get_mut(&instance_id) else {
            return false;
        };
        if let Some(undo) = &mut self.output_frame_undo {
            undo.record_instance(instance.id, Some(instance));
        }
        instance.preset_dependency_generation = Uuid::new_v4();
        true
    }
    /// Publish one cold-compiled dependency set only if its captured target/source generations
    /// still match. This never changes clocks, lane masks or programmer state.
    pub fn install_preset_source_values(
        &mut self,
        expected: &DynamicInstancePresetSources,
        values: Vec<DynamicPresetSourceValues>,
    ) -> Result<bool, DynamicRuntimeError> {
        let Some(instance) = self.instances.get_mut(&expected.instance_id) else {
            return Ok(false);
        };
        if !instance.matches_preset_source_manifest(
            expected.dependency_generation,
            &expected.ordered_targets,
            expected.sources.iter().map(|source| source.id),
        ) {
            return Ok(false);
        }
        let compiled =
            PresetValues::compile(values, &instance.programming_lanes, &instance.targets, true)?;
        if let Some(undo) = &mut self.output_frame_undo {
            undo.record_instance(instance.id, Some(instance));
        }
        instance.preset_values = compiled;
        instance.preset_values.prepared_generation = Some(expected.dependency_generation);
        Ok(true)
    }
}

/// The immutable per-instance fallback table avoids a source scan or retained-state mutation
/// inside the sampler. The live frame's cold-compiled source wins when supplied.
pub(super) struct RetainedPresetSources<'a> {
    pub current: &'a dyn DynamicValueSourceResolver,
    pub instance_id: Uuid,
    pub values: Arc<HashMap<(Uuid, FixtureId), DynamicValue>>,
}

impl DynamicValueSourceResolver for RetainedPresetSources<'_> {
    fn try_position_current_family(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<light_core::AttributeValue>, light_core::programming::TransitionError> {
        self.current.try_position_current_family(target, address)
    }
    fn try_current(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<DynamicValue>, light_core::programming::TransitionError> {
        self.current.try_current(target, address)
    }
    fn try_current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<light_core::AttributeValue>, light_core::programming::TransitionError> {
        self.current.try_current_family_base(target, address)
    }
    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.current.current(target, address)
    }
    fn authored_occurrence(
        &self,
        instance_id: Uuid,
        controller_id: Uuid,
        target: FixtureId,
        lane_id: Uuid,
    ) -> Option<crate::DynamicSourceOccurrenceId> {
        self.current
            .authored_occurrence(instance_id, controller_id, target, lane_id)
    }
    fn current_family_occurrence(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<crate::DynamicSourceOccurrenceId> {
        self.current.current_family_occurrence(target, address)
    }
    fn current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<light_core::AttributeValue> {
        self.current.current_family_base(target, address)
    }
    fn current_dependency(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> crate::DynamicSourceDependency {
        self.current.current_dependency(target, address)
    }
    fn preset(
        &self,
        source: &DynamicPresetSourceBinding,
        instance_id: Uuid,
        target: FixtureId,
    ) -> Option<DynamicValue> {
        self.current
            .preset(source, instance_id, target)
            .or_else(|| {
                (instance_id == self.instance_id)
                    .then(|| self.values.get(&(source.id, target)).cloned())
                    .flatten()
            })
    }
}
