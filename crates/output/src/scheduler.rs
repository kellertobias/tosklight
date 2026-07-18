//! Monotonic output scheduler and deadline accounting.

use crate::OutputHealth;
use std::{
    future::Future,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU16, Ordering},
    },
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

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
    mut tick: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = io::Result<u64>>,
{
    let mut deadline = Instant::now();
    while !cancel.is_cancelled() {
        let current_rate = rate_hz.load(Ordering::Relaxed).clamp(40, 44);
        let interval = Duration::from_secs_f64(1.0 / f64::from(current_rate));
        let tick_started = Instant::now();
        record_tick_result(&health, current_rate, tick().await);
        record_tick_duration(&health, tick_started, interval);
        deadline += interval;
        wait_for_deadline(&health, &mut deadline).await;
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
    current.scheduler_utilization =
        (tick_started.elapsed().as_secs_f64() / interval.as_secs_f64()) as f32;
}

async fn wait_for_deadline(health: &Mutex<OutputHealth>, deadline: &mut Instant) {
    let now = Instant::now();
    if now <= *deadline {
        tokio::time::sleep_until(tokio::time::Instant::from_std(*deadline)).await;
        return;
    }
    let lateness = now.duration_since(*deadline).as_micros() as u64;
    let mut current = health.lock().expect("output health mutex poisoned");
    current.deadline_misses += 1;
    current.maximum_lateness_micros = current.maximum_lateness_micros.max(lateness);
    *deadline = now;
}
