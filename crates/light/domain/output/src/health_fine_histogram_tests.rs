use super::*;

fn record(health: &mut OutputHealth, micros: u64) {
    let coarse = OUTPUT_TICK_DURATION_BUCKET_BOUNDS_MICROS
        .iter()
        .position(|bound| micros <= *bound)
        .unwrap_or(19);
    health.tick_duration_bucket_counts[coarse] += 1;
    health.record_fine_tick_duration(micros);
}

#[test]
fn inclusive_fine_boundaries_and_tails_conserve_every_tick() {
    let mut health = OutputHealth::default();
    let cases = [
        (0, 0),
        (1, 0),
        (249, 0),
        (250, 0),
        (251, 1),
        (500, 1),
        (501, 2),
        (31_999, 127),
        (32_000, 127),
        (32_001, 128),
        (48_000, 128),
        (48_001, 129),
        (100_001, 131),
        (1_000_001, 132),
        (u64::MAX, 132),
    ];
    let mut expected = [0; 133];
    for (micros, bucket) in cases {
        record(&mut health, micros);
        expected[bucket] += 1;
    }
    let (counts, total) = health.complete_fine_tick_duration_counts().unwrap();
    assert_eq!(counts, &expected);
    assert_eq!(total, cases.len() as u64);
    assert_eq!(
        total,
        health.tick_duration_bucket_counts.iter().sum::<u64>()
    );
    let bounds = OUTPUT_FINE_TICK_DURATION_BUCKET_BOUNDS_MICROS;
    assert_eq!(bounds.len(), 133);
    assert!(bounds.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        bounds[..128],
        std::array::from_fn::<_, 128, _>(|i| (i as u64 + 1) * 250)
    );
    assert_eq!(
        bounds[128..],
        [48_000, 64_000, 100_000, 1_000_000, 9_007_199_254_740_991]
    );
}

#[test]
fn clone_preserves_complete_counts_and_show_reset_clears_both_histograms() {
    let mut health = OutputHealth {
        frame_hz: 44.0,
        ..OutputHealth::default()
    };
    record(&mut health, 8_001);
    record(&mut health, 8_250);
    assert_eq!(
        health.clone().complete_fine_tick_duration_counts(),
        health.complete_fine_tick_duration_counts()
    );
    health.reset_for_new_show();
    assert_eq!(health.frame_hz, 44.0);
    assert_eq!(health.tick_duration_bucket_counts, [0; 20]);
    assert!(health.complete_fine_tick_duration_counts().is_none());
    record(&mut health, 500);
    assert_eq!(health.complete_fine_tick_duration_counts().unwrap().1, 1);
}

#[test]
fn legacy_health_json_round_trip_keeps_coarse_data_and_refuses_partial_fine_history() {
    let mut health = OutputHealth::default();
    record(&mut health, 7_001);
    let json = serde_json::to_value(&health).unwrap();
    assert!(json.get("fine_tick_duration_counts").is_none());
    assert_eq!(
        json["tick_duration_bucket_counts"]
            .as_array()
            .unwrap()
            .len(),
        20
    );
    let mut restored: OutputHealth = serde_json::from_value(json).unwrap();
    assert_eq!(
        restored.tick_duration_bucket_counts,
        health.tick_duration_bucket_counts
    );
    assert!(restored.complete_fine_tick_duration_counts().is_none());
    record(&mut restored, 8_001);
    assert!(restored.complete_fine_tick_duration_counts().is_none());
    restored.reset_for_new_show();
    record(&mut restored, 8_001);
    assert!(restored.complete_fine_tick_duration_counts().is_some());
}

#[test]
fn empty_mismatched_or_overflowed_histograms_never_claim_complete_coverage() {
    let mut health = OutputHealth::default();
    assert!(health.complete_fine_tick_duration_counts().is_none());
    health.record_fine_tick_duration(123);
    assert!(health.complete_fine_tick_duration_counts().is_none());
    health.fine_tick_duration_counts.counts = [0; 133];
    health.fine_tick_duration_counts.counts[0] = u64::MAX;
    health.tick_duration_bucket_counts[0] = u64::MAX;
    health.record_fine_tick_duration(123);
    assert!(health.complete_fine_tick_duration_counts().is_none());
    health.reset_for_new_show();
    health.fine_tick_duration_counts.counts[0] = u64::MAX;
    health.fine_tick_duration_counts.counts[1] = 1;
    assert!(health.complete_fine_tick_duration_counts().is_none());
    health.reset_for_new_show();
    record(&mut health, 123);
    health.tick_duration_bucket_counts[1] = u64::MAX;
    assert!(health.complete_fine_tick_duration_counts().is_none());
}
