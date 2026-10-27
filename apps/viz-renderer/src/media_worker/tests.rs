//! No threads, sockets, decoder or GPU: queued fake producers use the real lifetime mailbox.
use super::*;
use std::sync::mpsc;
use viz_scene::uuid::Uuid;

fn scene() -> Scene {
    let mut scene = Scene::default();
    scene.media_sources.push(MediaSourceBinding {
        id: Uuid::from_u128(1),
        server_id: Uuid::from_u128(2),
        host: "127.0.0.1".into(),
        port: 4809,
        advertised_source_id: 17,
        name: "Program".into(),
        aspect_ratio: Some(16.0 / 9.0),
        fallback_rgba: None,
    });
    scene
}
fn reconcile(workers: &mut MediaWorkers, scene: &Scene, starts: &mut Vec<Arc<AtomicBool>>) {
    workers.reconcile_with(scene, |_, token, _| starts.push(token));
}
fn frame(id: Uuid, pixel: u8) -> MediaFrame {
    MediaFrame {
        source_id: id,
        sequence: 999,
        width: 1,
        height: 1,
        rgba: vec![pixel, 0, 0, 255],
        persistent: false,
    }
}
fn publish(workers: &MediaWorkers, token: &Arc<AtomicBool>, frame: MediaFrame) -> bool {
    workers.mailbox.lock().unwrap().publish(token, frame)
}
#[test]
fn stable_source_reuses_its_worker_until_any_connection_identity_changes() {
    let mut workers = MediaWorkers::default();
    let mut scene = scene();
    let mut starts = Vec::new();
    reconcile(&mut workers, &scene, &mut starts);
    let first = Arc::clone(&starts[0]);
    scene.media_sources[0].name = "Renamed".into();
    scene.media_sources[0].aspect_ratio = Some(1.0);
    reconcile(&mut workers, &scene, &mut starts);
    assert_eq!(starts.len(), 1);
    assert!(Arc::ptr_eq(
        &first,
        &workers.workers[&scene.media_sources[0].id].stop
    ));
    assert!(workers.offline_notice().unwrap().contains("Renamed"));
    for field in ["host", "port", "source", "server"] {
        let previous = Arc::clone(starts.last().unwrap());
        match field {
            "host" => scene.media_sources[0].host = "localhost".into(),
            "port" => scene.media_sources[0].port = 4811,
            "source" => scene.media_sources[0].advertised_source_id = 29,
            _ => scene.media_sources[0].server_id = Uuid::from_u128(3),
        }
        let before = starts.len();
        reconcile(&mut workers, &scene, &mut starts);
        assert_eq!(starts.len(), before + 1, "{field} must replace the worker");
        assert!(previous.load(Ordering::Acquire));
        assert!(!Arc::ptr_eq(&previous, starts.last().unwrap()));
        assert!(!starts.last().unwrap().load(Ordering::Acquire));
    }
    assert_eq!(workers.workers.len(), 1);
}

