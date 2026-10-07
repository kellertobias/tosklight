//! Sampled tracing of output ticks whose phases exceed one slow-frame threshold.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

const SLOW_OUTPUT_PHASE_THRESHOLD: Duration = Duration::from_millis(20);
const SLOW_OUTPUT_PHASE_SAMPLE_LIMIT: u64 = 128;
static SLOW_OUTPUT_PHASE_SAMPLES: AtomicU64 = AtomicU64::new(0);

pub(super) fn trace_slow_output_phases(
    total: Duration,
    dynamic: Duration,
    engine: Duration,
    publish: Duration,
    send: Duration,
) {
    if total < SLOW_OUTPUT_PHASE_THRESHOLD {
        return;
    }
    let sample = SLOW_OUTPUT_PHASE_SAMPLES.fetch_add(1, Ordering::Relaxed);
    if sample >= SLOW_OUTPUT_PHASE_SAMPLE_LIMIT {
        return;
    }
    tracing::info!(
        sample = sample + 1,
        total_micros = duration_micros(total),
        dynamic_micros = duration_micros(dynamic),
        engine_micros = duration_micros(engine),
        publish_micros = duration_micros(publish),
        send_micros = duration_micros(send),
        "slow output tick phase sample"
    );
}

fn duration_micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}
