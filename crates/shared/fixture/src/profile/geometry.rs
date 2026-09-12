use super::{
    FixtureMode, FixtureProfile, GeometryGraph, GeometryMotion, GeometryMotionKind, GeometryNode,
    GeometryTemplate, ProfileError, Transform3, Vector3,
};
use light_core::{AttributeKey, AttributeValue};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

impl GeometryGraph {
    pub fn template(template: GeometryTemplate, heads: &[Uuid]) -> Self {
        let root = stable_uuid(&format!("geometry-root-{template:?}"));
        let mut nodes = vec![GeometryNode {
            id: root,
            name: "Chassis".into(),
            parent_id: None,
            transform: Transform3 {
                scale: Vector3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                ..Default::default()
            },
            pivot: Vector3::default(),
            glb_node: None,
            motion: None,
        }];
        match template {
            GeometryTemplate::MovingHead | GeometryTemplate::SharedPanMultiHead => {
                let pan = stable_uuid(&format!("{root}-pan"));
                nodes.push(GeometryNode {
                    id: pan,
                    name: "Pan arm".into(),
                    parent_id: Some(root),
                    transform: Transform3::default(),
                    pivot: Vector3::default(),
                    glb_node: None,
                    motion: Some(GeometryMotion {
                        max_speed_per_second: None,
                        acceleration_per_second_squared: None,
                        deceleration_per_second_squared: None,
                        attribute: AttributeKey("pan".into()),
                        kind: GeometryMotionKind::Rotation,
                        axis: Vector3 {
                            x: 0.0,
                            y: 1.0,
                            z: 0.0,
                        },
                        physical_min: -270.0,
                        physical_max: 270.0,
                    }),
                });
                for (index, _) in heads.iter().enumerate() {
                    nodes.push(GeometryNode {
                        id: stable_uuid(&format!("{pan}-tilt-{index}")),
                        name: format!("Tilt head {}", index + 1),
                        parent_id: Some(pan),
                        transform: Transform3::default(),
                        pivot: Vector3::default(),
                        glb_node: None,
                        motion: Some(GeometryMotion {
                            max_speed_per_second: None,
                            acceleration_per_second_squared: None,
                            deceleration_per_second_squared: None,
                            attribute: AttributeKey("tilt".into()),
                            kind: GeometryMotionKind::Rotation,
                            axis: Vector3 {
                                x: 1.0,
                                y: 0.0,
                                z: 0.0,
                            },
                            physical_min: -135.0,
                            physical_max: 135.0,
                        }),
                    });
                }
            }
            GeometryTemplate::Fixed | GeometryTemplate::Bar | GeometryTemplate::Matrix => {}
        }
        Self {
            nodes,
            emitters: Vec::new(),
        }
    }

    pub fn validate(&self, head_ids: &HashSet<Uuid>) -> Result<(), ProfileError> {
        let node_ids = self
            .nodes
            .iter()
            .map(|node| node.id)
            .collect::<HashSet<_>>();
        if node_ids.len() != self.nodes.len() {
            return Err(ProfileError::Invalid(
                "geometry node IDs must be unique".into(),
            ));
        }
        for node in &self.nodes {
            if node
                .parent_id
                .is_some_and(|parent| !node_ids.contains(&parent) || parent == node.id)
            {
                return Err(ProfileError::Invalid("geometry parent is invalid".into()));
            }
            let mut seen = HashSet::new();
            let mut cursor = node.parent_id;
            while let Some(parent) = cursor {
                if !seen.insert(parent) {
                    return Err(ProfileError::Invalid(
                        "geometry hierarchy contains a cycle".into(),
                    ));
                }
                cursor = self
                    .nodes
                    .iter()
                    .find(|candidate| candidate.id == parent)
                    .and_then(|candidate| candidate.parent_id);
            }
        }
        for node in &self.nodes {
            let Some(motion) = &node.motion else { continue };
            // A declared limit has to be a real rate. Zero would mean an axis that never arrives.
            for figure in [
                motion.max_speed_per_second,
                motion.acceleration_per_second_squared,
                motion.deceleration_per_second_squared,
            ]
            .into_iter()
            .flatten()
            {
                if !figure.is_finite() || figure <= 0.0 {
                    return Err(ProfileError::Invalid(
                        "geometry axis speed, acceleration and deceleration must be positive"
                            .into(),
                    ));
                }
            }
        }
        let mut emitter_ids = HashSet::new();
        for emitter in &self.emitters {
            if !emitter_ids.insert(emitter.id)
                || !node_ids.contains(&emitter.node_id)
                || emitter
                    .head_id
                    .is_some_and(|head| !head_ids.contains(&head))
                || emitter.beam_angle_degrees < 0.0
                || emitter.field_angle_degrees < emitter.beam_angle_degrees
            {
                return Err(ProfileError::Invalid("geometry emitter is invalid".into()));
            }
        }
        Ok(())
    }

    pub fn resolved_transforms(
        &self,
        values: &HashMap<AttributeKey, AttributeValue>,
    ) -> HashMap<Uuid, Transform3> {
        self.nodes
            .iter()
            .map(|node| {
                let mut transform = node.transform;
                if let Some(motion) = &node.motion
                    && let Some(level) = values
                        .get(&motion.attribute)
                        .and_then(AttributeValue::normalized)
                {
                    let physical = motion.physical_min
                        + (motion.physical_max - motion.physical_min) * level.clamp(0.0, 1.0);
                    match motion.kind {
                        GeometryMotionKind::Rotation => {
                            transform.rotation_degrees.x += motion.axis.x * physical;
                            transform.rotation_degrees.y += motion.axis.y * physical;
                            transform.rotation_degrees.z += motion.axis.z * physical;
                        }
                        GeometryMotionKind::Translation => {
                            transform.translation.x += motion.axis.x * physical;
                            transform.translation.y += motion.axis.y * physical;
                            transform.translation.z += motion.axis.z * physical;
                        }
                    }
                }
                (node.id, transform)
            })
            .collect()
    }
}

pub(crate) fn stable_uuid(value: &str) -> Uuid {
    fn hash(seed: u64, bytes: &[u8]) -> u64 {
        bytes.iter().fold(seed, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }
    let high = hash(0xcbf2_9ce4_8422_2325, value.as_bytes());
    let low = hash(0x8422_2325_cbf2_9ce4, value.as_bytes());
    Uuid::from_u128((u128::from(high) << 64) | u128::from(low))
}

impl FixtureProfile {
    /// This mode's geometry: the fixture's own, with its heads bound to the emitters.
    ///
    /// Every consumer asks for geometry this way, so neither a lifted profile nor one whose modes
    /// still carry their own graph needs to be special-cased anywhere but here.
    pub fn mode_geometry(&self, mode: &FixtureMode) -> GeometryGraph {
        // A mode that still carries its own graph is one the lift left alone, and its graph is
        // the more specific statement. After a lift the mode's graph is empty and this is the
        // fixture's.
        if !mode.geometry.nodes.is_empty() || self.geometry.nodes.is_empty() {
            return mode.geometry.clone();
        }
        let heads = mode
            .emitter_heads
            .iter()
            .map(|binding| (binding.emitter_id, binding.head_id))
            .collect::<HashMap<_, _>>();
        let mut bound = self.geometry.clone();
        bound.emitters.retain_mut(|emitter| {
            emitter.head_id = heads.get(&emitter.id).copied();
            // An emitter this personality drives nothing with is not lit in it. A four-lamp mode
            // of an eight-lamp blinder is exactly that, and drawing the other four would be a lie.
            emitter.head_id.is_some()
        });
        bound
    }
}
