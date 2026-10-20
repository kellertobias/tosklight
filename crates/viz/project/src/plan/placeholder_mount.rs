//! Legacy single-lens placeholders inherit the emitting surface of their renderer-owned body.

use super::*;

impl EmitterMount {
    /// An unbound identity body with a point at its origin specifies optics, not a physical lens
    /// location. Keep its identity and controls, but use the same mount as an empty geometry graph.
    /// Custom models, physical contracts and any authored placement remain authoritative.
    pub(super) fn for_placeholder(
        self,
        fixture: &PatchedFixture,
        geometry: &GeometryGraph,
    ) -> Option<Self> {
        if fixture.profile.model_asset.is_some()
            || geometry.physical_contract.is_some()
            || self.face.is_none()
        {
            return None;
        }
        let [node] = geometry.nodes.as_slice() else {
            return None;
        };
        let [emitter] = geometry.emitters.as_slice() else {
            return None;
        };
        let zero = Vector3::default();
        let one = Vector3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        };
        (node.parent_id.is_none()
            && (node.glb_node.is_none() || node.glb_node.as_deref() == Some("lens"))
            && node.motion.is_none()
            && node.pivot == zero
            && node.transform.translation == zero
            && node.transform.rotation_degrees == zero
            && node.transform.scale == one
            && emitter.node_id == node.id
            && emitter.origin == zero
            && emitter.orientation_degrees == zero
            && emitter.directional
            && matches!(emitter.layout, light_fixture::EmitterLayout::Point))
        .then_some(self)
    }
}
