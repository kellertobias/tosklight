use super::*;
use std::{sync::mpsc, time::Instant};
use viz_render::{media::FrozenMedia, still::StillOrigin};
use viz_scene::{Scene, SceneValues, ViewConfiguration};

fn frozen() -> Arc<FrozenStill> {
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
fn fake_choice(job: &Job) -> Result<RenderedImage, UltimateError> {
    let mut choice = job.choice.0.lock().unwrap();
    choice.accepting = true;
    choice.ultimate_reasons = vec!["unsupported optics".into()];
    job.phase(Phase::NeedsChoice {
        blender_version: "fake".into(),
        ultimate_reasons: choice.ultimate_reasons.clone(),
        ultra_explanation: Engine::Ultra.explanation().into(),
        ultimate_explanation: Engine::Ultimate.explanation().into(),
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        job.check_cancel()?;
        if let Some(engine) = choice.selected.take() {
            choice.accepting = false;
            assert_eq!(engine, Engine::Ultra);
            job.update(|status| status.engine = Some(engine));
            return Ok(RenderedImage {
                path: PathBuf::from("frozen-test.png"),
                width: STILL_WIDTH,
                height: STILL_HEIGHT,
            });
        }
        assert!(Instant::now() < deadline, "fake chooser timed out");
        choice = job
            .choice
            .1
            .wait_timeout(choice, Duration::from_millis(10))
            .unwrap()
            .0;
    }
}
#[test]
fn one_worker_exact_job_choice_and_frozen_lane_survive_chooser() {
    let mut jobs = StillJobs::default();
    let frozen = frozen();
    let id = jobs
        .launch(frozen.clone(), StillOptions::default(), fake_choice)
        .unwrap();
    let status = await_status(&jobs, |phase| matches!(phase, Phase::NeedsChoice { .. }));
    assert_eq!(status.pane_id, "preload");
    assert!(status.follow_preload);
    assert_eq!(status.time_seconds, 17.0);
    assert!(
        jobs.launch(frozen, StillOptions::default(), fake_choice)
            .is_err()
    );
    let wrong = StillJobId("previous-helper-job".into());
    assert!(jobs.choose(&wrong, Engine::Ultra).is_err());
    assert!(jobs.cancel(&wrong).is_err());
    assert!(jobs.choose(&id, Engine::Ultimate).is_err());
    jobs.choose(&id, Engine::Ultra).unwrap();
    assert!(jobs.choose(&id, Engine::Ultra).is_err());
    let saved = await_status(&jobs, |phase| phase.is_terminal());
    assert!(matches!(saved.phase, Phase::Saved { .. }));
    assert_eq!(saved.engine, Some(Engine::Ultra));
    assert!(jobs.cancel(&id).is_err());
}
#[test]
fn cancelled_choice_finishes_before_replacement_and_ignores_late_progress() {
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
            Err(UltimateError::Cancelled)
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
            Err(UltimateError::Cancelled)
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
        Err(UltimateError::Cancelled)
    })
    .unwrap();
    drop(jobs);
    receive
        .recv_timeout(Duration::from_secs(3))
        .expect("owned worker received cancellation");
}
