//! Dedicated Draft ownership: full-resolution geometry/lasers and half-resolution shafts.
//! Shared scene upload and opaque/shadow topology are retained; quality backends do not share
//! mutable targets, uniforms or shadow-cache identities.
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod math_tests;
mod passes;
mod preparation;
mod resources;
use super::{FrameContext, ShadedRenderer};
pub(super) use resources::DraftPasses;

pub(super) struct DraftRenderer {
    pub(super) scene: ShadedRenderer,
}
impl DraftRenderer {
    pub(super) fn new(context: &FrameContext<'_>, media: &crate::media::MediaAtlas) -> Self {
        Self {
            scene: ShadedRenderer::new_draft(context, media),
        }
    }
}
