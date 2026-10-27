use super::*;
use std::{sync::mpsc, time::Instant};
use viz_render::{media::FrozenMedia, still::StillOrigin};
use viz_scene::{Scene, SceneValues, ViewConfiguration};

pub(super) fn frozen() -> Arc<FrozenStill> {
    let mut scene = Scene::default();
    scene.bounds = viz_scene::Aabb {
        min: glam::Vec3::ZERO,
        max: glam::Vec3::ONE,
    };
    Arc::new(
        FrozenStill::new(
            &scene,
            &SceneValues::default(),
            &ViewConfiguration::default(),
            17.0,
            FrozenMedia::default(),
            StillOrigin {
                pane_id: "preload".into(),
                follow_preload: true,
            },
        )
        .unwrap(),
    )
}
fn await_status(jobs: &StillJobs, predicate: impl Fn(&Phase) -> bool) -> Status {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = jobs.status().filter(|status| predicate(&status.phase)) {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "bounded image worker deadline exceeded"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn one_native_worker_and_frozen_lane_survive_async_rendering() {
    let mut jobs = StillJobs::default();
    let frozen = frozen();
    let (ready, started) = mpsc::channel();
    let (release, held) = mpsc::channel();
    let id = jobs
        .launch(frozen.clone(), StillOptions::default(), move |job| {
            job.phase(Phase::Rendering {
                detail: "Native Ultra".into(),
            });
            ready.send(()).unwrap();
            held.recv_timeout(Duration::from_secs(3)).unwrap();
            Ok(RenderedImage {
                path: PathBuf::from("frozen-test.png"),
                width: STILL_WIDTH,
                height: STILL_HEIGHT,
            })
        })
        .unwrap();
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    let status = jobs.status().unwrap();
    assert_eq!(status.pane_id, "preload");
    assert!(status.follow_preload);
    assert_eq!(status.time_seconds, 17.0);
    assert_eq!(status.engine, Some(Engine::Ultra));
    assert!(
        jobs.launch(frozen, StillOptions::default(), |_| unreachable!())
            .is_err()
    );
    assert!(
        jobs.cancel(&StillJobId("previous-helper-job".into()))
            .is_err()
    );
    release.send(()).unwrap();
    let saved = await_status(&jobs, Phase::is_terminal);
    assert!(matches!(saved.phase, Phase::Saved { .. }));
    assert_eq!(saved.engine, Some(Engine::Ultra));
    assert!(jobs.cancel(&id).is_err());
}
#[test]
fn cancelled_native_job_finishes_before_replacement_and_ignores_late_progress() {
    let mut jobs = StillJobs::default();
    let id = jobs
        .launch(frozen(), StillOptions::default(), |job| {
            while !job.cancel.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            job.phase(Phase::Rendering {
                detail: "late progress".into(),
            });
            assert!(matches!(
                job.latest.lock().unwrap().as_ref().unwrap().phase,
                Phase::Cancelling { .. }
            ));
            Err(ImageError::Cancelled)
        })
        .unwrap();
    jobs.cancel(&id).unwrap();
    await_status(&jobs, |phase| matches!(phase, Phase::Cancelled));
    let deadline = Instant::now() + Duration::from_secs(3);
    while jobs.is_busy() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let replacement = jobs
        .launch(frozen(), StillOptions::default(), |_| {
            Err(ImageError::Cancelled)
        })
        .unwrap();
    assert_ne!(id, replacement);
    assert!(jobs.cancel(&id).is_err());
}
#[test]
fn dropping_controller_cancels_owned_worker_without_joining_ui() {
    let mut jobs = StillJobs::default();
    let (send, receive) = mpsc::channel();
    jobs.launch(frozen(), StillOptions::default(), move |job| {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !job.cancel.load(Ordering::Acquire) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        send.send(()).unwrap();
        Err(ImageError::Cancelled)
    })
    .unwrap();
    drop(jobs);
    receive
        .recv_timeout(Duration::from_secs(3))
        .expect("owned worker received cancellation");
}