enum Delivery {
    Frame(Arc<AtomicBool>, MediaFrame),
    Health(Arc<AtomicBool>, bool),
}
#[test]
fn queued_retired_frames_and_health_cannot_replace_current_source_or_refresh_its_liveness() {
    let mut workers = MediaWorkers::default();
    let mut scene = scene();
    let id = scene.media_sources[0].id;
    let mut starts = Vec::new();
    reconcile(&mut workers, &scene, &mut starts);
    let old = Arc::clone(&starts[0]);
    assert!(publish(&workers, &old, frame(id, 10)));
    let adopted = workers.drain().pop().unwrap().sequence;
    assert!(publish(&workers, &old, frame(id, 20))); // Pending old pixels must also be cleared.
    let (send, receive) = mpsc::channel();
    send.send(Delivery::Frame(Arc::clone(&old), frame(id, 30)))
        .unwrap();
    send.send(Delivery::Health(Arc::clone(&old), true)).unwrap();
    scene.media_sources[0].advertised_source_id = 29;
    reconcile(&mut workers, &scene, &mut starts);
    let current = Arc::clone(starts.last().unwrap());
    assert!(workers.drain().is_empty());
    assert!(workers.offline_notice().is_some());
    for _ in 0..2 {
        match receive.recv().unwrap() {
            Delivery::Frame(token, frame) => assert!(!publish(&workers, &token, frame)),
            Delivery::Health(token, live) => {
                workers.mailbox.lock().unwrap().set_live(id, &token, live)
            }
        }
    }
    assert!(
        workers.offline_notice().is_some(),
        "retired health cannot mark the replacement live"
    );
    send.send(Delivery::Frame(Arc::clone(&current), frame(id, 50)))
        .unwrap();
    send.send(Delivery::Frame(Arc::clone(&old), frame(id, 99)))
        .unwrap();
    send.send(Delivery::Health(old, false)).unwrap();
    for _ in 0..3 {
        match receive.recv().unwrap() {
            Delivery::Frame(token, frame) => {
                publish(&workers, &token, frame);
            }
            Delivery::Health(token, live) => {
                workers.mailbox.lock().unwrap().set_live(id, &token, live)
            }
        }
    }
    let frames = workers.drain();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].rgba[0], 50);
    assert!(
        frames[0].sequence > adopted,
        "replacement must pass atlas monotonic sequence admission"
    );
    assert!(
        workers.offline_notice().is_none(),
        "retired offline cannot mark the replacement offline"
    );
}
#[test]
fn removal_drops_pending_pixels_and_readd_keeps_sequences_monotonic_and_mailbox_capacity_one() {
    let mut workers = MediaWorkers::default();
    let mut scene = scene();
    let source = scene.media_sources[0].clone();
    let mut starts = Vec::new();
    reconcile(&mut workers, &scene, &mut starts);
    let old = Arc::clone(&starts[0]);
    assert!(publish(&workers, &old, frame(source.id, 10)));
    let adopted = workers.drain().pop().unwrap().sequence;
    assert!(publish(&workers, &old, frame(source.id, 20)));
    scene.media_sources.clear();
    reconcile(&mut workers, &scene, &mut starts);
    assert!(old.load(Ordering::Acquire));
    assert!(workers.workers.is_empty());
    assert!(workers.drain().is_empty());
    assert!(workers.offline_notice().is_none());
    assert!(!publish(&workers, &old, frame(source.id, 99)));
    scene.media_sources.push(source.clone());
    reconcile(&mut workers, &scene, &mut starts);
    let current = starts.last().unwrap();
    for pixel in 0..1000 {
        assert!(publish(
            &workers,
            current,
            frame(source.id, (pixel % 255) as u8)
        ));
    }
    let frames = workers.drain();
    assert_eq!(frames.len(), 1);
    assert!(frames[0].sequence > adopted);
    assert_eq!(frames[0].rgba[0], (999 % 255) as u8);
    assert!(
        !publish(&workers, current, frame(Uuid::from_u128(7), 77)),
        "token cannot publish another source"
    );
}
#[test]
fn fallback_transition_retires_live_generation_and_uploads_once_then_live_resumes_with_new_sequence()
 {
    let mut workers = MediaWorkers::default();
    let mut scene = scene();
    let id = scene.media_sources[0].id;
    let mut starts = Vec::new();
    reconcile(&mut workers, &scene, &mut starts);
    let old = Arc::clone(&starts[0]);
    publish(&workers, &old, frame(id, 10));
    let live = workers.drain().pop().unwrap().sequence;
    scene.media_sources[0].fallback_rgba = Some(vec![20, 0, 0, 255]);
    reconcile(&mut workers, &scene, &mut starts);
    assert!(old.load(Ordering::Acquire));
    let fallback = workers.drain().pop().unwrap();
    assert!(fallback.persistent && fallback.sequence > live);
    assert!(!publish(&workers, &old, frame(id, 99)));
    reconcile(&mut workers, &scene, &mut starts);
    assert!(
        workers.drain().is_empty(),
        "unchanged static fallback must not upload every frame"
    );
    scene.media_sources[0].fallback_rgba = None;
    reconcile(&mut workers, &scene, &mut starts);
    let current = Arc::clone(starts.last().unwrap());
    assert!(publish(&workers, &current, frame(id, 30)));
    assert!(workers.drain().pop().unwrap().sequence > fallback.sequence);
    drop(workers);
    assert!(current.load(Ordering::Acquire));
}

