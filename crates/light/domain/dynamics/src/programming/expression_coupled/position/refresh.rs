//! TL-842 P1: a controller of plain authored Pan/Tilt leaves compiles, every frame, to the same
//! one-leaf whole-Angles graph; only the two angles change. While nothing else holds last
//! frame's compiled forest, this frame's branch is compiled into it in place: the same tape,
//! preflight and leaf validation as a fresh compile, without rebuilding the graph, its lineage,
//! metadata and base endpoints, or dropping last frame's.
use super::*;

impl CompiledCoupledExpression {
    /// Rewrite this compiled plain whole-Angles forest with `forest`, exactly as
    /// [`Self::from_position_forest`] would compile it, and answer `true`. Anything else (another
    /// shape, a shared part, a changed address or role, a failed check) leaves it untouched and
    /// answers `false`, so the caller compiles afresh and meets the same error there.
    pub(crate) fn refresh_plain_whole(
        &mut self,
        retained: &[crate::DynamicRuntimeSample],
        forest: &[PositionForestNode],
        forest_root: usize,
    ) -> bool {
        let (
            [
                PositionForestNode::Whole {
                    expression,
                    lane_id,
                    sources,
                },
            ],
            0,
        ) = (forest, forest_root)
        else {
            return false;
        };
        if validate_forest_shape(forest, forest_root).is_err()
            || self.owner != ProgrammingOwner::Position
            || self.root != 1
            || !matches!(self.nodes.as_slice(), [Node::Underlay, Node::Leaf { .. }])
            || self.leaf_sources.len() != 2
            || !self.orthogonals.is_empty()
            || !self.models.is_empty()
            || self.position_node_origins
                != [
                    None,
                    Some(PositionCompiledNodeLocation::WholeTape(
                        0,
                        RetainedNodeId(0),
                    )),
                ]
        {
            return false;
        }
        let CoupledRetainedSources::PositionForest(old_retained) = &mut self.retained else {
            return false;
        };
        let Some(lineage) = self.position_forest_lineage.as_mut().and_then(Arc::get_mut) else {
            return false;
        };
        let unchanged_lineage = lineage.root == 0
            && *lineage.compiled_nodes == [1]
            && lineage.cohort_members.is_empty()
            && matches!(&*lineage.nodes, [PositionForestNode::Whole { lane_id: old, .. }] if old == lane_id)
            && matches!(&*lineage.wholes, [whole] if whole.forest_node == 0
                && whole.lane_id == *lane_id
                && *whole.tape_to_compiled == [Some(1)]);
        if !unchanged_lineage
            || old_retained.len() != retained.len()
            || Arc::get_mut(old_retained).is_none()
            || Arc::get_mut(&mut lineage.nodes).is_none()
            || Arc::get_mut(&mut lineage.wholes).is_none()
        {
            return false;
        }
        let Ok(Some((node, provenance, tape))) =
            forest_compile::plain_whole(expression, lane_id, sources)
        else {
            return false;
        };
        let (
            Node::Leaf {
                address,
                value,
                role,
                ..
            },
            Node::Leaf {
                address: old_address,
                role: old_role,
                ..
            },
        ) = (&node, &self.nodes[1])
        else {
            return false;
        };
        // Roles, underlay and base endpoint shape follow from the address and role alone.
        if address.address() != old_address.address() || role != old_role {
            return false;
        }
        let base = (role_of(address.address()) == CoupledExpressionRole::Base).then(|| {
            CoupledBaseEndpoint::Value {
                address: Arc::clone(address),
                value: value.clone(),
            }
        });
        match (
            &base,
            self.base_endpoints.as_slice(),
            self.base_endpoint_nodes.as_slice(),
        ) {
            (Some(_), [CoupledBaseEndpoint::Value { .. }], [origins]) if *origins == [1] => {}
            (None, [], []) => {}
            _ => return false,
        }
        Arc::get_mut(old_retained)
            .expect("checked unique")
            .clone_from_slice(retained);
        Arc::get_mut(&mut lineage.nodes).expect("checked unique")[0] = forest[0].clone();
        Arc::get_mut(&mut lineage.wholes).expect("checked unique")[0].tape = tape;
        if let Some(base) = base {
            self.base_endpoints[0] = base;
        }
        self.nodes[1] = node;
        self.leaf_sources[1] = provenance;
        true
    }

    /// Every compiled part of a plain whole-Angles forest, for equivalence tests.
    #[cfg(test)]
    pub(crate) fn describe_plain_whole(&self) -> String {
        let endpoint = |e: &CoupledComponentEndpoint| {
            format!(
                "{}:{:?}:{:?}:{:?}:{:?}:{:?}",
                e.lane_id,
                e.address.address(),
                e.value,
                e.role,
                e.occurrence,
                e.dependency_occurrence
            )
        };
        let node = |n: &Node| match n {
            Node::Underlay => "underlay".to_owned(),
            Node::Leaf {
                address,
                value,
                role,
                occurrence,
                dependency_occurrence,
            } => format!(
                "leaf {:?} {value:?} {role:?} {occurrence:?} {dependency_occurrence:?}",
                address.address()
            ),
            _ => "other".to_owned(),
        };
        let retained = match &self.retained {
            CoupledRetainedSources::PositionForest(samples) => format!("{samples:?}"),
            CoupledRetainedSources::Expression(expression) => format!("{expression:?}"),
        };
        let lineage = self.position_forest_lineage.as_ref().map(|lineage| {
            let nodes = lineage
                .nodes
                .iter()
                .map(|n| match n {
                    PositionForestNode::Whole {
                        expression,
                        lane_id,
                        sources,
                    } => format!(
                        "whole {expression:?} {lane_id} [{}]",
                        sources.iter().map(endpoint).collect::<Vec<_>>().join(",")
                    ),
                    _ => "other".to_owned(),
                })
                .collect::<Vec<_>>();
            let wholes = lineage
                .wholes
                .iter()
                .map(|w| {
                    format!(
                        "{} {} {:?} {:?}",
                        w.forest_node, w.lane_id, w.tape, w.tape_to_compiled
                    )
                })
                .collect::<Vec<_>>();
            format!(
                "{nodes:?} {} {:?} {wholes:?} {}",
                lineage.root,
                lineage.compiled_nodes,
                lineage.cohort_members.len()
            )
        });
        let bases = self
            .base_endpoints
            .iter()
            .map(|b| match b {
                CoupledBaseEndpoint::Value { address, value } => {
                    format!("{:?} {value:?}", address.address())
                }
                _ => "other".to_owned(),
            })
            .collect::<Vec<_>>();
        let leaf_sources = self
            .leaf_sources
            .iter()
            .map(|s| s.iter().map(endpoint).collect::<Vec<_>>().join(","))
            .collect::<Vec<_>>();
        format!(
            "retained {retained}\nlineage {lineage:?}\norigins {:?}\nowner {:?}\nnodes {:?}\nroot {}\nnode_roles {:?}\nunderlay {}\nroles {:?}\northogonals {}\nbases {bases:?} {:?}\nmodels {}\nleaf_sources {leaf_sources:?}",
            self.position_node_origins,
            self.owner,
            self.nodes.iter().map(node).collect::<Vec<_>>(),
            self.root,
            self.node_roles,
            self.base_underlay,
            self.roles,
            self.orthogonals.len(),
            self.base_endpoint_nodes,
            self.models.len(),
        )
    }
}
