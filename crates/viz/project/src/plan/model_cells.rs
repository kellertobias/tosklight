//! Default-body lens geometry supplies cells; logical heads supply control ownership.

use super::*;

pub(super) struct ModelCells {
    pub groups: Vec<Vec<Vec3>>,
    pub face: glam::Vec2,
    pub output_per_cell: f32,
    source_is_declared: bool,
}

pub(super) fn layout(
    scene: &Scene,
    fixture: &PatchedFixture,
    fixture_index: u32,
    class: OpticalClass,
    heads: usize,
    mount: EmitterMount,
) -> Option<ModelCells> {
    if fixture.profile.model_asset.is_some()
        || heads == 0
        || !matches!(
            class,
            OpticalClass::Wash | OpticalClass::Par | OpticalClass::Blinder
        )
    {
        return None;
    }
    let placed = scene.fixtures.get(fixture_index as usize)?;
    let model = scene.models.get(placed.model? as usize)?;
    let surfaces = model.emitting_surfaces();
    if surfaces.len() < 2 || surfaces.len() < heads {
        return None;
    }
    let scale = model.scale_to(placed.body.size);
    let face = surfaces
        .iter()
        .map(|surface| surface.size * scale)
        .reduce(glam::Vec2::min)?;
    let inverse = Quat::from_rotation_arc(Vec3::NEG_Y, mount.aim).inverse();
    let mut cells: Vec<Vec3> = surfaces
        .iter()
        .map(|surface| inverse * (surface.anchor * scale - mount.origin))
        .collect();
    let count = cells.len();
    let groups = if heads > 1 && model.has_head && class == OpticalClass::Wash {
        concentric_groups(&cells, heads, face.min_element())?
    } else {
        cells.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.z.total_cmp(&b.z)));
        let mut groups = vec![Vec::new(); heads];
        for (index, cell) in cells.into_iter().enumerate() {
            groups[index * heads / count].push(cell);
        }
        groups
    };
    Some(ModelCells {
        groups,
        face,
        output_per_cell: 1.0 / count as f32,
        source_is_declared: fixture.profile.optics.light_source.is_some(),
    })
}

/// Radial zones are inferred only when the glass actually forms concentric rings. A centre
/// lens and its surrounding inner ring can share the first zone when there are fewer controls
/// than radial bands. This also handles old embedded profiles with no authored cell layout.
fn concentric_groups(cells: &[Vec3], heads: usize, diameter: f32) -> Option<Vec<Vec<Vec3>>> {
    let tolerance = (diameter * 0.2).max(0.0005);
    let radius = |cell: Vec3| glam::Vec2::new(cell.x, cell.z).length();
    let mut ordered = cells.to_vec();
    ordered.sort_by(|a, b| radius(*a).total_cmp(&radius(*b)));
    let mut rings: Vec<Vec<Vec3>> = Vec::new();
    for cell in ordered {
        if rings
            .last()
            .is_none_or(|ring| (radius(ring[0]) - radius(cell)).abs() > tolerance)
        {
            rings.push(Vec::new());
        }
        rings.last_mut()?.push(cell);
    }
    if rings.len() < heads || rings.iter().skip(1).any(|ring| ring.len() < 3) {
        return None;
    }
    let merge = rings.len() - heads;
    let mut groups = vec![Vec::new(); heads];
    for (index, ring) in rings.into_iter().enumerate() {
        groups[index.saturating_sub(merge)].extend(ring);
    }
    Some(groups)
}

pub(super) fn head_layout(
    layout: Option<&ModelCells>,
    index: usize,
    count: usize,
    width: f32,
    mount: EmitterMount,
    optics: &EmitterOptics,
) -> (EmitterOptics, Vec3, EmitterLayoutCells) {
    let Some(layout) = layout else {
        return (
            optics.clone(),
            mount.origin + head_offset(index, count, optics.source.width, width),
            EmitterLayoutCells::single(),
        );
    };
    let mut optics = optics.clone();
    let cells = &layout.groups[index];
    let span = cells
        .iter()
        .fold(Vec3::ZERO, |bound, cell| bound.max(cell.abs()))
        * 2.0;
    if !layout.source_is_declared {
        optics.source.width = span.x + layout.face.x;
        optics.source.height = span.z + layout.face.y;
    }
    optics.output *= layout.output_per_cell * cells.len() as f32;
    (
        optics,
        mount.origin,
        EmitterLayoutCells {
            aperture_offsets: cells.clone(),
            aperture_size: Some(layout.face),
            ..EmitterLayoutCells::single()
        },
    )
}

pub(super) fn apply_placeholder(
    emitter: &mut EmitterInstance,
    layout: Option<&ModelCells>,
    mount: EmitterMount,
) {
    if layout.is_some() {
        let (optics, _, cells) = head_layout(layout, 0, 1, 0.0, mount, &emitter.optics);
        emitter.optics = optics;
        emitter.cells = cells;
    }
}

pub(super) fn geometry_scale(geometry: &GeometryGraph, scale: f32) -> f32 {
    if geometry.physical_contract.is_some() {
        1.0
    } else {
        scale
    }
}
