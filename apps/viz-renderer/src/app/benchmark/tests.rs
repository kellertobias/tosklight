use super::*;
use std::time::Duration;
use viz_render::{FrameStats, RenderError};
use viz_scene::{ProviderCapabilities, ProviderEvent, ProviderKind, SceneProvider};
struct Quiet;
impl SceneProvider for Quiet {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            kind: ProviderKind::LightingDesk,
            available: true,
            unavailable_reason: None,
            default_host: String::new(),
            default_port: 0,
            uses_network_input: false,
        }
    }
    fn poll(&mut self) -> Vec<ProviderEvent> {
        vec![]
    }
    fn request_resync(&mut self) {}
    fn shutdown(&mut self) {}
}

#[test]
fn skipped_and_failed_draws_do_not_reuse_success_stats_or_inflate_fps() {
    let start = Instant::now();
    let mut sample = BenchmarkSample::new(ViewMode::Full3d, start);
    let stats = FrameStats {
        cpu_micros: 5000,
        acquire_micros: 2000,
        degraded: true,
        ..Default::default()
    };
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(1));
    sample.record_outcome(&Err(RenderError::SkipFrame), start + Duration::from_secs(2));
    sample.record_outcome(
        &Err(RenderError::Surface("lost".into())),
        start + Duration::from_secs(3),
    );
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(4));
    assert_eq!(
        (
            sample.attempted,
            sample.frames,
            sample.skipped,
            sample.failed
        ),
        (4, 2, 1, 1)
    );
    assert_eq!(sample.frames as f64 / sample.seconds, 0.5);
    assert_eq!(sample.frame_millis, vec![3000.0]);
    assert_eq!(sample.cpu_millis, vec![3.0, 3.0]);
    assert_eq!(sample.wait_millis, vec![2.0, 2.0]);
    assert_eq!(sample.degraded_frames, 2);
}

#[test]
fn only_completed_unique_matching_gpu_samples_enter_percentiles() {
    let start = Instant::now();
    let mut sample = BenchmarkSample::new(ViewMode::Full3d, start);
    let mut stats = FrameStats {
        gpu_sample_id: Some(7),
        gpu_sample_matches_frame: true,
        gpu_micros: Some(4000),
        ..Default::default()
    };
    stats.gpu_passes.beams_micros = Some(4000);
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(1));
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(2));
    stats.gpu_sample_id = Some(8);
    stats.gpu_sample_matches_frame = false;
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(3));
    stats.gpu_sample_matches_frame = true;
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(4));
    stats.gpu_sample_id = Some(9);
    stats.gpu_micros = Some(8000);
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(5));
    assert_eq!(sample.gpu_millis, vec![4.0, 8.0]);
    assert_eq!(sample.gpu_pass_millis[3], vec![4.0, 4.0]);
    assert_eq!(sample.rejected_gpu, 1);
}

#[test]
fn no_gpu_and_error_only_window_reports_no_gpu_samples_or_presentations() {
    let start = Instant::now();
    let mut sample = BenchmarkSample::new(ViewMode::Lines3d, start);
    sample.record_outcome(&Err(RenderError::SkipFrame), start + Duration::from_secs(4));
    assert_eq!(sample.frames, 0);
    assert_eq!(sample.seconds, 4.0);
    sample.record_outcome(&Ok(FrameStats::default()), start + Duration::from_secs(5));
    assert!(sample.gpu_millis.is_empty());
    assert!(sample.frame_millis.is_empty());
    assert_eq!(percentile(&sample.gpu_millis, 0.95), 0.0); // printed as unavailable, never numeric zero
}

#[test]
fn completed_draw_restores_session_before_all_view_benchmark_advances() {
    use crate::session::Session;
    use crate::settings::Options;
    let now = Instant::now();
    let mut application = Application::new(Options {
        helper: true,
        benchmark_seconds: Some(0.01),
        benchmark_all_views: true,
        ..Default::default()
    });
    application.epoch = now - Duration::from_secs(3);
    let session = Session::new(Box::new(Quiet), ProviderKind::LightingDesk, now);
    let measured = || Measured {
        quality: viz_scene::RenderQuality::High,
        output_size: (1280, 720),
        fixtures: 0,
        emitters: 0,
        show: String::new(),
        inputs: vec![],
        latency: (0.0, 0.0, 0.0),
        dmx_hz: 0.0,
        scene_ready: true,
    };
    application.finish_benchmark_frame(session, Some((measured(), Ok(FrameStats::default()), now)));
    assert_eq!(
        application.session.as_ref().unwrap().source_view.mode,
        ViewMode::ALL[0]
    );
    for expected in ViewMode::ALL.into_iter().skip(1) {
        // This is the actual take/restore lifecycle used by draw(), not a standalone set_view call.
        let session = application.session.take().unwrap();
        application.benchmark_view_started = Instant::now() - Duration::from_secs(1);
        application.finish_benchmark_frame(
            session,
            Some((measured(), Ok(FrameStats::default()), Instant::now())),
        );
        assert_eq!(
            application.session.as_ref().unwrap().source_view.mode,
            expected
        );
        assert_eq!(application.benchmark.last().unwrap().mode, expected);
    }
}

