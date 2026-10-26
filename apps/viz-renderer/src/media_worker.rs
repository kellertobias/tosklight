//! Standalone-only CITP workers. Each unique source owns one capacity-one latest frame.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use viz_render::media::{EDGE, MediaFrame};
use viz_scene::{MediaSourceBinding, Scene};

mod mailbox;
use mailbox::Mailbox;
#[cfg(test)]
mod tests;

#[derive(PartialEq, Eq)]
struct Binding {
    server: viz_scene::uuid::Uuid,
    host: String,
    port: u16,
    source: u16,
}
impl From<&MediaSourceBinding> for Binding {
    fn from(source: &MediaSourceBinding) -> Self {
        Self {
            server: source.server_id,
            host: source.host.clone(),
            port: source.port,
            source: source.advertised_source_id,
        }
    }
}
impl Binding {
    fn matches(&self, source: &MediaSourceBinding) -> bool {
        self.server == source.server_id
            && self.host == source.host
            && self.port == source.port
            && self.source == source.advertised_source_id
    }
}
struct Worker {
    stop: Arc<AtomicBool>,
    binding: Binding,
}

#[derive(Default)]
pub struct MediaWorkers {
    workers: HashMap<viz_scene::uuid::Uuid, Worker>,
    mailbox: Arc<Mutex<Mailbox>>,
}

impl MediaWorkers {
    pub fn reconcile(&mut self, scene: &Scene) {
        self.reconcile_with(scene, spawn_source);
    }

    fn reconcile_with(
        &mut self,
        scene: &Scene,
        mut start: impl FnMut(MediaSourceBinding, Arc<AtomicBool>, Arc<Mutex<Mailbox>>),
    ) {
        let wanted: HashMap<_, _> = scene
            .media_sources
            .iter()
            .map(|source| (source.id, source))
            .collect();
        self.workers.retain(|id, worker| {
            let keep = wanted.get(id).is_some_and(|source| {
                source.fallback_rgba.is_none() && worker.binding.matches(source)
            });
            if !keep {
                worker.stop.store(true, Ordering::Release);
            }
            keep
        });
        if let Ok(mut mailbox) = self.mailbox.lock() {
            mailbox.retain(&wanted);
        }
        for source in &scene.media_sources {
            if let Some(rgba) = source.fallback_rgba.as_ref() {
                if let Ok(mut mailbox) = self.mailbox.lock() {
                    mailbox.fallback(source, rgba);
                }
                continue;
            }
            if self.workers.contains_key(&source.id) {
                if let Ok(mut mailbox) = self.mailbox.lock() {
                    mailbox.rename(source);
                }
                continue;
            }
            let stop = Arc::new(AtomicBool::new(false));
            if let Ok(mut mailbox) = self.mailbox.lock() {
                mailbox.activate(source, Arc::clone(&stop));
            }
            self.workers.insert(
                source.id,
                Worker {
                    stop: Arc::clone(&stop),
                    binding: Binding::from(source),
                },
            );
            start(source.clone(), stop, Arc::clone(&self.mailbox));
        }
    }

    pub fn drain(&self) -> Vec<MediaFrame> {
        self.mailbox
            .lock()
            .map(|mut state| state.drain())
            .unwrap_or_default()
    }

    pub fn offline_notice(&self) -> Option<String> {
        self.mailbox.lock().ok()?.offline_notice()
    }
}

impl Drop for MediaWorkers {
    fn drop(&mut self) {
        for worker in self.workers.values() {
            worker.stop.store(true, Ordering::Release);
        }
    }
}

fn spawn_source(source: MediaSourceBinding, stop: Arc<AtomicBool>, mailbox: Arc<Mutex<Mailbox>>) {
    std::thread::Builder::new()
        .name(format!("viz-media-{}", source.name))
        .spawn(move || run_source(source, stop, mailbox))
        .ok();
}

fn run_source(source: MediaSourceBinding, stop: Arc<AtomicBool>, mailbox: Arc<Mutex<Mailbox>>) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    runtime.block_on(async move {
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
                if let Ok(mut state) = mailbox.lock() {
                    state.set_live(source.id, &stop, false);
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
                let frame = MediaFrame {
                    source_id: source.id,
                    sequence: 0, // Assigned only after the current lifetime is admitted.
                    width: EDGE,
                    height: EDGE,
                    rgba,
                    persistent: false,
                };
                if let Ok(mut state) = mailbox.lock() {
                    state.publish(&stop, frame);
                }
            }
            if let Ok(mut state) = mailbox.lock() {
                state.set_live(source.id, &stop, false);
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });
}
