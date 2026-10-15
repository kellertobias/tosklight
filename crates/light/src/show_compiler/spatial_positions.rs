//! Static programming positions share the physical Patch placement, in desk metres.
use super::{objects, patch};
use crate::ActionError;
use light_core::{FixtureId, spatial::RigidTransform};
use light_dynamics::SpatialPosition;
use light_fixture::{GeometryBracket, GeometryGraph, PatchedFixture, Vector3};
use light_show::PortableShowCandidate;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Derives positions from one captured document, including its embedded profile revisions.
/// Legacy Stage-only identities remain readable; Patch owners always override them.
pub fn compile_show_spatial_positions(
    candidate: PortableShowCandidate<'_>,
) -> Result<HashMap<FixtureId, SpatialPosition>, ActionError> {
    derive_for_patch(candidate, &patch::compile_patch(candidate)?)
}

pub(super) fn derive_for_patch(
    candidate: PortableShowCandidate<'_>,
    fixtures: &[PatchedFixture],
) -> Result<HashMap<FixtureId, SpatialPosition>, ActionError> {
    let mut positions = objects::decode_dynamic_stage_positions(candidate)?;
    for fixture in fixtures {
        positions.remove(&fixture.fixture_id);
        for head in &fixture.logical_heads {
            positions.remove(&head.fixture_id);
        }
        let origin = [fixture.location.x, fixture.location.y, fixture.location.z]
            .map(|v| f64::from(v) / 1000.0);
        positions.insert(fixture.fixture_id, spatial(origin));
        let geometry = fixture
            .definition
            .profile_snapshot
            .as_ref()
            .and_then(|profile| {
                let mode = profile
                    .modes
                    .iter()
                    .find(|mode| Some(mode.id) == fixture.definition.mode_id)?;
                Some(profile.mode_geometry(mode))
            });
        for head in &fixture.logical_heads {
            let local = match (geometry.as_ref(), head.profile_head_id) {
                (Some(graph), Some(id)) => head_anchor(graph, id, fixture.bracket_angle),
                _ => Some([0.0; 3]),
            };
            let Some(local) = local else {
                continue;
            };
            let Some(rotation) = RigidTransform::euler_xyz(vector(
                fixture.rotation.x,
                fixture.rotation.y,
                fixture.rotation.z,
            )) else {
                continue;
            };
            // model_scale changes Venue artwork, not physical lighting geometry.
            let desk = RigidTransform::DESK_TO_PROFILE
                .inverse()
                .point(local.map(|v| v / 1000.0));
            let point = rotation.point(desk);
            let world = std::array::from_fn(|i| origin[i] + point[i]);
            if world.iter().all(|v| v.is_finite()) {
                positions.insert(head.fixture_id, spatial(world));
            }
        }
    }
    // position_master stores absolute placement at rest. Live Point carry is not a new
    // static ranking input; multipatch instances likewise share this primary programmer owner.
    Ok(positions)
}

fn spatial(p: [f64; 3]) -> SpatialPosition {
    SpatialPosition {
        x: p[0] as f32,
        y: p[1] as f32,
        z: p[2] as f32,
    }
}
fn vector(x: f32, y: f32, z: f32) -> [f64; 3] {
    [x, y, z].map(f64::from)
}
fn v(p: Vector3) -> [f64; 3] {
    vector(p.x, p.y, p.z)
}

