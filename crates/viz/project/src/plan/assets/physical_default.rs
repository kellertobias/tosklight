//! Bind renderer-owned bodies to authored motion without changing the physical solver's graph.

use crate::PatchedFixture;
use light_fixture::{FixtureMode, PositionAxisRole};
use viz_scene::{ModelPartKind, Scene};

/// Default bodies are already assembled. Bake their fitted metres, rather than applying the
/// profile's neutral translations a second time. Only dynamic graph deltas subsequently move them.
pub(super) fn bind(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    scene: &mut Scene,
    original: u32,
) -> u32 {
    let graph = fixture.profile.mode_geometry(mode);
    let Some(position) = mode.position_physical.as_ref() else {
        return original;
    };
    if graph.physical_contract.is_none() || light_fixture::is_derived_position_geometry(&graph) {
        return original;
    }
    let unique_node = |role| {
        let mut nodes = position
            .bindings
            .iter()
            .filter(|b| b.role == role)
            .map(|b| b.node_id);
        let first = nodes.next()?;
        nodes.all(|id| id == first).then_some(first)
    };
    let (Some(pan), Some(tilt)) = (
        unique_node(PositionAxisRole::Pan),
        unique_node(PositionAxisRole::Tilt),
    ) else {
        return original;
    };
    let mut model = scene.models[original as usize].clone();
    if model.emitter_anchor.is_none() {
        return original;
    }
    let physical = &fixture.profile.physical;
    let size = crate::fallback::body_size(
        crate::fallback::classify(&fixture.profile.fixture_type),
        true,
        physical.width_millimetres,
        physical.height_millimetres,
        physical.depth_millimetres,
    );
    let scale = model.scale_to(size);
    for part in &mut model.parts {
        for point in &mut part.positions {
            *point = point.map(|coordinate| coordinate * scale);
        }
        part.geometry_node_id = match part.kind {
            ModelPartKind::Base => None,
            ModelPartKind::Yoke => Some(pan),
            ModelPartKind::Head => Some(tilt),
        };
    }
    model.extent *= scale;
    model.head_pivot *= scale;
    model.emitter_anchor = model.emitter_anchor.map(|p| p * scale);
    model.emitter_size = model.emitter_size.map(|s| s * scale);
    model.bracket_hinge = model.bracket_hinge.map(|p| p * scale);
    scene.models.push(model);
    scene.models.len() as u32 - 1
}
