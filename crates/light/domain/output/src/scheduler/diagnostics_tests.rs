use super::*;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tracing::Instrument;

#[test]
fn only_explicit_all_enables_scheduler_diagnostics() {
    for value in [
        None,
        Some(""),
        Some("1"),
        Some("true"),
        Some("false"),
        Some("ALL"),
        Some(" all "),
    ] {
        assert!(!enabled(value));
    }
    assert!(enabled(Some("all")));
}
#[test]
fn admission_is_bounded_and_disabled_does_not_count() {
    let mut disabled = Budget {
        enabled: false,
        admitted: 0,
        cap_reported: false,
        origin: None,
    };
    assert!(disabled.next().is_none());
    assert_eq!(disabled.admitted, 0);
    assert!(!disabled.cap_reported);
    let mut active = Budget {
        enabled: true,
        admitted: 0,
        cap_reported: false,
        origin: Some(Instant::now()),
    };
    for _ in 0..LIMIT {
        assert!(active.next().is_some());
    }
    assert_eq!(active.admitted, LIMIT);
    assert!(!active.cap_reported);
    assert!(active.next().is_none());
    assert!(active.cap_reported);
    assert!(active.next().is_none());
    assert_eq!(active.admitted, LIMIT);
}

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Buffer {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}

#[test]
fn formatted_tick_sequence_survives_future_poll_boundaries() {
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buffer.clone())
        .without_time()
        .with_ansi(false)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let span = tracing::info_span!("output_tick", tick_seq = 1u64);
            async {
                tokio::task::yield_now().await;
                tracing::info!(
                    ordered_total_micros = 3u64,
                    "nested trace formatting fixture"
                );
            }
            .instrument(span.clone())
            .await;
            tick(
                &span,
                Duration::from_millis(25),
                Duration::from_millis(31),
                true,
                true,
                16,
            );
        });
    });
    let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines
            .iter()
            .all(|line| line.contains("output_tick{tick_seq=1}")),
        "{output}"
    );
    assert!(lines[0].contains("ordered_total_micros=3"));
    assert!(lines[1].contains("bounded output scheduler tick sample"));
    for field in [
        "schema_version=1",
        "interval_micros=25000",
        "tick_micros=31000",
        "scheduled_tick=1",
        "result_ok=1",
        "packets=16",
    ] {
        assert!(lines[1].contains(field), "{output}");
    }
    println!("{output}");
}

#[test]
fn signed_wait_offsets_preserve_early_and_late_original_targets() {
    let origin = Instant::now();
    let requested = origin + Duration::from_millis(20);
    assert_eq!(signed_micros(requested, origin), 20_000);
    assert_eq!(signed_micros(origin, requested), -20_000);
    assert_eq!(signed_micros(requested, requested), 0);
    assert!(WaitObservation::begin(None, requested, origin).is_none());
    assert_eq!(
        signed_micros(origin + Duration::from_millis(5), requested),
        -15_000
    );
    assert_eq!(
        signed_micros(origin + Duration::from_millis(55), requested),
        35_000
    );
}

#[test]
fn original_wait_target_and_signed_resume_are_retained_in_formatted_evidence() {
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buffer.clone())
        .without_time()
        .with_ansi(false)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let origin = Instant::now();
        let wait = WaitObservation::begin(
            Some(origin),
            origin + Duration::from_millis(20),
            origin + Duration::from_millis(5),
        )
        .unwrap();
        resumed_at(wait, 3, origin + Duration::from_millis(8));
        let overdue = WaitObservation::begin(
            Some(origin),
            origin + Duration::from_millis(20),
            origin + Duration::from_millis(55),
        )
        .unwrap();
        resumed_at(overdue, 0, origin + Duration::from_millis(55));
    });
    let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    let rows = output.lines().collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    for field in [
        "resume_reason=3",
        "requested_deadline_micros=20000",
        "wait_entered_micros=5000",
        "remaining_to_deadline_micros=15000",
        "actual_resume_micros=8000",
        "actual_wait_micros=3000",
        "resume_offset_micros=-12000",
    ] {
        assert!(rows[0].contains(field), "{output}");
    }
    for field in [
        "resume_reason=0",
        "requested_deadline_micros=20000",
        "wait_entered_micros=55000",
        "remaining_to_deadline_micros=-35000",
        "actual_resume_micros=55000",
        "actual_wait_micros=0",
        "resume_offset_micros=35000",
    ] {
        assert!(rows[1].contains(field), "{output}");
    }
}

fn numeric_field(row: &str, key: &str) -> i64 {
    row.split_whitespace()
        .find_map(|part| part.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("missing {key}: {row}"))
        .parse()
        .unwrap()
}

#[test]
fn real_wait_branches_distinguish_timer_cancellation_notification_and_rebase() {
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buffer.clone())
        .without_time()
        .with_ansi(false)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let origin = Instant::now();
            let health = std::sync::Mutex::new(crate::OutputHealth::default());
            let cancel = tokio_util::sync::CancellationToken::new();
            let mut target = Instant::now() + Duration::from_millis(2);
            let timer_target = target;
            assert!(
                super::super::wait_for_deadline_or_wake(
                    &health,
                    &mut target,
                    Duration::from_millis(20),
                    &cancel,
                    None,
                    Some(origin)
                )
                .await
            );
            assert_eq!(target, timer_target);
            let cancelled = tokio_util::sync::CancellationToken::new();
            cancelled.cancel();
            target = Instant::now() + Duration::from_secs(10);
            let cancel_target = target;
            assert!(
                super::super::wait_for_deadline_or_wake(
                    &health,
                    &mut target,
                    Duration::from_millis(20),
                    &cancelled,
                    None,
                    Some(origin)
                )
                .await
            );
            assert_eq!(target, cancel_target);
            let wake = tokio::sync::Notify::new();
            wake.notify_one();
            target = Instant::now() + Duration::from_secs(10);
            let notified_target = target;
            assert!(
                !super::super::wait_for_deadline_or_wake(
                    &health,
                    &mut target,
                    Duration::from_millis(20),
                    &cancel,
                    Some(&wake),
                    Some(origin)
                )
                .await
            );
            assert_eq!(target, notified_target);
            target = Instant::now() - Duration::from_millis(100);
            let original = target;
            assert!(
                super::super::wait_for_deadline_or_wake(
                    &health,
                    &mut target,
                    Duration::from_millis(20),
                    &cancel,
                    None,
                    Some(origin)
                )
                .await
            );
            assert!(
                target > original,
                "existing missed-deadline rebase is preserved"
            );
            assert_eq!(health.lock().unwrap().deadline_misses, 1);
        });
    });
    let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    let rows = output
        .lines()
        .filter(|row| row.contains("bounded output scheduler wait result"))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 4, "{output}");
    for (row, reason) in rows.iter().zip([1, 2, 3, 0]) {
        assert_eq!(numeric_field(row, "resume_reason"), reason);
        let actual = numeric_field(row, "actual_resume_micros");
        let requested = numeric_field(row, "requested_deadline_micros");
        let entered = numeric_field(row, "wait_entered_micros");
        // Each offset truncates independently to microseconds, so allow one microsecond.
        assert!((actual - requested - numeric_field(row, "resume_offset_micros")).abs() <= 1);
        assert!(
            (requested - entered - numeric_field(row, "remaining_to_deadline_micros")).abs() <= 1
        );
        if reason == 2 || reason == 3 {
            assert!(actual < requested);
        }
        if reason == 0 {
            assert!(numeric_field(row, "resume_offset_micros") >= 100_000);
            assert_eq!(numeric_field(row, "actual_wait_micros"), 0);
        }
    }
}