/// A multi-emitter head is anchored at its common geometry ancestor, never the first
/// emitter. Disjoint roots share the implicit fixture frame. No geometry means root.
fn head_anchor(graph: &GeometryGraph, head: Uuid, bracket_angle: f32) -> Option<[f64; 3]> {
    let nodes: HashMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    if nodes.len() != graph.nodes.len() {
        return None;
    }
    let mut paths = Vec::new();
    for emitter in graph
        .emitters
        .iter()
        .filter(|emitter| emitter.head_id == Some(head))
    {
        let mut path = Vec::new();
        let mut seen = HashSet::new();
        let mut next = Some(emitter.node_id);
        while let Some(id) = next {
            if !seen.insert(id) {
                return None;
            }
            let node = nodes.get(&id)?;
            path.push(id);
            next = node.parent_id;
        }
        paths.push(path);
    }
    let Some(first) = paths.first() else {
        return Some([0.0; 3]);
    };
    let Some(anchor) = first
        .iter()
        .find(|id| paths.iter().all(|path| path.contains(id)))
    else {
        return Some([0.0; 3]);
    };
    let mut point = [0.0; 3];
    let mut next = Some(*anchor);
    while let Some(id) = next {
        let node = nodes.get(&id)?;
        let pivot = v(node.pivot);
        let scale = v(node.transform.scale).map(|s| if s == 0.0 { 1.0 } else { s });
        let local = std::array::from_fn(|i| (point[i] - pivot[i]) * scale[i]);
        let rotated =
            RigidTransform::euler_xyz(v(node.transform.rotation_degrees))?.direction(local);
        let translation = v(node.transform.translation);
        point = std::array::from_fn(|i| rotated[i] + pivot[i] + translation[i]);
        if let Some(contract) = graph.physical_contract.as_ref().filter(|c| c.version == 1) {
            if let GeometryBracket::Hinge {
                node_id,
                pivot,
                axis,
            } = &contract.bracket
            {
                if *node_id == id {
                    point = RigidTransform::about_pivot(
                        RigidTransform::axis_angle(v(*axis), f64::from(bracket_angle))?,
                        v(*pivot),
                    )?
                    .point(point);
                }
            }
        }
        next = node.parent_id;
    }
    // Match the shared physical forward model: without an authored hinge the bracket
    // rotates the whole fixture about its profile X axis, before mounting into desk axes.
    let has_hinge = graph
        .physical_contract
        .as_ref()
        .filter(|c| c.version == 1)
        .is_some_and(|c| matches!(c.bracket, GeometryBracket::Hinge { .. }));
    if !has_hinge {
        point = RigidTransform::axis_angle([1.0, 0.0, 0.0], f64::from(bracket_angle))?.point(point);
    }
    point.iter().all(|v| v.is_finite()).then_some(point)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn graph() -> (GeometryGraph, Uuid, Uuid) {
        let head = Uuid::from_u128(100);
        let root = Uuid::from_u128(101);
        let left = Uuid::from_u128(102);
        let right = Uuid::from_u128(103);
        let graph = serde_json::from_value(json!({
            "nodes": [
                {"id":root,"name":"Common","parent_id":null,
                  "transform":{"translation":{"x":100,"y":200,"z":300},"rotation_degrees":{"x":0,"y":0,"z":0},"scale":{"x":0,"y":0,"z":0}}},
                {"id":left,"name":"Left","parent_id":root,
                  "transform":{"translation":{"x":-500,"y":0,"z":0},"rotation_degrees":{"x":0,"y":0,"z":0},"scale":{"x":0,"y":0,"z":0}}},
                {"id":right,"name":"Right","parent_id":root,
                  "transform":{"translation":{"x":500,"y":0,"z":0},"rotation_degrees":{"x":0,"y":0,"z":0},"scale":{"x":0,"y":0,"z":0}}}
            ],
            "emitters":[
                {"id":Uuid::from_u128(104),"name":"L","node_id":left,"head_id":head,
                  "beam_angle_degrees":10,"field_angle_degrees":20,"layout":{"type":"point"}},
                {"id":Uuid::from_u128(105),"name":"R","node_id":right,"head_id":head,
                  "beam_angle_degrees":10,"field_angle_degrees":20,"layout":{"type":"point"}}
            ]
        }))
        .unwrap();
        (graph, head, root)
    }

    #[test]
    fn multi_emitter_anchor_is_common_ancestor_independent_of_emitter_order() {
        let (mut graph, head, _) = graph();
        assert_eq!(head_anchor(&graph, head, 0.), Some([100., 200., 300.]));
        graph.emitters.reverse();
        assert_eq!(head_anchor(&graph, head, 0.), Some([100., 200., 300.]));
        // Disjoint roots have the implicit fixture-frame ancestor, not an arbitrary lamp.
        graph.nodes[1].parent_id = None;
        assert_eq!(head_anchor(&graph, head, 0.), Some([0.; 3]));
    }

    #[test]
    fn head_anchor_applies_parent_scale_pivot_rotation_and_declared_bracket() {
        let (mut graph, head, root) = graph();
        graph.emitters.truncate(1);
        graph.nodes[0].transform.scale.x = 2.;
        graph.nodes[0].pivot.x = 100.;
        graph.nodes[0].transform.rotation_degrees.z = 90.;
        // child origin -500; pivoted parent scaled displacement -1200 rotated to -Y.
        let point = head_anchor(&graph, head, 0.).unwrap();
        assert!((point[0] - 200.).abs() < 1e-8);
        assert!((point[1] + 1000.).abs() < 1e-8);
        graph.physical_contract = Some(
            serde_json::from_value(json!({
                "version":1,"provenance":{},
                "bracket":{"kind":"hinge","node_id":root,
                    "pivot":{"x":0,"y":0,"z":0},"axis":{"x":0,"y":0,"z":1}}
            }))
            .unwrap(),
        );
        let bracket = head_anchor(&graph, head, 90.).unwrap();
        assert!((bracket[0] - 1000.).abs() < 1e-8);
        assert!((bracket[1] - 200.).abs() < 1e-8);
        assert_eq!(bracket[2], 300.);
    }

    #[test]
    fn malformed_geometry_does_not_fall_back_to_an_invented_head_position() {
        let (mut graph, head, root) = graph();
        graph.nodes[0].parent_id = Some(graph.nodes[1].id);
        assert_eq!(head_anchor(&graph, head, 0.), None);
        graph.nodes[0].parent_id = Some(Uuid::from_u128(999));
        assert_eq!(head_anchor(&graph, head, 0.), None);
        graph.nodes[0].parent_id = None;
        graph.nodes.push(graph.nodes[0].clone());
        assert_eq!(head_anchor(&graph, head, 0.), None);
        graph.nodes.retain(|node| node.id != root);
        graph.emitters.clear();
        assert_eq!(head_anchor(&graph, head, 0.), Some([0.; 3]));
    }
    #[test]
    fn hinge_less_bracket_turns_nonzero_head_anchor_about_fixture_origin() {
        let (mut graph, head, _) = graph();
        for bracket in [
            None,
            Some(GeometryBracket::Fixed),
            Some(GeometryBracket::Unknown),
        ] {
            graph.physical_contract =
                bracket.map(|bracket| light_fixture::GeometryPhysicalContract {
                    version: 1,
                    provenance: Default::default(),
                    bracket,
                });
            let point = head_anchor(&graph, head, 90.).unwrap();
            assert!((point[0] - 100.).abs() < 1e-8);
            assert!((point[1] + 300.).abs() < 1e-8);
            assert!((point[2] - 200.).abs() < 1e-8);
        }
    }
}