#[test]
fn queued_frame_after_final_report_cannot_count_or_report_again() {
    use crate::settings::Options;
    let mut application = Application::new(Options {
        helper: true,
        benchmark_seconds: Some(0.01),
        ..Default::default()
    });
    application.start_benchmark();
    application.benchmark_view_started = Instant::now() - Duration::from_secs(1);
    let measured = Measured {
        quality: viz_scene::RenderQuality::High,
        output_size: (1280, 720),
        fixtures: 0,
        emitters: 0,
        show: String::new(),
        inputs: vec![],
        latency: (0.0, 0.0, 0.0),
        dmx_hz: 0.0,
        scene_ready: true,
    };
    assert!(application.record_benchmark(Instant::now(), &Ok(FrameStats::default()), &measured));
    let count = application.benchmark[0].frames;
    assert!(!application.record_benchmark(Instant::now(), &Ok(FrameStats::default()), &measured));
    assert_eq!(application.benchmark[0].frames, count);
}

#[test]
fn overlapping_gpu_intervals_are_flagged_once_only_for_matching_unique_samples() {
    let start = Instant::now();
    let mut sample = BenchmarkSample::new(ViewMode::Full3d, start);
    let timing = viz_render::GpuPassTimings {
        beams_micros: Some(45_000),
        lasers_micros: Some(45_000),
        composite_micros: Some(45_000),
        overlay_micros: Some(45_000),
        interval_union_micros: Some(45_000),
        overlapping_passes: true,
        ..Default::default()
    };
    let mut stats = FrameStats {
        gpu_sample_id: Some(7),
        gpu_sample_matches_frame: true,
        gpu_micros: timing.total_micros(),
        gpu_passes: timing,
        ..Default::default()
    };
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(1));
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(2));
    stats.gpu_sample_id = Some(8);
    stats.gpu_sample_matches_frame = false;
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(3));
    stats.gpu_sample_matches_frame = true;
    sample.record_outcome(&Ok(stats), start + Duration::from_secs(4));
    assert_eq!(sample.gpu_millis, vec![45.0]);
    assert_eq!(sample.overlapping_gpu_samples, 1);
    assert_eq!(sample.rejected_gpu, 1);
    for pass in [3, 4, 6, 7] {
        assert_eq!(sample.gpu_pass_millis[pass], vec![45.0]);
    }
}

#[test]
fn single_view_start_preserves_selected_mode_authored_and_local_camera() {
    use crate::{session::Session, settings::Options};
    for cli_view in [None, Some(ViewMode::Lines3d)] {
        let mut application = Application::new(Options {
            helper: true,
            benchmark_seconds: Some(1.0),
            view: cli_view,
            ..Default::default()
        });
        let mut session = Session::new(Box::new(Quiet), ProviderKind::LightingDesk, Instant::now());
        session.source_view.mode = ViewMode::Lines3d;
        session.source_view.camera.position = glam::Vec3::new(6.5, 3.8, 12.0);
        session.source_view.camera.target = glam::Vec3::new(0.0, 2.5, 0.0);
        session.source_view.camera.fov_degrees = 54.0;
        let authored = session.source_view.camera;
        let mut local = authored;
        local.position.x = -5.0;
        application.camera.adopt(&local);
        application.camera_is_local = true;
        let before = application.camera.camera(&authored, ViewMode::Lines3d);
        application.session = Some(session);
        application.start_benchmark();
        let source = &application.session.as_ref().unwrap().source_view;
        assert_eq!(source.mode, ViewMode::Lines3d);
        assert_eq!(source.camera, authored);
        assert!(application.camera_is_local);
        assert_eq!(
            application.camera.camera(&source.camera, source.mode),
            before
        );
        assert_eq!(application.benchmark[0].mode, ViewMode::Lines3d);
        assert_eq!(application.benchmark_views(), vec![ViewMode::Lines3d]);
    }
}
