//! One storage allocation also carries the standard indexed-indirect arguments at offset zero.
use super::ProjectedEdge;
use bytemuck::{Pod, Zeroable};

/// First 20 bytes are DrawIndexedIndirectArgs; records begin at the next 16-byte boundary.
#[repr(C, align(16))]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(in crate::renderer::lines) struct ProjectedHeader {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
    pub padding: [u32; 3],
}
impl ProjectedHeader {
    pub const fn empty() -> Self {
        Self {
            index_count: 0,
            instance_count: 1,
            first_index: 0,
            base_vertex: 0,
            first_instance: 0,
            padding: [0; 3],
        }
    }
}

/// Every valid invocation reserves six indices; the counter must never wrap even when all
/// source edges survive. A depth-only chunk still binds one complete runtime-array record.
pub(in crate::renderer::lines) fn projected_size(edge_count: usize) -> Option<u64> {
    if edge_count > u32::MAX as usize / 6 {
        return None;
    }
    edge_count
        .max(1)
        .checked_mul(size_of::<ProjectedEdge>())?
        .checked_add(size_of::<ProjectedHeader>())
        .and_then(|bytes| u64::try_from(bytes).ok())
}
