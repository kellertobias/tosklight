//! Dedicated Draft ownership: reduced scene attachments and further-reduced shafts.
//! Shared scene upload and opaque/shadow topology are retained; quality backends do not share
//! mutable targets, uniforms or shadow-cache identities.
mod beam_cache;
mod composite_policy;
#[cfg(test)]
mod cone_bounds_tests;
mod fused;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod math_tests;
mod passes;
pub(in crate::renderer) use beam_cache::{BeamCache, Key as BeamCacheKey};
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

#[cfg(test)]
mod radial_profile_tests;

#[cfg(test)]
mod surface_math_tests;
