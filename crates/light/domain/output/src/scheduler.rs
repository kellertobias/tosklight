//! Monotonic output scheduler and deadline accounting.

use crate::{OUTPUT_TICK_DURATION_BUCKET_BOUNDS_MICROS, OutputHealth};
use std::{
    future::Future,
    future::pending,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU16, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

// @tour rust-by-example:30 Inject asynchronous work with generic futures
// The scheduler owns timing and cancellation while callers provide the async tick operation.
// Tests can drive the real deadline loop without opening output sockets.

/// Runs output ticks independently from persistence and API work.
pub async fn run_scheduler<F, Fut>(
    rate_hz: u16,
    cancel: CancellationToken,
    health: Arc<Mutex<OutputHealth>>,
    tick: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = io::Result<u64>>,
{
    run_scheduler_dynamic(Arc::new(AtomicU16::new(rate_hz)), cancel, health, tick).await
}

pub async fn run_scheduler_dynamic<F, Fut>(
    rate_hz: Arc<AtomicU16>,
    cancel: CancellationToken,
    health: Arc<Mutex<OutputHealth>>,
    tick: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = io::Result<u64>>,
{
    run_scheduler_dynamic_inner(rate_hz, cancel, health, None, tick).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn programmer_wake_interrupts_the_next_scheduled_deadline() {
        let health = Mutex::new(OutputHealth::default());
        let cancel = CancellationToken::new();
        let wake = Notify::new();
        wake.notify_one();
        let mut deadline = Instant::now() + Duration::from_secs(1);

        assert!(
            !wait_for_deadline_or_wake(&health, &mut deadline, &cancel, Some(&wake)).await,
            "a programmer wake requests an immediate extra output tick"
        );
    }
}

pub async fn run_scheduler_dynamic_wakeable<F, Fut>(
    rate_hz: Arc<AtomicU16>,
    cancel: CancellationToken,
    health: Arc<Mutex<OutputHealth>>,
    wake: Arc<Notify>,
    tick: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = io::Result<u64>>,
{
    run_scheduler_dynamic_inner(rate_hz, cancel, health, Some(wake), tick).await;
}

async fn run_scheduler_dynamic_inner<F, Fut>(
    rate_hz: Arc<AtomicU16>,
    cancel: CancellationToken,
    health: Arc<Mutex<OutputHealth>>,
    wake: Option<Arc<Notify>>,
    mut tick: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = io::Result<u64>>,
{
    let mut deadline = Instant::now();
    let mut scheduled_tick = true;
    while !cancel.is_cancelled() {
        let current_rate = rate_hz.load(Ordering::Relaxed).clamp(40, 44);
        let interval = Duration::from_secs_f64(1.0 / f64::from(current_rate));
        let tick_started = Instant::now();
        record_tick_result(&health, current_rate, tick().await);
        record_tick_duration(&health, tick_started, interval);
        if scheduled_tick {
            deadline += interval;
        } else if Instant::now() >= deadline {
            // A requested extra tick that overlaps the next regular deadline has already
            // produced the frame due at that deadline. Advance the cadence instead of
            // reporting the useful extra work as a scheduler miss.
            deadline += interval;
        }
        scheduled_tick =
            wait_for_deadline_or_wake(&health, &mut deadline, &cancel, wake.as_deref()).await;
    }
}

fn record_tick_result(health: &Mutex<OutputHealth>, current_rate: u16, result: io::Result<u64>) {
    let mut current = health.lock().expect("output health mutex poisoned");
    match result {
        Ok(packets) => {
            current.frames_sent += 1;
            current.packets_sent += packets;
            current.frame_hz = f32::from(current_rate);
        }
        Err(_) => current.send_errors += 1,
    }
}

fn record_tick_duration(health: &Mutex<OutputHealth>, tick_started: Instant, interval: Duration) {
    let tick_micros = tick_started.elapsed().as_micros() as u64;
    let mut current = health.lock().expect("output health mutex poisoned");
    current.last_tick_micros = tick_micros;
    current.maximum_tick_micros = current.maximum_tick_micros.max(tick_micros);
    let bucket = OUTPUT_TICK_DURATION_BUCKET_BOUNDS_MICROS
        .partition_point(|upper_bound| *upper_bound < tick_micros);
    let bucket = bucket.min(current.tick_duration_bucket_counts.len() - 1);
    current.tick_duration_bucket_counts[bucket] += 1;
    current.scheduler_utilization =
        (tick_started.elapsed().as_secs_f64() / interval.as_secs_f64()) as f32;
}

async fn wait_for_deadline_or_wake(
    health: &Mutex<OutputHealth>,
    deadline: &mut Instant,
    cancel: &CancellationToken,
    wake: Option<&Notify>,
) -> bool {
    let now = Instant::now();
    if now > *deadline {
        let lateness = now.duration_since(*deadline).as_micros() as u64;
        let mut current = health.lock().expect("output health mutex poisoned");
        current.deadline_misses += 1;
        current.maximum_lateness_micros = current.maximum_lateness_micros.max(lateness);
        *deadline = now;
        return true;
    }
    let wake = async {
        match wake {
            Some(wake) => wake.notified().await,
            None => pending().await,
        }
    };
    tokio::select! {
        _ = tokio::time::sleep_until(tokio::time::Instant::from_std(*deadline)) => true,
        _ = cancel.cancelled() => true,
        _ = wake => false,
    }
}
