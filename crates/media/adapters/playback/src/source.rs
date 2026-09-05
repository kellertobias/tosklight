//! The render-facing loader contract. Runtime implementations never wait for storage.
use crate::{ClipLoader, loader::LoadedClip};
use media_domain::{AssetId, SourceFailure};
use std::{path::Path, sync::Arc};

pub trait MediaLoader {
    /// Begins one layer/mask selection, including its pending load.
    fn begin_selection(&mut self, asset: AssetId);
    /// A pending load returns `None`; errors remain visible to the source lifecycle.
    fn request_load(
        &mut self,
        asset: AssetId,
        path: &Path,
    ) -> Result<Option<LoadedClip>, SourceFailure>;
    fn finish_selection(&mut self, asset: AssetId);
    fn release_selection(&mut self, asset: AssetId);
    fn request_frame(
        &mut self,
        asset: AssetId,
        frame: usize,
        consumer: u64,
    ) -> Option<(usize, Arc<[u8]>)>;
    fn release_consumer(&mut self, _consumer: u64) {}
    fn retry(&mut self, _asset: AssetId) {}
    fn failure(&self, _asset: AssetId) -> Option<SourceFailure> {
        None
    }
}

impl MediaLoader for ClipLoader {
    fn begin_selection(&mut self, _asset: AssetId) {}
    fn request_load(
        &mut self,
        asset: AssetId,
        path: &Path,
    ) -> Result<Option<LoadedClip>, SourceFailure> {
        self.load(asset, path, &mut |_| {})
            .map(Some)
            .map_err(|error| {
                tracing::warn!(%asset, %error, "cannot load selected media");
                SourceFailure::MissingFile
            })
    }
    fn finish_selection(&mut self, asset: AssetId) {
        self.cache_mut().pin(asset);
    }
    fn release_selection(&mut self, asset: AssetId) {
        self.cache_mut().unpin(asset);
    }
    fn request_frame(
        &mut self,
        asset: AssetId,
        frame: usize,
        _consumer: u64,
    ) -> Option<(usize, Arc<[u8]>)> {
        self.frame(asset, frame).map(|payload| (frame, payload))
    }
}
