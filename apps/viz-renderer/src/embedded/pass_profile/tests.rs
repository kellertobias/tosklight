use super::*;
use std::path::PathBuf;

struct Evidence(PathBuf);
impl Evidence {
    fn new() -> Self {
        let root = PathBuf::from(
            std::env::var_os("LIGHT_TMP_DIR").expect("canonical LIGHT_TMP_DIR for profile tests"),
        );
        std::fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "stage-pass-profile-test-{}.jsonl",
            uuid::Uuid::new_v4()
        )))
    }
}
impl Drop for Evidence {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn sample(id: u64) -> FrameStats {
    FrameStats {
        gpu_micros: Some(12),
        gpu_sample_id: Some(id),
        gpu_sample_matches_frame: true,
        gpu_passes: viz_render::GpuPassTimings {
            opaque_micros: Some(8),
            beams_micros: Some(4),
            interval_union_micros: Some(12),
            overlapping_passes: false,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn observe(profile: &mut PassProfile, stats: &FrameStats) {
    let mut admission = viz_scene::gpu_timing::TimingAdmission::new(
        viz_scene::gpu_timing::TimingCadence::OneHz,
        false,
    );
    admission.begin_frame(true, false);
    profile.observe(
        stats,
        &ViewConfiguration::default(),
        (1920, 540),
        7,
        true,
        Some(admission.coverage(3, 2)),
    );
}

#[test]
fn provenance_and_monotonic_sample_identity_gate_actual_jsonl_rows() {
    let evidence = Evidence::new();
    let mut profile = PassProfile::open(&evidence.0).unwrap();
    let mut stats = sample(7);
    stats.gpu_sample_matches_frame = false;
    observe(&mut profile, &stats);
    assert_eq!(std::fs::metadata(&evidence.0).unwrap().len(), 0);
    stats.gpu_sample_matches_frame = true;
    observe(&mut profile, &stats);
    observe(&mut profile, &stats);
    observe(&mut profile, &sample(6));
    observe(&mut profile, &sample(8));
    let text = std::fs::read_to_string(&evidence.0).unwrap();
    let rows: Vec<serde_json::Value> = text
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["gpu_sample_id"], 7);
    assert_eq!(rows[0]["gpu_interval_union_micros"], 12);
    assert_eq!(rows[0]["gpu_pass_intervals_overlap"], false);
    assert_eq!(rows[0]["timing_coverage"]["diagnosticCadence"], "oneHz");
    assert_eq!(rows[0]["timing_coverage"]["framesAdmitted"], 1);
    assert_eq!(rows[0]["timing_coverage"]["completedReadbacks"], 3);
    assert_eq!(rows[0]["timing_coverage"]["validSamples"], 2);
    assert_eq!(rows[0]["gpu_provenance_valid"], true);
    assert_eq!(rows[0]["pixels"], 1920 * 540);
    assert_eq!(rows[0]["pass_labels"][2], "opaque");
    assert_eq!(rows[0]["pass_micros"][2], 8);
    assert!(text.lines().all(|row| row.len() < ROW_BYTES));
    let mut missing = sample(9);
    missing.gpu_sample_id = None;
    assert!(!eligible(&missing, None));
    missing = sample(9);
    missing.gpu_passes = Default::default();
    assert!(!eligible(&missing, None));
}

#[test]
fn row_and_byte_caps_include_existing_appenders_without_an_unbounded_queue() {
    let evidence = Evidence::new();
    std::fs::write(&evidence.0, vec![b'\n'; MAX_ROWS as usize - 1]).unwrap();
    let mut first = PassProfile::open(&evidence.0).unwrap();
    let mut second = PassProfile::open(&evidence.0).unwrap();
    assert_eq!(first.append(b"{}\n").unwrap(), Append::Written);
    assert_eq!(second.append(b"{}\n").unwrap(), Append::Capped);
    assert_eq!(first.append(b"{}\n").unwrap(), Append::Capped);
    drop(first);
    drop(second);
    let file = OpenOptions::new().write(true).open(&evidence.0).unwrap();
    file.set_len(MAX_BYTES).unwrap();
    drop(file);
    let mut full = PassProfile::open(&evidence.0).unwrap();
    observe(&mut full, &sample(1));
    assert!(full.file.is_none());
    assert_eq!(std::fs::metadata(&evidence.0).unwrap().len(), MAX_BYTES);
    let mut row = RowBuffer::default();
    assert!(row.write_all(&[0; ROW_BYTES + 1]).is_err());
}

#[test]
fn contention_skips_without_consuming_sample_and_io_failure_disables_once() {
    let evidence = Evidence::new();
    let mut profile = PassProfile::open(&evidence.0).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&evidence.0)
        .unwrap();
    lock.lock().unwrap();
    observe(&mut profile, &sample(1));
    assert_eq!(profile.last_sample, None);
    lock.unlock().unwrap();
    observe(&mut profile, &sample(1));
    assert_eq!(profile.last_sample, Some(1));
    lock.set_len(0).unwrap();
    observe(&mut profile, &sample(2));
    assert!(profile.file.is_none());
    observe(&mut profile, &sample(3));
    assert_eq!(std::fs::metadata(&evidence.0).unwrap().len(), 0);
}
