//! Fixed routing for the synthetic whole Angles of exactly two original authored axes.
//! Fresh values, source occurrences and the retained tape remain per compilation. No runtime
//! value, target, rank, operation origin or Current is cached. The general compiler is unchanged.
use super::*;

struct Admitted<'a> {
    expression: &'a Arc<DynamicSampleExpression>,
    sources: &'a Arc<[CoupledComponentEndpoint]>,
    lane_id: uuid::Uuid,
}

fn admit<'a>(
    retained: &[crate::DynamicRuntimeSample],
    forest: &'a [PositionForestNode],
    root: usize,
) -> Option<Admitted<'a>> {
    if root != 0 || retained.len() != 2 || retained[0].lane_id == retained[1].lane_id {
        return None;
    }
    let [
        PositionForestNode::Whole {
            expression,
            lane_id,
            sources,
        },
    ] = forest
    else {
        return None;
    };
    let DynamicSampleExpression::Programming {
        address,
        value: DynamicValue::Family(AttributeValue::Position(position)),
        occurrence: None,
        dependency_occurrence: None,
    } = expression.as_ref()
    else {
        return None;
    };
    if address.representation != DynamicFamilyRepresentation::Angles
        || address.component.is_some()
        || sources.len() != 2
        || *lane_id != retained.iter().map(|s| s.lane_id).max()?
    {
        return None;
    }
    let PositionIntent::Angles {
        pan_degrees: ScalarIntent::Value(pan),
        tilt_degrees: ScalarIntent::Value(tilt),
    } = position.as_ref()
    else {
        return None;
    };
    let first = &retained[0];
    let mut seen = [false; 2];
    for (sample, source) in retained.iter().zip(sources.iter()) {
        if sample.instance_id != first.instance_id
            || sample.controller_id != first.controller_id
            || sample.target != first.target
            || sample.priority != first.priority
            || sample.activated_at_millis != first.activated_at_millis
            || sample.activation_mix != first.activation_mix
        {
            return None;
        }
        let DynamicSampleExpression::Programming {
            address,
            value: DynamicValue::Scalar(value),
            occurrence,
            dependency_occurrence: None,
        } = &sample.expression
        else {
            return None;
        };
        let index = match address.component {
            Some(ProgrammingComponent::Pan) => 0,
            Some(ProgrammingComponent::Tilt) => 1,
            _ => return None,
        };
        if address.representation != DynamicFamilyRepresentation::Angles
            || seen[index]
            || *value != [*pan, *tilt][index]
            || source.lane_id != sample.lane_id
            || source.role != CoupledLeafRole::Authored
            || source.address.address() != address.as_ref()
            || source.value != DynamicValue::Scalar(*value)
            || source.occurrence != *occurrence
            || source.dependency_occurrence.is_some()
        {
            return None;
        }
        seen[index] = true;
    }
    seen.iter().all(|v| *v).then_some(Admitted {
        expression,
        sources,
        lane_id: *lane_id,
    })
}

#[cfg(test)]
thread_local! {
    static GENERAL_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static COUNTS: std::cell::Cell<(usize,usize)> = const { std::cell::Cell::new((0,0)) };
}

impl PositionForestCompilation {
    pub(in crate::programming::expression_coupled::position) fn try_plain_angles(
        &mut self,
        retained: &[crate::DynamicRuntimeSample],
        forest: &[PositionForestNode],
        root: usize,
    ) -> Result<bool, TransitionError> {
        #[cfg(test)]
        let admitted = if GENERAL_ONLY.with(std::cell::Cell::get) {
            None
        } else {
            admit(retained, forest, root)
        };
        #[cfg(not(test))]
        let admitted = admit(retained, forest, root);
        let Some(admitted) = admitted else {
            #[cfg(test)]
            COUNTS.with(|n| {
                let (fast, general) = n.get();
                n.set((fast, general + 1));
            });
            return Ok(false);
        };
        // Same original validation, in the same order, before any graph mutation. Only the
        // now-proven one-leaf reachability and remapping passes are omitted.
        let tape = Arc::new(RetainedExpressionTape::from_roots(std::slice::from_ref(
            admitted.expression,
        ))?);
        let RetainedExpressionNode::Programming {
            address,
            value,
            occurrence,
            dependency_occurrence,
        } = &tape.nodes[0]
        else {
            unreachable!("one authored whole Angles leaf");
        };
        let mut node = compile_leaf(address, value, None, &mut Vec::new())?;
        let Node::Leaf {
            occurrence: slot,
            dependency_occurrence: dependency_slot,
            ..
        } = &mut node
        else {
            unreachable!("compiled whole Angles leaf");
        };
        *slot = *occurrence;
        *dependency_slot = dependency_occurrence.clone();
        self.wholes.push(PositionForestWholeLineage {
            forest_node: 0,
            lane_id: admitted.lane_id,
            tape,
            tape_to_compiled: Arc::from([Some(1)]),
        });
        self.nodes.push(node);
        self.position_node_origins
            .push(Some(PositionCompiledNodeLocation::WholeTape(
                0,
                RetainedNodeId(0),
            )));
        self.leaf_sources.push(Arc::clone(admitted.sources));
        self.mapped.push(1);
        #[cfg(test)]
        COUNTS.with(|n| {
            let (fast, general) = n.get();
            n.set((fast + 1, general));
        });
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
