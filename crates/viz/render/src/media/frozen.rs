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
    /// Adopt bounded immutable CPU pixels without allocating textures or touching a GPU.
    pub fn from_sources(mut sources: Vec<FrozenMediaSource>) -> Result<Self, String> {
        if sources.len() > (MAX_SOURCES - 1) as usize {
            return Err("Frozen media exceeds the native source budget".into());
        }
        let mut ids = HashSet::new();
        for source in &sources {
            let frame = &source.frame;
            if !ids.insert(frame.source_id) {
                return Err("Frozen media repeats a source identity".into());
            }
            if frame.width != EDGE
                || frame.height != EDGE
                || frame.rgba.len() != (EDGE * EDGE * 4) as usize
            {
                return Err(format!("Frozen media must be {EDGE}x{EDGE} RGBA8"));
            }
            if !source.flicker.is_finite() || !(0.0..=1.0).contains(&source.flicker) {
                return Err("Frozen media has invalid flicker".into());
            }
        }
        sources.sort_by_key(|source| source.frame.source_id);
        Ok(Self { sources })
    }

    pub fn sources(&self) -> &[FrozenMediaSource] {
        &self.sources
    }

    /// Merge immutable show-owned fallback pixels at the freeze boundary, with no network,
    /// decoder, texture allocation or GPU upload. Adopted frames remain authoritative.
    pub fn with_scene_fallbacks(mut self, scene: &viz_scene::Scene) -> Result<Self, String> {
        let wanted: HashSet<_> = scene
            .media_sections
            .iter()
            .flat_map(|section| [section.source_id, section.fallback_source_id])
            .flatten()
            .collect();
        if wanted.len() > (MAX_SOURCES - 1) as usize {
            return Err(format!(
                "The frozen scene needs {} media sources; the native atlas supports {}",
                wanted.len(),
                MAX_SOURCES - 1
            ));
        }
        self.sources
            .retain(|source| wanted.contains(&source.frame.source_id));
        let mut declared = HashSet::new();
        for source in scene
            .media_sources
            .iter()
            .filter(|source| wanted.contains(&source.id))
        {
            if !declared.insert(source.id) {
                return Err("The frozen scene repeats a media source identity".into());
            }
            if let Some(pixels) = source.fallback_rgba.as_ref() {
                if pixels.len() != (EDGE * EDGE * 4) as usize {
                    return Err(format!(
                        "Frozen fallback {} is not {EDGE}x{EDGE} RGBA8",
                        source.name
                    ));
                }
            }
        }
        for source in scene
            .media_sources
            .iter()
            .filter(|source| wanted.contains(&source.id))
        {
            if self
                .sources
                .iter()
                .any(|frozen| frozen.frame.source_id == source.id)
            {
                continue;
            }
            if let Some(pixels) = source.fallback_rgba.as_ref() {
                self.sources.push(FrozenMediaSource {
                    frame: Arc::new(MediaFrame {
                        source_id: source.id,
                        sequence: 1,
                        width: EDGE,
                        height: EDGE,
                        rgba: pixels.clone(),
                        persistent: true,
                    }),
                    available: true,
                    flicker: 0.0,
                });
            }
        }
        self.sources.sort_by_key(|source| source.frame.source_id);
        Ok(self)
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
