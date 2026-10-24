//! Installed Blender cancellation after its actual render-start marker, never a fake process.
use super::*;
use glam::Vec3;
use std::{fs, path::Path, time::Instant};
use viz_render::{media::FrozenMedia, still::StillOrigin};
use viz_scene::*;

fn frozen_fixture() -> Arc<FrozenStill> {
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
        position: Vec3::new(0.0, 4.0, 0.0),
        installed_colour: [1.0; 3],
        body: FixtureBody {
            kind: BodyKind::Lantern,
            size: Vec3::splat(0.002),
        },
        ..Default::default()
    });
    scene.emitters.push(EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "Owned cancellation fixture".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 50.0,
        field_angle_degrees: 50.0,
        optics: EmitterOptics {
            sharpness: 1.0,
            uniformity: 1.0,
            ..Default::default()
        },
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Floor,
        position: Vec3::new(0.0, -0.05, 0.0),
        size: Vec3::new(12.0, 0.1, 12.0),
        colour: [0.4; 3],
        roughness: 1.0,
        ..Default::default()
    });
    scene.recompute_bounds();
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 0.8;
    values.emitters[0].colour = [1.0; 3];
    values.atmosphere.density = 0.0;
    let mut view = ViewConfiguration {
        quality: RenderQuality::Draft,
        mode: ViewMode::Full3d,
        ambient: 0.0,
        floor_grid: false,
        show_labels: false,
        background: Some([0.0; 3]),
        ..Default::default()
    };
    view.camera.position = Vec3::new(0.0, 5.0, 6.0);
    view.camera.target = Vec3::ZERO;
    Arc::new(
        FrozenStill::new(
            &scene,
            &values,
            &view,
            4.25,
            FrozenMedia::default(),
            StillOrigin {
                pane_id: "installed-blender-cancel-proof".into(),
                follow_preload: false,
            },
        )
        .unwrap(),
    )
}
fn wait_for(jobs: &StillJobs, deadline: Instant, matches: impl Fn(&Status) -> bool) -> Status {
    loop {
        let status = jobs.status().expect("owned job status");
        if matches(&status) {
            return status;
        }
        assert!(
            !status.phase.is_terminal(),
            "job finished before required observation: {:?}",
            status.phase
        );
        assert!(
            Instant::now() < deadline,
            "timed out waiting for owned capture phase: {:?}",
            status.phase
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn wait_cancelled_and_clean(jobs: &StillJobs, destination: &Path, directory: &Path) -> Status {
    let deadline = Instant::now() + Duration::from_secs(30);
    let terminal = wait_for(jobs, deadline, |status| status.phase.is_terminal());
    assert_eq!(
        terminal.phase,
        Phase::Cancelled,
        "cancellation must never publish a Saved result"
    );
    while jobs.is_busy() {
        assert!(
            Instant::now() < deadline,
            "cancelled worker did not finish its owned cleanup"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!destination.exists(), "cancelled job published a PNG");
    let leftovers: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(
        leftovers.is_empty(),
        "owned staging/output files survived cancellation: {leftovers:?}"
    );
    terminal
}

#[test]
#[ignore = "requires installed Blender and explicitly serialized real 4K cancellation"]
fn installed_blender_render_cancels_cleans_owned_staging_and_allows_retry() {
    let blender = PathBuf::from(
        std::env::var_os("LIGHT_STILL_TEST_BLENDER")
            .expect("explicit installed Blender executable"),
    );
    assert!(
        blender.is_file(),
        "configured Blender executable is missing"
    );
    let root = PathBuf::from(
        std::env::var_os("LIGHT_STILL_IMAGE_EVIDENCE_DIR").expect("canonical evidence directory"),
    );
    assert!(root.is_absolute(), "evidence directory must be absolute");
    fs::create_dir_all(&root).unwrap();
    let directory = root.join(format!("ultimate-cancel-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap(); // Test owns only this newly created directory.
    let destination = directory.join("cancelled.png");
    let frozen = frozen_fixture();
    let options = StillOptions {
        destination: Some(destination.clone()),
        configured_blender: Some(blender),
        probe_timeout: Duration::from_secs(30),
        ultimate_timeout: Duration::from_secs(180),
        cancel_cleanup_timeout: Duration::from_secs(30),
        ..Default::default()
    };
    let mut jobs = StillJobs::default();
    let id = jobs.start(frozen.clone(), options.clone()).unwrap();
    let choice = wait_for(&jobs, Instant::now() + Duration::from_secs(45), |status| {
        matches!(status.phase, Phase::NeedsChoice { .. })
    });
    let Phase::NeedsChoice {
        ultimate_reasons, ..
    } = &choice.phase
    else {
        unreachable!()
    };
    assert!(
        ultimate_reasons.is_empty(),
        "fixture is not eligible: {ultimate_reasons:?}"
    );
    jobs.choose(&id, Engine::Ultimate).unwrap();
    let rendering = wait_for(
        &jobs,
        Instant::now() + Duration::from_secs(90),
        |status| matches!(&status.phase, Phase::Rendering { detail } if detail.trim() == "rendering 3840x2160 Cycles PNG"),
    );
    assert_eq!(rendering.engine, Some(Engine::Ultimate));
    let cancelled_at = Instant::now();
    jobs.cancel(&id).unwrap(); // Only the controller's owned Blender process may be terminated.
    let cancelled = wait_cancelled_and_clean(&jobs, &destination, &directory);
    let cleanup_millis = cancelled_at.elapsed().as_millis();
    let retry = jobs
        .start(frozen, options)
        .expect("controller must be reusable after owned cleanup");
    assert_ne!(retry, id);
    jobs.cancel(&retry).unwrap(); // Queued/checking retry is cancelled without selecting an engine.
    let retried = wait_cancelled_and_clean(&jobs, &destination, &directory);
    let evidence = root.join(format!("ultimate-cancel-{}.json", uuid::Uuid::new_v4()));
    fs::write(
        &evidence,
        serde_json::to_vec_pretty(&serde_json::json!({
            "rendering": rendering, "cancelled": cancelled, "cleanupMillis": cleanup_millis,
            "retryCancelled": retried, "publishedPng": false, "ownedStagingRemaining": false,
            "directory": directory,
        }))
        .unwrap(),
    )
    .unwrap();
    fs::remove_dir(&directory).unwrap();
    println!(
        "Installed Blender cancellation evidence: {}",
        evidence.display()
    );
}