#[test]
fn embedded_cpu_cache_rejects_rebound_and_removed_sources_without_live_upload_queue() {
    let mut workers = MediaWorkers::for_embedded_capture();
    let mut scene = scene();
    let id = scene.media_sources[0].id;
    let mut starts = Vec::new();
    reconcile(&mut workers, &scene, &mut starts);
    assert!(workers.frozen_media().unwrap().sources().is_empty());
    let valid = |pixel| MediaFrame {
        source_id: id,
        sequence: 1,
        width: EDGE,
        height: EDGE,
        rgba: vec![pixel; (EDGE * EDGE * 4) as usize],
        persistent: false,
    };
    let old = Arc::clone(&starts[0]);
    assert!(publish(&workers, &old, valid(12)));
    assert!(
        workers.drain().is_empty(),
        "CPU-only mode never queues atlas uploads"
    );
    let frozen = workers.frozen_media().unwrap();
    scene.media_sources[0].port += 1;
    reconcile(&mut workers, &scene, &mut starts);
    assert!(workers.frozen_media().unwrap().sources().is_empty());
    assert!(!publish(&workers, &old, valid(99)));
    assert!(publish(&workers, &starts[1], valid(35)));
    assert_eq!(
        workers.frozen_media().unwrap().sources()[0].frame.rgba[0],
        35
    );
    assert_eq!(frozen.sources()[0].frame.rgba[0], 12);
    scene.media_sources.clear();
    reconcile(&mut workers, &scene, &mut starts);
    assert!(workers.frozen_media().unwrap().sources().is_empty());
    assert!(!publish(&workers, &starts[1], valid(99)));
}

#[test]
fn changed_fallback_pixels_refresh_the_same_capture_source_and_preserve_frozen_look() {
    let mut workers = MediaWorkers::for_embedded_capture();
    let mut scene = scene();
    let id = scene.media_sources[0].id;
    let pixels = |value| vec![value; (EDGE * EDGE * 4) as usize];
    scene.media_sources[0].fallback_rgba = Some(pixels(12));
    let mut starts = Vec::new();
    reconcile(&mut workers, &scene, &mut starts);
    let first = workers.frozen_media().unwrap();
    let first_frame = &first.sources()[0].frame;
    assert_eq!(first_frame.source_id, id);
    assert_eq!(first_frame.rgba[0], 12);

    // Same source/server/connection identity, with newly authored fallback artwork.
    scene.media_sources[0].fallback_rgba = Some(pixels(35));
    reconcile(&mut workers, &scene, &mut starts);
    let second = workers.frozen_media().unwrap();
    let second_frame = &second.sources()[0].frame;
    assert_eq!(second_frame.source_id, id);
    assert_eq!(second_frame.rgba[0], 35);
    assert!(second_frame.sequence > first_frame.sequence);
    assert_eq!(
        first_frame.rgba[0], 12,
        "an already frozen image remains immutable"
    );

    reconcile(&mut workers, &scene, &mut starts);
    let unchanged = workers.frozen_media().unwrap();
    assert!(Arc::ptr_eq(second_frame, &unchanged.sources()[0].frame));
    assert!(
        starts.is_empty(),
        "static fallback updates start no worker/socket"
    );
    assert!(
        workers.drain().is_empty(),
        "CPU capture never queues GPU atlas uploads"
    );
}
