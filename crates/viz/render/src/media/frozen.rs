//! Bounded CPU ownership of adopted atlas pixels and immutable still-job snapshots.
use super::{EDGE, MAX_SOURCES, MediaAtlas, MediaFrame, available};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::Instant,
};
use viz_scene::uuid::Uuid;

#[derive(Clone, Debug)]
pub struct FrozenMediaSource {
    pub frame: Arc<MediaFrame>,
    /// Availability when snapshot() was called, independent of future live hold expiry.
    pub available: bool,
    pub flicker: f32,
}

#[derive(Clone, Debug, Default)]
pub struct FrozenMedia {
    sources: Vec<FrozenMediaSource>,
}
impl FrozenMedia {
    pub fn sources(&self) -> &[FrozenMediaSource] {
        &self.sources
    }

    /// Restore into a fresh capture-only atlas. Frozen availability is held indefinitely;
    /// expired sources still receive layers because a fallback ignores live availability.
    /// The original frames, live atlas and their persistent flags are never changed.
    pub fn restore_into(&self, atlas: &mut MediaAtlas, queue: &wgpu::Queue) -> Result<(), String> {
        if !atlas.layers.is_empty() {
            return Err("frozen media requires a fresh capture-only atlas".into());
        }
        for source in &self.sources {
            atlas.update(queue, &source.frame)?;
            let id = source.frame.source_id;
            if source.available {
                atlas.persistent.insert(id);
            } else {
                atlas.updated_at.remove(&id);
                atlas.persistent.remove(&id);
            }
            atlas.flicker.insert(id, source.flicker);
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct RetainedFrames {
    frames: BTreeMap<Uuid, Arc<MediaFrame>>,
}
impl RetainedFrames {
    /// Validation/sequence/budget checks precede cloning, so rejected pixels are never retained.
    pub(super) fn prepare(&self, frame: &MediaFrame) -> Result<Option<Arc<MediaFrame>>, String> {
        if frame.width != EDGE
            || frame.height != EDGE
            || frame.rgba.len() != (EDGE * EDGE * 4) as usize
        {
            return Err(format!("media frame must be {EDGE}x{EDGE} RGBA8"));
        }
        if let Some(previous) = self.frames.get(&frame.source_id) {
            if previous.sequence == frame.sequence {
                return Ok(None);
            }
        } else if self.frames.len() >= (MAX_SOURCES - 1) as usize {
            return Err(format!("media source budget is {MAX_SOURCES}"));
        }
        Ok(Some(Arc::new(frame.clone())))
    }

    pub(super) fn adopt(&mut self, frame: Arc<MediaFrame>) {
        self.frames.insert(frame.source_id, frame);
    }

    pub(super) fn snapshot(
        &self,
        now: Instant,
        updated: &HashMap<Uuid, Instant>,
        persistent: &HashSet<Uuid>,
        flicker: &HashMap<Uuid, f32>,
    ) -> FrozenMedia {
        FrozenMedia {
            sources: self
                .frames
                .iter()
                .map(|(id, frame)| FrozenMediaSource {
                    frame: Arc::clone(frame),
                    available: available(persistent.contains(id), updated.get(id).copied(), now),
                    flicker: flicker.get(id).copied().unwrap_or(0.0),
                })
                .collect(),
        }
    }
}
