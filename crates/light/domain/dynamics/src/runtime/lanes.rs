use super::*;
use std::collections::HashSet;

/// All preserves historical Dynamic playback behavior. Recorded typed On rows
/// use explicit lane identities; live Groups can apply a uniform set to new members.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DynamicLaneSelection {
    #[default]
    All,
    Uniform {
        lanes: Vec<Uuid>,
    },
    PerTarget {
        targets: Vec<DynamicTargetLanes>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicTargetLanes {
    pub target: FixtureId,
    pub lanes: Vec<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicControllerLaneSelection {
    pub controller_id: Uuid,
    pub selection: DynamicLaneSelection,
}

impl DynamicLaneSelection {
    /// A recorded component selects its complete Angle bundle. Preserve the authored IDs
    /// separately so a later hot edit can recompile closure against the current definition.
    fn complete_angle_pair(&mut self, definition: &DynamicDefinition) {
        let angle_ids = definition
            .lanes
            .iter()
            .filter(|lane| lane.is_programming_angles())
            .map(|lane| lane.id)
            .collect::<Vec<_>>();
        let complete = |lanes: &mut Vec<Uuid>| {
            if lanes.iter().any(|id| angle_ids.contains(id)) {
                for id in &angle_ids {
                    if !lanes.contains(id) {
                        lanes.push(*id);
                    }
                }
                lanes.sort_unstable();
            }
        };
        match self {
            Self::All => {}
            Self::Uniform { lanes } => complete(lanes),
            Self::PerTarget { targets } => {
                for target in targets {
                    complete(&mut target.lanes);
                }
            }
        }
    }

    /// Legacy On rows used to start the entire definition even when only one
    /// attribute row survived storage. Retain that behavior until the definition
    /// becomes typed; then freeze the retained fallback's lane IDs explicitly.
    pub fn for_recorded_values(
        reference: &crate::DynamicReference,
        definition: &DynamicDefinition,
        rows: &[(FixtureId, Uuid)],
    ) -> Self {
        let legacy = reference
            .embedded_fallback
            .definition
            .required_programming_contract()
            == 0;
        if legacy && definition.required_programming_contract() == 0 {
            return Self::All;
        }
        let mut targets = HashMap::<FixtureId, HashSet<Uuid>>::new();
        for (target, lane) in rows {
            let selected = targets.entry(*target).or_default();
            if legacy {
                selected.extend(
                    reference
                        .embedded_fallback
                        .definition
                        .lanes
                        .iter()
                        .map(|lane| lane.id),
                );
            } else {
                selected.insert(*lane);
            }
        }
        if matches!(
            definition.target_binding,
            DynamicTargetBinding::LiveGroup { .. }
        ) {
            let mut lanes = targets
                .into_values()
                .flatten()
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            lanes.sort_unstable();
            Self::Uniform { lanes }
        } else {
            let mut targets = targets
                .into_iter()
                .map(|(target, lanes)| {
                    let mut lanes = lanes.into_iter().collect::<Vec<_>>();
                    lanes.sort_unstable();
                    DynamicTargetLanes { target, lanes }
                })
                .collect::<Vec<_>>();
            targets.sort_by_key(|target| target.target.0);
            Self::PerTarget { targets }
        }
    }
}

#[derive(Clone)]
pub(super) struct CompiledLaneSelection {
    authored: DynamicLaneSelection,
    /// Fx-hashed (TL-639 round 4): pinning asks once per target and lane each frame. Only
    /// membership is read; `authored` keeps the order.
    uniform: rustc_hash::FxHashSet<Uuid>,
    targets: rustc_hash::FxHashMap<FixtureId, rustc_hash::FxHashSet<Uuid>>,
}

impl CompiledLaneSelection {
    fn new(
        authored: DynamicLaneSelection,
        definition: &DynamicDefinition,
    ) -> Result<Self, DynamicRuntimeError> {
        let lanes = |ids: &[Uuid]| {
            let unique = ids.iter().copied().collect::<rustc_hash::FxHashSet<_>>();
            if unique.len() != ids.len() || ids.iter().any(Uuid::is_nil) {
                Err(DynamicRuntimeError::InvalidSnapshot(
                    "lane selection contains invalid or duplicate lane IDs".into(),
                ))
            } else {
                Ok(unique)
            }
        };
        let mut uniform = rustc_hash::FxHashSet::default();
        let mut targets = rustc_hash::FxHashMap::default();
        // Validate before closure; normalization must not hide malformed duplicate IDs.
        match &authored {
            DynamicLaneSelection::All => {}
            DynamicLaneSelection::Uniform { lanes: ids } => uniform = lanes(ids)?,
            DynamicLaneSelection::PerTarget { targets: selected } => {
                for target in selected {
                    if target.target.0.is_nil()
                        || targets
                            .insert(target.target, lanes(&target.lanes)?)
                            .is_some()
                    {
                        return Err(DynamicRuntimeError::InvalidSnapshot(
                            "lane selection contains invalid or duplicate targets".into(),
                        ));
                    }
                }
            }
        }
        let mut effective = authored.clone();
        effective.complete_angle_pair(definition);
        match effective {
            DynamicLaneSelection::All => {}
            DynamicLaneSelection::Uniform { lanes } => uniform.extend(lanes),
            DynamicLaneSelection::PerTarget { targets: selected } => {
                for target in selected {
                    targets
                        .get_mut(&target.target)
                        .expect("validated target")
                        .extend(target.lanes);
                }
            }
        }
        Ok(Self {
            authored,
            uniform,
            targets,
        })
    }

    pub(super) fn allows(&self, target: FixtureId, lane: Uuid) -> bool {
        match self.authored {
            DynamicLaneSelection::All => true,
            DynamicLaneSelection::Uniform { .. } => self.uniform.contains(&lane),
            DynamicLaneSelection::PerTarget { .. } => self
                .targets
                .get(&target)
                .is_some_and(|lanes| lanes.contains(&lane)),
        }
    }

    /// A normalized partner may disappear from the current definition while its
    /// original authored Angle source is held. Admit only that deterministic live
    /// partner identity, with an independently allowed retained authored source.
    pub(super) fn allows_retained(
        &self,
        definition: &DynamicDefinition,
        key: (Uuid, FixtureId, Uuid),
        expression: &DynamicSampleExpression,
        values: &SampleValueMap,
    ) -> bool {
        if self.allows(key.1, key.2) {
            return true;
        }
        fn partner(
            definition: &DynamicDefinition,
            lane: Uuid,
            expression: &DynamicSampleExpression,
        ) -> bool {
            use crate::programming::expression::{ExpressionNode, ExpressionNodeRef};
            ExpressionNodeRef::new(expression)
                .postorder(false)
                .is_ok_and(|nodes| {
                    nodes.into_iter().all(|node| match node.node() {
                        Ok(ExpressionNode::Current(address)) => {
                            address.representation == crate::DynamicFamilyRepresentation::Angles
                                && address.component.is_some_and(|component| {
                                    definition.is_automatic_angle_partner_id(lane, component)
                                })
                        }
                        Ok(ExpressionNode::Transition { .. }) => true,
                        _ => false,
                    })
                })
        }
        partner(definition, key.2, expression)
            && values.iter().any(|(other, value)| {
                other.0 == key.0
                    && other.1 == key.1
                    && other.2 != key.2
                    && self.allows(other.1, other.2)
                    && value.contains_angles()
            })
    }
}

impl DynamicInstance {
    pub(super) fn rebind_angle_lane_selections(&mut self) {
        for selection in self.lane_selections.values_mut() {
            *selection = CompiledLaneSelection::new(selection.authored.clone(), &self.definition)
                .expect("previously validated lane selection");
        }
    }

    pub(super) fn lane_is_active(&self, controller: Uuid, target: FixtureId, lane: Uuid) -> bool {
        self.lane_selections
            .get(&controller)
            .is_none_or(|selection| selection.allows(target, lane))
    }

    pub(super) fn lane_selection_snapshot(&self) -> Vec<DynamicControllerLaneSelection> {
        let mut selections = self
            .lane_selections
            .iter()
            .map(
                |(controller_id, selection)| DynamicControllerLaneSelection {
                    controller_id: *controller_id,
                    selection: selection.authored.clone(),
                },
            )
            .collect::<Vec<_>>();
        selections.sort_by_key(|selection| selection.controller_id);
        selections
    }
}

pub(super) fn restore_lane_selections(
    selections: Vec<DynamicControllerLaneSelection>,
    controllers: &HashMap<Uuid, DynamicController>,
    definition: &DynamicDefinition,
    supported: u16,
) -> Result<HashMap<Uuid, CompiledLaneSelection>, DynamicRuntimeError> {
    let mut restored = HashMap::new();
    for selection in selections {
        validate_selection_support(&selection.selection, supported)?;
        if !controllers.contains_key(&selection.controller_id)
            || restored
                .insert(
                    selection.controller_id,
                    CompiledLaneSelection::new(selection.selection, definition)?,
                )
                .is_some()
        {
            return Err(DynamicRuntimeError::InvalidSnapshot(
                "lane selection has no unique controller".into(),
            ));
        }
    }
    Ok(restored)
}

fn validate_selection_support(
    selection: &DynamicLaneSelection,
    supported: u16,
) -> Result<(), DynamicRuntimeError> {
    if !matches!(selection, DynamicLaneSelection::All)
        && supported < light_core::programming::PROGRAMMING_CONTRACT_VERSION
    {
        return Err(DynamicRuntimeError::InvalidSnapshot(
            "explicit Dynamic lane selection requires programming contract 1".into(),
        ));
    }
    Ok(())
}

impl DynamicRuntime {
    /// Called during controller reconciliation, before sampling. Compilation and
    /// equality checks happen here; output ticks use only membership lookups.
    pub fn set_controller_lane_selection(
        &mut self,
        instance: Uuid,
        controller: Uuid,
        selection: DynamicLaneSelection,
    ) -> Result<bool, DynamicRuntimeError> {
        validate_selection_support(&selection, self.supported_programming_contract)?;
        let instance = self
            .instances
            .get_mut(&instance)
            .ok_or(DynamicRuntimeError::MissingInstance)?;
        if !instance.controllers.contains_key(&controller) {
            return Err(DynamicRuntimeError::MissingController);
        }
        if instance
            .lane_selections
            .get(&controller)
            .map_or(matches!(selection, DynamicLaneSelection::All), |existing| {
                existing.authored == selection
            })
        {
            return Ok(false);
        }
        let compiled = CompiledLaneSelection::new(selection, &instance.definition)?;
        if let Some(undo) = &mut self.output_frame_undo {
            undo.record_instance(instance.id, Some(instance));
        }
        for values in [
            &mut instance.last_sample_values,
            &mut instance.synchronized_hold_values,
        ] {
            let retained = values
                .iter()
                .filter_map(|(key, value)| {
                    (key.0 != controller
                        || compiled.allows_retained(&instance.definition, *key, value, values))
                    .then_some(*key)
                })
                .collect::<HashSet<_>>();
            values.retain(|key, _| retained.contains(key));
        }
        instance.unavailable_samples.retain(|key, _| {
            instance.last_sample_values.contains_key(key)
                || instance.synchronized_hold_values.contains_key(key)
        });
        instance.lane_selections.insert(controller, compiled);
        Ok(true)
    }
}
