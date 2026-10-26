use super::*;

fn sample(pairs: &[(GpuPass, u64, u64)], period: f32) -> GpuPassTimings {
    let mut bytes = [0_u8; TIMESTAMP_BYTES as usize];
    let mut mask = 0;
    for &(pass, start, end) in pairs {
        mask |= 1 << pass.index();
        let (a, b) = pass.queries();
        for (index, ticks) in [(a, start), (b, end)] {
            let offset = index as usize * 8;
            bytes[offset..offset + 8].copy_from_slice(&ticks.to_le_bytes());
        }
    }
    decode_timings(&bytes, mask, period)
}

#[test]
fn identical_metal_pass_intervals_retain_durations_but_count_coverage_once() {
    let pairs = [
        GpuPass::Beams,
        GpuPass::Lasers,
        GpuPass::Composite,
        GpuPass::Overlay,
    ]
    .map(|pass| (pass, 1_000_000, 46_123_000));
    let timing = sample(&pairs, 1.0);
    assert_eq!(timing.total_micros(), Some(45_123));
    assert!(timing.overlapping_passes);
    for micros in [
        timing.beams_micros,
        timing.lasers_micros,
        timing.composite_micros,
        timing.overlay_micros,
    ] {
        assert_eq!(micros, Some(45_123));
    }
}

#[test]
fn disjoint_and_partially_overlapping_ranges_preserve_gaps_and_submicrosecond_coverage() {
    let disjoint = sample(
        &[(GpuPass::Cull, 0, 600), (GpuPass::Opaque, 1000, 1600)],
        1.0,
    );
    assert_eq!(
        (disjoint.cull_micros, disjoint.opaque_micros),
        (Some(0), Some(0))
    );
    assert_eq!(
        disjoint.total_micros(),
        Some(1),
        "round only after raw interval union"
    );
    assert!(!disjoint.overlapping_passes);
    let partial = sample(
        &[
            (GpuPass::Cull, 10, 30),
            (GpuPass::Opaque, 20, 40),
            (GpuPass::Composite, 50, 60),
        ],
        1000.0,
    );
    assert_eq!(partial.total_micros(), Some(40));
    assert!(partial.overlapping_passes);
}

#[test]
fn an_invalid_required_pair_invalidates_the_whole_sample_without_cheap_partial_total() {
    for pair in [
        (GpuPass::Composite, 40, 30),
        (GpuPass::Composite, u64::MAX - 5, 3),
        (GpuPass::Composite, u64::MAX, u64::MAX),
    ] {
        assert_eq!(
            sample(&[(GpuPass::Opaque, 10, 20), pair], 1000.0),
            GpuPassTimings::default()
        );
    }
    let bytes = [0_u8; 16];
    assert_eq!(
        decode_timings(&bytes, 1 << GpuPass::Composite.index(), 1.0),
        GpuPassTimings::default()
    );
    assert_eq!(sample(&[], 1.0).total_micros(), None);
}

#[test]
fn timestamp_conversion_rejects_invalid_periods_and_preserves_large_clock_precision() {
    for period in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
        assert_eq!(
            sample(&[(GpuPass::Opaque, 0, u64::MAX - 1)], period),
            GpuPassTimings::default()
        );
    }
    let start = u64::MAX - 50_001;
    assert_eq!(
        sample(&[(GpuPass::Opaque, start, start + 40_000)], 1.0).total_micros(),
        Some(40)
    );
}

#[test]
fn an_unrepresentable_union_invalidates_even_individually_representable_passes() {
    let timings = sample(
        &[
            (GpuPass::Opaque, 0, 1_000_000),
            (GpuPass::Composite, 2_000_000, 3_000_000),
        ],
        1e16,
    );
    assert_eq!(timings, GpuPassTimings::default());
}
