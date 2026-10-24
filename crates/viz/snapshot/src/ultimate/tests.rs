use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let parent = std::env::var_os("LIGHT_TMP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp")
            });
        let directory = parent.join(format!(
            "ultimate-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn view() -> viz_scene::ViewConfiguration {
    let mut view = viz_scene::ViewConfiguration::default();
    view.ambient = 0.0;
    view.background = Some([0.0; 3]);
    view.fog_variation.lamp_cloudiness = 0.0;
    view.fog_variation.laser_cloudiness = 0.0;
    view
}
fn png(path: &Path, width: u32, height: u32) {
    let file = fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&vec![0; width as usize * height as usize * 4])
        .unwrap();
}

#[test]
fn publication_decodes_real_dimensions_preserves_existing_files_and_honors_cancel() {
    let scratch = Scratch::new();
    let staged = scratch.0.join("staged.png");
    let destination = scratch.0.join("operator.png");
    fs::write(&destination, b"user image must survive").unwrap();
    png(&staged, 3840, 2160);
    let cancel = AtomicBool::new(false);
    let image = publish_png(&staged, &destination, &cancel).unwrap();
    assert_eq!((image.width, image.height), (3840, 2160));
    assert_ne!(image.path, destination);
    assert_eq!(fs::read(&destination).unwrap(), b"user image must survive");
    assert!(
        staged.is_file(),
        "publisher never owns caller staging lifetime"
    );
    cancel.store(true, Ordering::Release);
    assert_eq!(
        publish_png(&staged, &destination, &cancel).unwrap_err(),
        UltimateError::Cancelled
    );
    assert!(
        image.path.is_file(),
        "cancellation never deletes completed images"
    );
}

#[test]
fn invalid_truncated_and_non_4k_images_never_publish() {
    let scratch = Scratch::new();
    let staged = scratch.0.join("invalid.png");
    let destination = scratch.0.join("operator.png");
    let cancel = AtomicBool::new(false);
    png(&staged, 32, 24);
    assert!(publish_png(&staged, &destination, &cancel).is_err());
    png(&staged, 3840, 2160);
    let file = fs::OpenOptions::new().write(true).open(&staged).unwrap();
    file.set_len(64).unwrap();
    assert!(publish_png(&staged, &destination, &cancel).is_err());
    assert!(!destination.exists());
}

#[test]
fn explicit_fidelity_report_is_bounded_and_refuses_known_missing_features() {
    let mut scene = viz_scene::Scene::default();
    scene.emitters.push(viz_scene::EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: String::new(),
        local_origin: glam::Vec3::ZERO,
        tilt_pivot: glam::Vec3::ZERO,
        local_orientation_degrees: glam::Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 12.0,
        field_angle_degrees: 36.0,
        optics: viz_scene::EmitterOptics::default(),
        kind: viz_scene::EmitterKind::Beam,
        cells: viz_scene::EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
    let mut values = viz_scene::SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 1.0;
    values.emitters[0].gobo = 0.5;
    values.emitters[0].prism = 0.5;
    values.emitters[0].shaper_blades[0] = 0.5;
    let mut report = UltimateEligibility::inspect(&scene, &values, &view());
    assert!(report.reasons().iter().any(|r| r.contains("gobo")));
    assert!(report.reasons().iter().any(|r| r.contains("prism")));
    assert!(report.reasons().iter().any(|r| r.contains("framing")));
    assert!(!report.is_supported());
    for index in 0..100 {
        report.require_unsupported(format!("additional feature {index}"));
    }
    assert_eq!(report.reasons().len(), 32);
    values.emitters[0].intensity = 0.0;
    values.emitters[0].held_intensity = 0.0;
    assert!(UltimateEligibility::inspect(&scene, &values, &view()).is_supported());
}

#[cfg(unix)]
fn executable(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}
#[cfg(unix)]
#[test]
fn subprocess_deadline_cancel_and_flooded_logs_are_bounded() {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut command = tokio::process::Command::new("/bin/sh");
    command.args(["-c", "exec sleep 30"]);
    let start = Instant::now();
    assert!(matches!(
        process::run(command, Duration::from_millis(80), &cancel, |_| {}),
        Err(UltimateError::TimedOut)
    ));
    assert!(start.elapsed() < Duration::from_secs(3));
    let mut command = tokio::process::Command::new("/bin/sh");
    command.args(["-c", "exec sleep 30"]);
    let token = cancel.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(60));
        token.store(true, Ordering::Release);
    });
    assert!(matches!(
        process::run(command, Duration::from_secs(3), &cancel, |_| {}),
        Err(UltimateError::Cancelled)
    ));
    worker.join().unwrap();
    cancel.store(false, Ordering::Release);
    let mut command = tokio::process::Command::new("/bin/sh");
    command.args(["-c", "i=0; while [ $i -lt 10000 ]; do printf '0123456789abcdef0123456789abcdef\\n'; i=$((i+1)); done"]);
    let outcome = process::run(command, Duration::from_secs(5), &cancel, |_| {}).unwrap();
    assert!(outcome.status.success());
    assert!(outcome.stdout.len() <= 32 * 1024);
}

