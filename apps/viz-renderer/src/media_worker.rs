//! Standalone-only CITP workers. Each unique source owns one capacity-one latest frame.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use viz_render::media::{EDGE, MediaFrame};
use viz_scene::{MediaSourceBinding, Scene};

struct Worker {
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct MediaWorkers {
    workers: HashMap<viz_scene::uuid::Uuid, Worker>,
    latest: Arc<Mutex<HashMap<viz_scene::uuid::Uuid, MediaFrame>>>,
    health: Arc<Mutex<HashMap<viz_scene::uuid::Uuid, (String, bool)>>>,
}

impl MediaWorkers {
    pub fn reconcile(&mut self, scene: &Scene) {
        let wanted = scene
            .media_sources
            .iter()
            .map(|source| source.id)
            .collect::<std::collections::HashSet<_>>();
        self.workers.retain(|id, worker| {
            if wanted.contains(id) {
                true
            } else {
                worker.stop.store(true, Ordering::Release);
                false
            }
        });
        if let Ok(mut health) = self.health.lock() {
            health.retain(|id, _| wanted.contains(id));
        }
        for source in &scene.media_sources {
            if let Some(rgba) = source.fallback_rgba.as_ref() {
                if let Ok(mut latest) = self.latest.lock() {
                    latest.entry(source.id).or_insert_with(|| MediaFrame {
                        source_id: source.id,
                        sequence: 1,
                        width: EDGE,
                        height: EDGE,
                        rgba: rgba.clone(),
                        persistent: true,
                    });
                }
                if let Ok(mut health) = self.health.lock() {
                    health.remove(&source.id);
                }
                continue;
            }
            if !self.workers.contains_key(&source.id) {
                self.start(source.clone());
            }
        }
    }

    fn start(&mut self, source: MediaSourceBinding) {
        let id = source.id;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let latest = Arc::clone(&self.latest);
        let health = Arc::clone(&self.health);
        if let Ok(mut state) = health.lock() {
            state.insert(id, (source.name.clone(), false));
        }
        std::thread::Builder::new()
            .name(format!("viz-media-{}", source.name))
            .spawn(move || run_source(source, thread_stop, latest, health))
            .ok();
        self.workers.insert(id, Worker { stop });
    }

    pub fn drain(&self) -> Vec<MediaFrame> {
        self.latest
            .lock()
            .map(|mut frames| frames.drain().map(|(_, frame)| frame).collect())
            .unwrap_or_default()
    }

    pub fn offline_notice(&self) -> Option<String> {
        let health = self.health.lock().ok()?;
        let names = health
            .values()
            .filter_map(|(name, live)| (!*live).then_some(name.as_str()))
            .collect::<Vec<_>>();
        (!names.is_empty()).then(|| {
            format!(
                "Media offline — showing fallback or black; retrying {}",
                names.join(", ")
            )
        })
    }
}

impl Drop for MediaWorkers {
    fn drop(&mut self) {
        for worker in self.workers.values() {
            worker.stop.store(true, Ordering::Release);
        }
    }
}

fn run_source(
    source: MediaSourceBinding,
    stop: Arc<AtomicBool>,
    latest: Arc<Mutex<HashMap<viz_scene::uuid::Uuid, MediaFrame>>>,
    health: Arc<Mutex<HashMap<viz_scene::uuid::Uuid, (String, bool)>>>,
) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    runtime.block_on(async move {
        let mut sequence = 0_u64;
        while !stop.load(Ordering::Acquire) {
            let address = tokio::net::lookup_host((source.host.as_str(), source.port))
                .await
                .ok()
                .and_then(|mut addresses| addresses.next());
            let Some(address) = address else {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            };
            let connected = light_media::CitpPreviewSubscription::connect(
                address,
                Duration::from_secs(2),
                source.advertised_source_id,
                EDGE as u16,
                EDGE as u16,
                10,
                Duration::from_secs(5),
            )
            .await;
            let Ok(mut subscription) = connected else {
                if let Ok(mut state) = health.lock()
                    && let Some((_, live)) = state.get_mut(&source.id)
                {
                    *live = false;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            };
            while !stop.load(Ordering::Acquire) {
                let Ok(image) = subscription.next_frame().await else {
                    break;
                };
                let decoded = match image.format {
                    light_media::ImageFormat::Jpeg | light_media::ImageFormat::Png => {
                        image::load_from_memory(&image.bytes).ok()
                    }
                    light_media::ImageFormat::Rgb8 => image::RgbImage::from_raw(
                        u32::from(image.width),
                        u32::from(image.height),
                        image.bytes,
                    )
                    .map(image::DynamicImage::ImageRgb8),
                };
                let Some(decoded) = decoded else { continue };
                let rgba = decoded
                    .resize_exact(EDGE, EDGE, image::imageops::FilterType::Triangle)
                    .to_rgba8()
                    .into_raw();
                sequence = sequence.wrapping_add(1).max(1);
                let frame = MediaFrame {
                    source_id: source.id,
                    sequence,
                    width: EDGE,
                    height: EDGE,
                    rgba,
                    persistent: false,
                };
                if let Ok(mut slot) = latest.lock() {
                    slot.insert(source.id, frame);
                }
                if let Ok(mut state) = health.lock()
                    && let Some((_, live)) = state.get_mut(&source.id)
                {
                    *live = true;
                }
            }
            if let Ok(mut state) = health.lock()
                && let Some((_, live)) = state.get_mut(&source.id)
            {
                *live = false;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });
}
