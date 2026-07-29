//! Scheduler and delivery health reported to operators.

use serde::{Deserialize, Serialize};

/// Inclusive upper bounds for output scheduler tick-duration histogram buckets.
///
/// The final JavaScript-safe bound is the overflow bucket for any practical runtime tick.
pub const OUTPUT_TICK_DURATION_BUCKET_BOUNDS_MICROS: [u64; 20] = [
    250,
    500,
    750,
    1_000,
    1_250,
    1_500,
    2_000,
    3_000,
    4_000,
    6_000,
    8_000,
    12_000,
    16_000,
    24_000,
    32_000,
    48_000,
    64_000,
    100_000,
    1_000_000,
    9_007_199_254_740_991,
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OutputHealth {
    pub frames_sent: u64,
    pub packets_sent: u64,
    pub send_errors: u64,
    pub deadline_misses: u64,
    pub maximum_lateness_micros: u64,
    pub frame_hz: f32,
    pub last_tick_micros: u64,
    pub maximum_tick_micros: u64,
    pub tick_duration_bucket_counts: [u64; OUTPUT_TICK_DURATION_BUCKET_BOUNDS_MICROS.len()],
    pub scheduler_utilization: f32,
}
