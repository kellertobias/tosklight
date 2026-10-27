//! Renderer-local lane demand; its serial prevents a delayed response surviving a double flip.
use crate::{client::DeskClient, wire::PreloadProjection};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Default)]
pub(super) struct OutputDemand(Arc<AtomicU64>);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OutputRequest(u64);
pub(super) struct OutputRead {
    pub(super) request: OutputRequest,
    pub(super) output: Box<crate::wire::OutputDmxSnapshot>,
    pub(super) preload: Option<Box<PreloadProjection>>,
}
impl OutputDemand {
    pub(super) fn snapshot(&self) -> OutputRequest {
        OutputRequest(self.0.load(Ordering::Acquire))
    }
    pub(super) fn accepts(&self, request: OutputRequest) -> bool {
        self.snapshot() == request
    }
    pub(super) fn set_following(&self, following: bool) {
        // Only the owning render thread changes demand. Advancing on each actual transition
        // preserves parity while rejecting Live -> Preload -> Live stale replies.
        let current = self.snapshot();
        if current.follows_preload() != following {
            self.0.store(
                current
                    .0
                    .checked_add(1)
                    .expect("Stage lane demand exhausted"),
                Ordering::Release,
            );
        }
    }
}
impl OutputRequest {
    fn follows_preload(self) -> bool {
        self.0 & 1 != 0
    }
    pub(super) async fn read(self, client: &DeskClient) -> Option<OutputRead> {
        let output = client.output_dmx(self.follows_preload()).await?;
        // Old servers expose a normalized overlay separately. Live never needs that read.
        let preload = if self.follows_preload() && output.native_protocol == 0 {
            client.preload_projection().await.map(Box::new)
        } else {
            None
        };
        Some(OutputRead {
            request: self,
            output: Box::new(output),
            preload,
        })
    }
}
#[cfg(test)]
mod tests;
