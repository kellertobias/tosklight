use light_output::{OutputHealth, run_scheduler};
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn scheduler_updates_health_and_stops_on_cancellation() {
    let cancel = CancellationToken::new();
    let stop = cancel.clone();
    let health = Arc::new(Mutex::new(OutputHealth::default()));
    let result = Arc::clone(&health);
    let ticks = Arc::new(AtomicU64::new(0));
    let tick_count = Arc::clone(&ticks);

    run_scheduler(44, cancel, health, move || {
        let ticks = Arc::clone(&tick_count);
        let stop = stop.clone();
        async move {
            let count = ticks.fetch_add(1, Ordering::Relaxed) + 1;
            if count >= 3 {
                stop.cancel();
            }
            Ok(2)
        }
    })
    .await;

    let result = result.lock().unwrap().clone();
    assert_eq!(result.frames_sent, 3);
    assert_eq!(result.packets_sent, 6);
    assert_eq!(result.frame_hz, 44.0);
}

#[tokio::test]
async fn scheduler_records_a_slow_tick_as_a_missed_deadline() {
    let cancel = CancellationToken::new();
    let stop = cancel.clone();
    let health = Arc::new(Mutex::new(OutputHealth::default()));
    let result = Arc::clone(&health);

    run_scheduler(44, cancel, health, move || {
        let stop = stop.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(35)).await;
            stop.cancel();
            Ok(1)
        }
    })
    .await;

    let result = result.lock().unwrap().clone();
    assert_eq!(result.frames_sent, 1);
    assert_eq!(result.deadline_misses, 1);
    assert!(result.maximum_lateness_micros > 0);
    assert!(result.scheduler_utilization > 1.0);
}

#[tokio::test]
async fn a_cancelled_scheduler_does_not_start_another_tick() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let health = Arc::new(Mutex::new(OutputHealth::default()));
    let result = Arc::clone(&health);
    let ticks = Arc::new(AtomicU64::new(0));
    let tick_count = Arc::clone(&ticks);

    run_scheduler(44, cancel, health, move || {
        tick_count.fetch_add(1, Ordering::Relaxed);
        async { Ok::<_, io::Error>(0) }
    })
    .await;

    assert_eq!(ticks.load(Ordering::Relaxed), 0);
    assert_eq!(result.lock().unwrap().frames_sent, 0);
}
