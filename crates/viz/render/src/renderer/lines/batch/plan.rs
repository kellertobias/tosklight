//! CPU-only checked allocation planning; chunks never discard instances to fit a GPU limit.
use super::super::Segment;
use super::{WireInstance, failure};
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
    pub triangles: Vec<Reference>,
    pub edge_refs: Vec<Reference>,
    pub links: Vec<(usize, usize)>,
    assets: HashMap<MeshKind, (u32, u32)>,
}

pub(super) fn build<'a>(
    drawn: &[(MeshKind, Vec<MeshInstance>)],
    asset: impl Fn(MeshKind) -> Option<Asset<'a>>,
    limit: u64,
) -> Result<Vec<ChunkPlan>, RenderError> {
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
            if !current.fits(*kind, asset, limit) {
                if current.links.is_empty() {
                    return Err(failure(&format!(
                        "asset {kind:?} and one instance exceed the {limit}-byte storage budget; simplify the fixture model"
                    )));
                }
                chunks.push(current);
                current = ChunkPlan::default();
                if !current.fits(*kind, asset, limit) {
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
    fn fits(&self, kind: MeshKind, asset: Asset<'_>, limit: u64) -> bool {
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
            if new { asset.data.indices.len() } else { 0 },
            4,
        ) && add(
            self.edges.len(),
            if new { asset.edges.len() } else { 0 },
            size_of::<Segment>(),
        ) && limit >= 256 // DynamicBuffer's minimum allocation.
            && add(self.links.len(), 1, size_of::<WireInstance>())
            && add(
                self.triangles.len(),
                asset.data.indices.len() / 3,
                size_of::<Reference>(),
            )
            && add(
                self.edge_refs.len(),
                asset.edges.len(),
                size_of::<Reference>(),
            )
            && self
                .triangles
                .len()
                .checked_add(asset.data.indices.len() / 3)
                .is_some_and(|n| n <= u32::MAX as usize / 3)
            && self
                .edge_refs
                .len()
                .checked_add(asset.edges.len())
                .is_some_and(|n| n <= u32::MAX as usize / 6)
    }
    fn push(
        &mut self,
        kind: MeshKind,
        asset: Asset<'_>,
        link: (usize, usize),
    ) -> Result<(), RenderError> {
        let checked =
            |n: usize| u32::try_from(n).map_err(|_| failure("geometry offset exceeds u32"));
        let (indices, edges) = if let Some(offset) = self.assets.get(&kind) {
            *offset
        } else {
            let vertex = checked(self.positions.len())?;
            let offsets = (checked(self.indices.len())?, checked(self.edges.len())?);
            self.positions.extend(
                asset
                    .data
                    .vertices
                    .iter()
                    .map(|v| [v.position[0], v.position[1], v.position[2], 1.0]),
            );
            for &index in &asset.data.indices {
                self.indices.push(
                    index
                        .checked_add(vertex)
                        .ok_or_else(|| failure("rebased vertex index exceeds u32"))?,
                );
            }
            self.edges.extend_from_slice(asset.edges);
            self.assets.insert(kind, offsets);
            offsets
        };
        let instance = checked(self.links.len())?;
        for triangle in 0..asset.data.indices.len() / 3 {
            self.triangles.push(Reference {
                offset: indices
                    .checked_add(checked(triangle * 3)?)
                    .ok_or_else(|| failure("triangle offset exceeds u32"))?,
                instance,
            });
        }
        for edge in 0..asset.edges.len() {
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
