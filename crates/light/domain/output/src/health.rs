//! Scheduler and delivery health reported to operators.

use serde::{Deserialize, Serialize};

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
    pub scheduler_utilization: f32,
}
