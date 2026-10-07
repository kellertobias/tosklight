//! Cold validation of a matching runtime/origin checkpoint. Domain runtime validation (lane
//! shapes, models, selection, controllers and timing) still runs at its normal restore boundary.
use super::*;
use light_dynamics::{
    DynamicHeldPayload, DynamicRuntimeSnapshot, DynamicSampleExpression, RetainedExpressionNode,
    RetainedExpressionTape, RetainedNodeId,
};

/// Persist these fields together. Old runtime history without occurrence IDs has unknown
/// provenance and may omit the catalogue. Referenced IDs never silently degrade to Unknown.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(in crate::runtime) struct DynamicRuntimeSourceCheckpoint {
    pub runtime: DynamicRuntimeSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origins: Option<DynamicSourceOriginsSnapshot>,
}

impl DynamicRuntimeSourceCheckpoint {
    pub fn capture(
        runtime: DynamicRuntimeSnapshot,
        origins: &DynamicSourceOrigins,
    ) -> Result<Self, IntentError> {
        origins.validate_runtime(&runtime)?;
        Ok(Self {
            runtime,
            origins: Some(origins.snapshot()),
        })
    }

    /// This constructs a candidate pair only. The caller validates/restores the runtime before
    /// atomically publishing either side, preserving the previous pair if either check fails.
    pub fn restore(self) -> Result<(DynamicRuntimeSnapshot, DynamicSourceOrigins), IntentError> {
        let origins = match self.origins {
            Some(snapshot) => DynamicSourceOrigins::restore(snapshot, [])?,
            None => DynamicSourceOrigins::default(),
        };
        origins.validate_runtime(&self.runtime)?;
        Ok((self.runtime, origins))
    }
}

impl DynamicSourceOrigins {
    pub fn validate_runtime(&self, runtime: &DynamicRuntimeSnapshot) -> Result<(), IntentError> {
        self.runtime_reachability(runtime).map(|_| ())
    }

    /// Preserve active assignments plus every origin reachable from either held map. Unused
    /// tape storage is not live history; the domain may compact those nodes separately.
    pub fn prune_runtime(
        &mut self,
        runtime: &DynamicRuntimeSnapshot,
    ) -> Result<usize, IntentError> {
        let reachable = self.runtime_reachability(runtime)?;
        self.prune(reachable.ids)
    }

