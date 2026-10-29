//! Dynamic source textures, separate from immutable fixture gobo artwork.

mod frozen;
#[cfg(test)]
mod tests;

use frozen::RetainedFrames;
pub use frozen::{FrozenMedia, FrozenMediaSource};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};
use viz_scene::uuid::Uuid;

pub const EDGE: u32 = 512;
pub const MAX_SOURCES: u32 = 32;
pub const LAST_GOOD_HOLD: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub struct MediaFrame {
    pub source_id: Uuid,
    pub sequence: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Static show-owned fallback frames never expire. Live CITP frames hold for two seconds.
    pub persistent: bool,
}

pub struct MediaAtlas {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    layers: BTreeMap<Uuid, u32>,
    retained: RetainedFrames,
    updated_at: HashMap<Uuid, Instant>,
    persistent: HashSet<Uuid>,
    average: HashMap<Uuid, [f32; 3]>,
    flicker: HashMap<Uuid, f32>,
    uploads: u64,
}

impl MediaAtlas {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viz media atlas"),
            size: wgpu::Extent3d {
                width: EDGE,
                height: EDGE,
                depth_or_array_layers: MAX_SOURCES,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("viz media atlas"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        // Every unassigned/no-frame source is black.
        queue.write_texture(
            texture.as_image_copy(),
            &vec![0; (EDGE * EDGE * 4) as usize],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(EDGE * 4),
                rows_per_image: Some(EDGE),
            },
            wgpu::Extent3d {
                width: EDGE,
                height: EDGE,
                depth_or_array_layers: 1,
            },
        );
        Self {
            texture,
            view,
            layers: BTreeMap::new(),
            retained: RetainedFrames::default(),
            updated_at: HashMap::new(),
            persistent: HashSet::new(),
            average: HashMap::new(),
            flicker: HashMap::new(),
            uploads: 0,
        }
    }

    /// Retire removed/rebound sources before adopting frames from their new producers.
    /// Layers are reused only after the next frame fully overwrites them. Frozen jobs retain
    /// their own immutable Arc pixels, independent of this live atlas's ownership.
    pub fn retain_sources(&mut self, sources: &HashSet<Uuid>) {
        self.layers.retain(|id, _| sources.contains(id));
        self.retained.retain(sources);
        self.updated_at.retain(|id, _| sources.contains(id));
        self.persistent.retain(|id| sources.contains(id));
        self.average.retain(|id, _| sources.contains(id));
        self.flicker.retain(|id, _| sources.contains(id));
    }

    pub fn layer_with_fallback(&self, source: Option<Uuid>, fallback: Option<Uuid>) -> u32 {
        let now = Instant::now();
        if let Some(id) = source
            && available(
                self.persistent.contains(&id),
                self.updated_at.get(&id).copied(),
                now,
            )
        {
            return self.layers.get(&id).copied().unwrap_or(0);
        }
        fallback
            .and_then(|id| self.layers.get(&id).copied())
            .unwrap_or(0)
    }

    pub fn update(&mut self, queue: &wgpu::Queue, frame: &MediaFrame) -> Result<bool, String> {
        let Some(adopted) = self.retained.prepare(frame)? else {
            return Ok(false);
        };
        let layer = if let Some(layer) = self.layers.get(&frame.source_id) {
            *layer
        } else {
            let layer = (1..MAX_SOURCES)
                .find(|candidate| !self.layers.values().any(|layer| layer == candidate))
                .ok_or_else(|| format!("media source budget is {MAX_SOURCES}"))?;
            self.layers.insert(frame.source_id, layer);
            layer
        };
        let average = average_rgb(&frame.rgba);
        let previous = self
            .average
            .insert(frame.source_id, average)
            .unwrap_or(average);
        self.flicker.insert(
            frame.source_id,
            ((average[0] - previous[0]).abs()
                + (average[1] - previous[1]).abs()
                + (average[2] - previous[2]).abs())
                / 3.0,
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &frame.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(EDGE * 4),
                rows_per_image: Some(EDGE),
            },
            wgpu::Extent3d {
                width: EDGE,
                height: EDGE,
                depth_or_array_layers: 1,
            },
        );
        self.retained.adopt(adopted);
        self.updated_at.insert(frame.source_id, Instant::now());
        if frame.persistent {
            self.persistent.insert(frame.source_id);
        } else {
            self.persistent.remove(&frame.source_id);
        }
        self.uploads += 1;
        Ok(true)
    }

    /// Freeze the actual invocation-time availability and adopted pixels without copying pixels.
    /// Later live updates and hold expiry cannot mutate this snapshot.
    pub fn snapshot(&self) -> FrozenMedia {
        self.retained.snapshot(
            Instant::now(),
            &self.updated_at,
            &self.persistent,
            &self.flicker,
        )
    }

    /// Stable identity of the currently presented live/fallback availability. It changes once
    /// when a live source's two-second hold expires, waking demand rendering for that transition.
    pub fn presentation_identity(&self) -> u64 {
        let now = Instant::now();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.uploads.hash(&mut hasher);
        for (id, layer) in &self.layers {
            id.hash(&mut hasher);
            layer.hash(&mut hasher);
            let available = available(
                self.persistent.contains(id),
                self.updated_at.get(id).copied(),
                now,
            );
            available.hash(&mut hasher);
        }
        hasher.finish()
    }

    pub fn appearance(&self, source: Option<Uuid>, fallback: Option<Uuid>) -> ([f32; 3], f32) {
        let layer = self.layer_with_fallback(source, fallback);
        let Some(id) = self
            .layers
            .iter()
            .find_map(|(id, candidate)| (*candidate == layer && layer != 0).then_some(*id))
        else {
            return ([0.0; 3], 0.0);
        };
        (
            self.average.get(&id).copied().unwrap_or([0.0; 3]),
            self.flicker.get(&id).copied().unwrap_or(0.0),
        )
    }

    pub fn upload_count(&self) -> u64 {
        self.uploads
    }
}

fn average_rgb(rgba: &[u8]) -> [f32; 3] {
    let mut total = [0_u64; 3];
    let mut count = 0_u64;
    for pixel in rgba.chunks_exact(4).step_by(64) {
        total[0] += u64::from(pixel[0]);
        total[1] += u64::from(pixel[1]);
        total[2] += u64::from(pixel[2]);
        count += 1;
    }
    if count == 0 {
        return [0.0; 3];
    }
    [
        total[0] as f32 / count as f32 / 255.0,
        total[1] as f32 / count as f32 / 255.0,
        total[2] as f32 / count as f32 / 255.0,
    ]
}

fn available(persistent: bool, updated: Option<Instant>, now: Instant) -> bool {
    persistent || updated.is_some_and(|updated| now.duration_since(updated) <= LAST_GOOD_HOLD)
}