#[cfg(unix)]
#[test]
fn capability_probe_checks_features_and_invalidates_changed_executable() {
    let scratch = Scratch::new();
    let path = scratch.0.join("fake-blender");
    let marker = "#!/bin/sh\nprintf '%s\\n' 'TOSKLIGHT_BLENDER_PROBE={\"version\":\"test\",\"gltf\":true,\"cycles\":true}'\n";
    executable(&path, marker);
    let cancel = AtomicBool::new(false);
    let mut cache = BlenderProbeCache::default();
    let capability = cache
        .probe(Some(&path), Duration::from_secs(2), &cancel)
        .unwrap();
    assert_eq!(capability.version, "test");
    executable(
        &path,
        "#!/bin/sh\nprintf '%s\\n' 'TOSKLIGHT_BLENDER_PROBE={\"version\":\"changed\",\"gltf\":false,\"cycles\":true}'\n",
    );
    assert!(capability.verify_identity().is_err());
    assert!(
        cache
            .probe(Some(&path), Duration::from_secs(2), &cancel)
            .unwrap_err()
            .to_string()
            .contains("glTF")
    );
}

#[cfg(unix)]
#[test]
fn managed_fake_backend_publishes_4k_and_leaves_frozen_package_and_user_images_intact() {
    let scratch = Scratch::new();
    let frozen = scratch.0.join("frozen");
    fs::create_dir(&frozen).unwrap();
    let source_png = scratch.0.join("fixture.png");
    png(&source_png, 3840, 2160);
    let mut scene = viz_scene::Scene::default();
    scene.bounds = viz_scene::Aabb {
        min: glam::Vec3::ZERO,
        max: glam::Vec3::ONE,
    };
    let values = viz_scene::SceneValues::default();
    let captured = crate::capture(
        &scene,
        &values,
        &crate::CaptureContext {
            show: "Frozen render".into(),
            source: "test".into(),
            scene_revision: scene.revision,
            look: crate::SnapshotLook::default(),
            camera: crate::SnapshotCamera::default(),
        },
    );
    fs::write(
        frozen.join(crate::DOCUMENT_FILE),
        serde_json::to_vec(&captured.document).unwrap(),
    )
    .unwrap();
    fs::write(frozen.join(crate::GEOMETRY_FILE), &captured.geometry).unwrap();
    let executable_path = scratch.0.join("fake-blender");
    let quoted = serde_json::to_string(source_png.to_str().unwrap()).unwrap();
    let script = format!(
        "#!/usr/bin/env python3\nimport sys,shutil,os,pathlib,runpy\nif '--python-expr' in sys.argv:\n print('TOSKLIGHT_BLENDER_PROBE={{\"version\":\"test\",\"gltf\":true,\"cycles\":true}}',flush=True)\nelse:\n budget=sys.argv.index('--threads')\n assert budget<sys.argv.index('--python')\n threads=int(sys.argv[budget+1])\n assert 1<=threads<=8 and os.environ['TOSKLIGHT_CAPTURE_THREADS']==str(threads)\n script=pathlib.Path(sys.argv[sys.argv.index('--python')+1])\n runpy.run_path(str(script.with_name('parent_guard.py')))['install_parent_guard']()\n shutil.copyfile({quoted},sys.argv[-1])\n print('TOSKLIGHT_PROGRESS image written',flush=True)\n"
    );
    executable(&executable_path, &script);
    let cancel = Arc::new(AtomicBool::new(false));
    let capabilities = BlenderProbeCache::default()
        .probe(Some(&executable_path), Duration::from_secs(3), &cancel)
        .unwrap();
    let destination = scratch.0.join("operator.png");
    fs::write(&destination, b"existing user image").unwrap();
    let request = UltimateRequest {
        frozen_directory: frozen.clone(),
        destination: destination.clone(),
        eligibility: UltimateEligibility::inspect(&scene, &values, &view()),
        timeout: Duration::from_secs(3),
    };
    let mut states = Vec::new();
    let image =
        render_ultimate(&request, &capabilities, cancel, |state| states.push(state)).unwrap();
    assert_eq!((image.width, image.height), (3840, 2160));
    assert!(matches!(states.last(), Some(UltimateProgress::Saved(_))));
    assert!(states.iter().any(|state|matches!(state,UltimateProgress::Rendering(detail) if detail.contains("background CPU limit"))));
    assert!(image.path.is_file());
    assert_eq!(fs::read(destination).unwrap(), b"existing user image");
    assert!(frozen.join(crate::DOCUMENT_FILE).is_file());
    assert!(!fs::read_dir(&scratch.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".tosklight-image-")
    }));
}

#[cfg(unix)]
#[test]
fn owned_python_guard_rejects_abandoned_launch_and_allows_current_owner() {
    let guard = include_str!("parent_guard.py");
    let cancel = AtomicBool::new(false);
    let mut abandoned = tokio::process::Command::new("python3");
    abandoned.arg("-c").arg(format!("{guard}\nos.environ['TOSKLIGHT_CAPTURE_PARENT_PID']='1073741824'\ninstall_parent_guard()\nraise RuntimeError('guard did not stop abandoned job')"));
    let result = process::run(abandoned, Duration::from_secs(3), &cancel, |_| {}).unwrap();
    assert_eq!(result.status.code(), Some(84));
    assert!(result.stderr.contains("capture owner exited"));
    let mut owned = tokio::process::Command::new("python3");
    owned.arg("-c").arg(format!(
        "{guard}\ninstall_parent_guard()\nprint('owner alive',flush=True)"
    ));
    let result = process::run(owned, Duration::from_secs(3), &cancel, |_| {}).unwrap();
    assert!(result.status.success());
    assert!(result.stdout.contains("owner alive"));
}