    fn runtime_reachability(
        &self,
        runtime: &DynamicRuntimeSnapshot,
    ) -> Result<RuntimeReachability, IntentError> {
        let mut roots = Vec::new();
        let mut root_targets = Vec::new();
        let mut declared_roots = HashMap::<usize, HashSet<RetainedNodeId>>::new();
        for instance in &runtime.instances {
            for row in instance
                .last_sample_values
                .iter()
                .chain(&instance.synchronized_hold_values)
            {
                let expression = match &row.payload {
                    DynamicHeldPayload::TapeRoot { tape_root } => {
                        let tape = instance
                            .expression_tape
                            .as_ref()
                            .ok_or_else(|| invalid("held row has no retained expression tape"))?;
                        let allowed = declared_roots
                            .entry(Arc::as_ptr(tape) as usize)
                            .or_insert_with(|| tape.roots.iter().copied().collect());
                        if !allowed.contains(tape_root) {
                            return Err(invalid("held row does not name a declared retained root"));
                        }
                        DynamicSampleExpression::Retained {
                            tape: Arc::clone(tape),
                            root: *tape_root,
                        }
                    }
                    // Expression edges are Arcs: this copies only the root, not its history.
                    DynamicHeldPayload::Expression { expression } => expression.clone(),
                    DynamicHeldPayload::Legacy { .. } => continue,
                };
                roots.push(Arc::new(expression));
                root_targets.push(row.target);
            }
        }

        // Import the entire forest in one pass. The importer validates each shared source tape
        // once and preserves shared descendants across rows/maps instead of expanding per row.
        let tape = RetainedExpressionTape::from_roots(&roots)?;
        let mut ids = HashSet::new();
        let mut targets = Vec::<Option<FixtureId>>::with_capacity(tape.nodes.len());
        for node in &tape.nodes {
            let mut target = None;
            match node {
                RetainedExpressionNode::LegacyScalar {
                    occurrence,
                    dependency_occurrence,
                    ..
                } => {
                    // Legacy attribute names do not provide a typed family owner. Do not invent
                    // one; still validate the dependency's explicit kind and target.
                    target = self.occurrence_target(*occurrence, false, None, &mut ids)?;
                    merge_target(
                        &mut target,
                        self.occurrence_target(
                            dependency_occurrence
                                .as_ref()
                                .and_then(|dependency| dependency.occurrence),
                            true,
                            None,
                            &mut ids,
                        )?,
                    )?;
                }
                RetainedExpressionNode::Programming {
                    address,
                    occurrence,
                    dependency_occurrence,
                    ..
                } => {
                    target = self.occurrence_target(
                        *occurrence,
                        false,
                        Some(address.owner()),
                        &mut ids,
                    )?;
                    merge_target(
                        &mut target,
                        self.occurrence_target(
                            dependency_occurrence
                                .as_ref()
                                .and_then(|dependency| dependency.occurrence),
                            true,
                            Some(address.owner()),
                            &mut ids,
                        )?,
                    )?;
                }
                RetainedExpressionNode::AngleNumeric { program } => {
                    let owner = Some(program.address.owner());
                    target = self.occurrence_target(program.occurrence, false, owner, &mut ids)?;
                    let mut dependencies = Vec::new();
                    program.visit_dependencies(&mut |dependency| {
                        dependencies.push(dependency.occurrence)
                    });
                    for occurrence in dependencies {
                        merge_target(
                            &mut target,
                            self.occurrence_target(occurrence, true, owner, &mut ids)?,
                        )?;
                    }
                }
                RetainedExpressionNode::Scale {
                    address,
                    baseline_occurrence,
                    ..
                } => {
                    target = self.occurrence_target(
                        *baseline_occurrence,
                        true,
                        Some(address.owner()),
                        &mut ids,
                    )?;
                }
                RetainedExpressionNode::AngleCurrent { .. }
                | RetainedExpressionNode::Transition { .. } => {}
            }
            for child in node.children() {
                merge_target(&mut target, targets[child.0 as usize])?;
            }
            targets.push(target);
        }
        for (root, expected) in tape.roots.iter().zip(root_targets) {
            if targets[root.0 as usize].is_some_and(|target| target != expected) {
                return Err(invalid(
                    "retained expression occurrence belongs to another target",
                ));
            }
        }
        Ok(RuntimeReachability {
            ids,
            visited_nodes: tape.nodes.len(),
        })
    }

    fn occurrence_target(
        &self,
        id: Option<DynamicSourceOccurrenceId>,
        dependency: bool,
        expected_owner: Option<ProgrammingOwner>,
        ids: &mut HashSet<DynamicSourceOccurrenceId>,
    ) -> Result<Option<FixtureId>, IntentError> {
        let Some(id) = id else {
            return Ok(None);
        };
        let record = self
            .get(id)
            .ok_or_else(|| invalid("runtime history refers to an unknown source occurrence"))?;
        ids.insert(id);
        match record.binding {
            DynamicSourceBinding::Authored { target, .. } if !dependency => Ok(Some(target)),
            DynamicSourceBinding::Fixed { target, owner, .. } if !dependency => {
                if expected_owner.is_some_and(|expected| expected != owner) {
                    return Err(invalid(
                        "retained fixed occurrence belongs to another family",
                    ));
                }
                Ok(Some(target))
            }
            DynamicSourceBinding::StaticBaseline { target, owner } if dependency => {
                if expected_owner.is_some_and(|expected| expected != owner) {
                    return Err(invalid(
                        "retained baseline occurrence belongs to another family",
                    ));
                }
                Ok(Some(target))
            }
            _ => Err(invalid(
                "retained expression source role disagrees with its occurrence",
            )),
        }
    }
}

struct RuntimeReachability {
    ids: HashSet<DynamicSourceOccurrenceId>,
    /// Focused tests prove one unique graph walk across repeated held roots.
    #[cfg_attr(not(test), allow(dead_code))]
    visited_nodes: usize,
}

fn merge_target(
    target: &mut Option<FixtureId>,
    other: Option<FixtureId>,
) -> Result<(), IntentError> {
    if let Some(other) = other {
        if target.is_some_and(|current| current != other) {
            return Err(invalid(
                "retained expression combines origins from different targets",
            ));
        }
        *target = Some(other);
    }
    Ok(())
}

#[cfg(test)]
#[path = "checkpoint_tests.rs"]
mod tests;
