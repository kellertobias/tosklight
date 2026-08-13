//! Dynamic source textures, separate from immutable fixture gobo artwork.

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
    sequences: HashMap<Uuid, u64>,
    updated_at: HashMap<Uuid, Instant>,
    persistent: HashSet<Uuid>,
    average: HashMap<Uuid, [f32; 3]>,
    flicker: HashMap<Uuid, f32>,
    next_layer: u32,
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
            sequences: HashMap::new(),
            updated_at: HashMap::new(),
            persistent: HashSet::new(),
            average: HashMap::new(),
            flicker: HashMap::new(),
            next_layer: 1,
            uploads: 0,
        }
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
        if frame.width != EDGE
            || frame.height != EDGE
            || frame.rgba.len() != (EDGE * EDGE * 4) as usize
        {
            return Err(format!("media frame must be {EDGE}x{EDGE} RGBA8"));
        }
        if self.sequences.get(&frame.source_id) == Some(&frame.sequence) {
            return Ok(false);
        }
        let layer = if let Some(layer) = self.layers.get(&frame.source_id) {
            *layer
        } else {
            if self.next_layer >= MAX_SOURCES {
                return Err(format!("media source budget is {MAX_SOURCES}"));
            }
            let layer = self.next_layer;
            self.next_layer += 1;
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
        self.sequences.insert(frame.source_id, frame.sequence);
        self.updated_at.insert(frame.source_id, Instant::now());
        if frame.persistent {
            self.persistent.insert(frame.source_id);
        } else {
            self.persistent.remove(&frame.source_id);
        }
        self.uploads += 1;
        Ok(true)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_frames_hold_for_two_seconds_while_fallbacks_never_expire() {
        let now = Instant::now();
        assert!(available(false, Some(now - LAST_GOOD_HOLD), now));
        assert!(!available(
            false,
            Some(now - LAST_GOOD_HOLD - Duration::from_millis(1)),
            now
        ));
        assert!(available(true, Some(now - Duration::from_secs(60)), now));
    }

    #[test]
    fn sampled_average_drives_projector_colour_without_per_pixel_gpu_lights() {
        let pixels = [128, 64, 32, 255].repeat(128);
        let average = average_rgb(&pixels);
        assert!((average[0] - 128.0 / 255.0).abs() < 0.001);
        assert!((average[1] - 64.0 / 255.0).abs() < 0.001);
        assert!((average[2] - 32.0 / 255.0).abs() < 0.001);
    }
}
