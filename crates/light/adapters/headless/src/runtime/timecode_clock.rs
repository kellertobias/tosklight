use std::sync::Arc;

use light_application::timeline::{SystemTimecodeClock, TimecodeClock};
use light_core::{ApplicationClock, ManualClock};

/// Production keeps a monotonic clock. The controlled bench shares its application clock with
/// Timecode so one advance moves cue timing, timeline position, and reconstructed output together.
pub(super) fn runtime_clock(manual: Option<&Arc<ManualClock>>) -> Arc<dyn TimecodeClock> {
    match manual {
        Some(clock) => Arc::new(BenchTimecodeClock(Arc::clone(clock))),
        None => Arc::new(SystemTimecodeClock::default()),
    }
}

struct BenchTimecodeClock(Arc<ManualClock>);

impl TimecodeClock for BenchTimecodeClock {
    fn now_micros(&self) -> u64 {
        u64::try_from(self.0.now().timestamp_micros()).unwrap_or(0)
    }
}
