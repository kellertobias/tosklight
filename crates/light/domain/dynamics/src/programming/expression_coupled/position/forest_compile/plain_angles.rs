//! Import one validated whole-Angles leaf without temporary generic graph/remapping vectors.
//! The caller retains the ordinary fresh tape and owner preflight. Values and provenance stay
//! fresh, and arbitrary forest offsets, retained metadata and original source order are kept.
use super::*;

#[cfg(test)]
thread_local! {
    static GENERAL_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static COUNTS: std::cell::Cell<(usize,usize)> = const { std::cell::Cell::new((0,0)) };
}

impl PositionForestCompilation {
    pub(super) fn try_plain_angles(
        &mut self,
        forest_node: usize,
        lane_id: &uuid::Uuid,
        sources: &Arc<[CoupledComponentEndpoint]>,
        tape: &Arc<RetainedExpressionTape>,
    ) -> Result<Option<usize>, TransitionError> {
        #[cfg(test)]
        let enabled = !GENERAL_ONLY.with(std::cell::Cell::get);
        #[cfg(not(test))]
        let enabled = true;
        let leaf = match tape.nodes.as_slice() {
            [
                RetainedExpressionNode::Programming {
                    address,
                    value: value @ DynamicValue::Family(AttributeValue::Position(position)),
                    occurrence,
                    dependency_occurrence,
                },
            ] if enabled
                && tape.roots == [RetainedNodeId(0)]
                && address.representation == DynamicFamilyRepresentation::Angles
                && address.component.is_none()
                && matches!(
                    position.as_ref(),
                    PositionIntent::Angles {
                        pan_degrees: ScalarIntent::Value(_),
                        tilt_degrees: ScalarIntent::Value(_),
                    }
                ) =>
            {
                Some((address, value, occurrence, dependency_occurrence))
            }
            _ => None,
        };
        let Some((address, value, occurrence, dependency_occurrence)) = leaf else {
            #[cfg(test)]
            COUNTS.with(|n| {
                let (fast, general) = n.get();
                n.set((fast, general + 1));
            });
            return Ok(None);
        };
        // Exact original leaf validation and occurrence copy, before any graph mutation.
        let mut node = compile_leaf(address, value, None, &mut Vec::new())?;
        let Node::Leaf {
            address,
            value,
            role,
            occurrence: slot,
            dependency_occurrence: dependency_slot,
        } = &mut node
        else {
            unreachable!("compiled whole Angles leaf");
        };
        *slot = *occurrence;
        *dependency_slot = dependency_occurrence.clone();
        let provenance = if sources.is_empty() {
            Arc::from([CoupledComponentEndpoint {
                lane_id: *lane_id,
                address: Arc::clone(address),
                value: value.clone(),
                role: *role,
                occurrence: *slot,
                dependency_occurrence: dependency_slot.clone(),
            }])
        } else {
            Arc::clone(sources)
        };
        let next = self.nodes.len();
        self.wholes.push(PositionForestWholeLineage {
            forest_node,
            lane_id: *lane_id,
            tape: Arc::clone(tape),
            tape_to_compiled: Arc::from([Some(next)]),
        });
        self.nodes.push(node);
        self.position_node_origins
            .push(Some(PositionCompiledNodeLocation::WholeTape(
                forest_node,
                RetainedNodeId(0),
            )));
        self.leaf_sources.push(provenance);
        #[cfg(test)]
        COUNTS.with(|n| {
            let (fast, general) = n.get();
            n.set((fast + 1, general));
        });
        Ok(Some(next))
    }
}

#[cfg(test)]
mod tests;
