//! CPU-only checked allocation planning; chunks never discard instances to fit a GPU limit.
use super::super::Segment;
use super::{PROJECTION_WORKGROUP, WireInstance, failure, projected_size};
use crate::{
    instances::{MeshInstance, MeshKind},
    mesh::MeshData,
    renderer::RenderError,
};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) struct Asset<'a> {
    pub data: &'a MeshData,
    pub edges: &'a [Segment],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct Reference {
    pub offset: u32,
    pub instance: u32,
}
#[derive(Default)]
pub(super) struct ChunkPlan {
    pub positions: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub edges: Vec<Segment>,
    pub vertices: Vec<Reference>,
    pub edge_refs: Vec<Reference>,
    pub edge_indices: Vec<u32>,
    pub links: Vec<(usize, usize)>,
    assets: HashMap<MeshKind, (u32, u32)>,
}

pub(super) fn build<'a>(
    drawn: &[(MeshKind, Vec<MeshInstance>)],
    asset: impl Fn(MeshKind) -> Option<Asset<'a>>,
    limit: u64,
    max_workgroups: u32,
) -> Result<Vec<ChunkPlan>, RenderError> {
    let max_edges = u64::from(max_workgroups) * u64::from(PROJECTION_WORKGROUP);
    if max_edges == 0 {
        return Err(failure(
            "the GPU exposes no compute workgroups for edge preprocessing",
        ));
    }
    let mut chunks = Vec::new();
    let mut current = ChunkPlan::default();
    for (kind_index, (kind, instances)) in drawn.iter().enumerate() {
        if instances.is_empty() {
            continue;
        }
        let Some(asset) = asset(*kind) else { continue };
        if asset
            .data
            .indices
            .iter()
            .any(|&index| index as usize >= asset.data.vertices.len())
        {
            return Err(failure("model contains an out-of-range triangle index"));
        }
        for instance in 0..instances.len() {
            if !current.fits(*kind, asset, limit, max_edges) {
                if current.links.is_empty() {
                    return Err(failure(&format!(
                        "asset {kind:?} and one instance exceed the {limit}-byte storage budget; simplify the fixture model"
                    )));
                }
                chunks.push(current);
                current = ChunkPlan::default();
                if !current.fits(*kind, asset, limit, max_edges) {
                    return Err(failure(&format!(
                        "asset {kind:?} and one instance exceed the {limit}-byte storage budget; simplify the fixture model"
                    )));
                }
            }
            current.push(*kind, asset, (kind_index, instance))?;
        }
    }
    if !current.links.is_empty() {
        chunks.push(current);
    }
    Ok(chunks)
}

impl ChunkPlan {
    fn fits(&self, kind: MeshKind, asset: Asset<'_>, limit: u64, max_edges: u64) -> bool {
        let new = !self.assets.contains_key(&kind);
        let add = |length: usize, count: usize, stride: usize| {
            length
                .checked_add(count)
                .and_then(|n| n.checked_mul(stride))
                .is_some_and(|bytes| bytes.max(16) as u64 <= limit)
        };
        add(
            self.positions.len(),
            if new { asset.data.vertices.len() } else { 0 },
            16,
        ) && add(
            self.indices.len(),
            asset.data.indices.len(),
            4,
        ) && add(
            self.edges.len(),
            if new { asset.edges.len() } else { 0 },
            size_of::<Segment>(),
        ) && limit >= 256 // DynamicBuffer's minimum allocation.
            && add(self.links.len(), 1, size_of::<WireInstance>())
            && add(
                self.vertices.len(),
                asset.data.vertices.len(),
                size_of::<Reference>(),
            )
            && add(
                self.edge_refs.len(),
                asset.edges.len(),
                size_of::<Reference>(),
            )
            && self.edge_refs.len().checked_add(asset.edges.len()).and_then(projected_size)
                .is_some_and(|bytes| bytes <= limit)
            && self.edge_refs.len().checked_add(asset.edges.len())
                .is_some_and(|count| count as u64 <= max_edges)
            && self
                .indices
                .len()
                .checked_add(asset.data.indices.len())
                .is_some_and(|n| n <= u32::MAX as usize)
            && self.vertices.len().checked_add(asset.data.vertices.len())
                .is_some_and(|n| n <= u32::MAX as usize)
            && self.edge_refs.len().checked_add(asset.edges.len())
                .is_some_and(|n| n <= u32::MAX as usize / 4)
            && asset.edges.len().checked_mul(6).is_some_and(|added| {
                add(self.edge_indices.len(), added, 4)
                    && self.edge_indices.len().checked_add(added)
                        .is_some_and(|n| n <= u32::MAX as usize)
            })
    }
    fn push(
        &mut self,
        kind: MeshKind,
        asset: Asset<'_>,
        link: (usize, usize),
    ) -> Result<(), RenderError> {
        let checked =
            |n: usize| u32::try_from(n).map_err(|_| failure("geometry offset exceeds u32"));
        let (positions, edges) = if let Some(offset) = self.assets.get(&kind) {
            *offset
        } else {
            let vertex = checked(self.positions.len())?;
            let offsets = (vertex, checked(self.edges.len())?);
            self.positions.extend(
                asset
                    .data
                    .vertices
                    .iter()
                    .map(|v| [v.position[0], v.position[1], v.position[2], 1.0]),
            );
            self.edges.extend_from_slice(asset.edges);
            self.assets.insert(kind, offsets);
            offsets
        };
        let instance = checked(self.links.len())?;
        // Distinct virtual IDs per instance let the indexed vertex cache reuse transformed
        // vertices within that instance, while actual positions stay shared across instances.
        let virtual_base = checked(self.vertices.len())?;
        for vertex in 0..asset.data.vertices.len() {
            self.vertices.push(Reference {
                offset: positions
                    .checked_add(checked(vertex)?)
                    .ok_or_else(|| failure("position offset exceeds u32"))?,
                instance,
            });
        }
        for &index in &asset.data.indices {
            self.indices.push(
                index
                    .checked_add(virtual_base)
                    .ok_or_else(|| failure("virtual vertex index exceeds u32"))?,
            );
        }
        for edge in 0..asset.edges.len() {
            self.edge_indices
                .extend_from_slice(&quad_indices(self.edge_refs.len())?);
            self.edge_refs.push(Reference {
                offset: edges
                    .checked_add(checked(edge)?)
                    .ok_or_else(|| failure("edge offset exceeds u32"))?,
                instance,
            });
        }
        self.links.push(link);
        Ok(())
    }
}

/// Four virtual corners per AA quad, in exactly the former six-vertex triangle-list order.
pub(super) fn quad_indices(edge: usize) -> Result<[u32; 6], RenderError> {
    let base = u32::try_from(edge)
        .ok()
        .and_then(|n| n.checked_mul(4))
        .filter(|base| base.checked_add(3).is_some())
        .ok_or_else(|| failure("edge virtual vertex index exceeds u32"))?;
    Ok([base, base + 1, base + 2, base, base + 2, base + 3])
}
