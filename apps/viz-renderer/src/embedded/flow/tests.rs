use super::*;
use std::time::Duration;

fn row() -> Row {
    Row {
        pid: 123,
        elapsed_micros: 7,
        event: "begin_reserved",
        frame: Some(FrameId {
            generation: 1,
            slot: 1,
            sequence: 2,
        }),
        accepted: None,
        blocked_micros: 0,
        total_blocked_micros: 0,
        dropped_events_observed: 0,
    }
}
fn sink(capacity: usize, epoch: Instant) -> (Flow, Receiver<Row>) {
    let (sender, receiver) = mpsc::sync_channel(capacity);
    (
        Flow {
            epoch,
            sender: Some(sender),
            dropped: Arc::new(AtomicU64::new(0)),
            blocked_since: None,
            total_blocked: 0,
        },
        receiver,
    )
}
#[test]
fn frame_flow_is_default_off_and_requires_a_valid_absolute_destination() {
    assert!(Flow::from_environment(false).is_none());
    assert!(output_path(false, Some(Path::new("/unavailable/path")), 123).is_none());
    assert!(output_path(true, None, 123).is_none());
    assert!(output_path(true, Some(Path::new("relative.jsonl")), 123).is_none());
    assert!(
        output_path(
            true,
            Some(Path::new("/missing-stage-flow-parent/file")),
            123
        )
        .is_none()
    );
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("flow-probe");
    let path = output_path(true, Some(&base), 123).unwrap();
    assert_eq!(path.file_name().unwrap(), "flow-probe.123.jsonl");
    assert!(
        !path.exists(),
        "validating a destination must not create its file"
    );
}
#[test]
fn only_due_no_free_time_is_accumulated_and_transitions_are_coalesced() {
    let epoch = Instant::now();
    let (mut flow, receiver) = sink(16, epoch);
    flow.wait(epoch, false); // Deadline/hidden is not credit pressure.
    flow.wait(epoch + Duration::from_millis(10), true);
    flow.wait(epoch + Duration::from_millis(15), true);
    flow.wait(epoch + Duration::from_millis(18), false);
    flow.wait(epoch + Duration::from_millis(22), false);
    flow.wait(epoch + Duration::from_millis(30), true);
    flow.wait(epoch + Duration::from_millis(33), false);
    let rows: Vec<_> = receiver.try_iter().collect();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].event, "due_no_free_enter");
    assert_eq!(rows[1].blocked_micros, 8_000);
    assert_eq!(rows[3].blocked_micros, 3_000);
    assert_eq!(rows[3].total_blocked_micros, 11_000);
}
#[test]
fn bounded_queue_drops_without_waiting_and_exposes_missing_events() {
    let (mut flow, receiver) = sink(1, Instant::now());
    flow.emit(row());
    flow.emit(row());
    assert_eq!(flow.dropped.load(Ordering::Relaxed), 1);
    receiver.try_recv().unwrap();
    flow.emit(row());
    assert_eq!(receiver.try_recv().unwrap().dropped_events_observed, 1);
    drop(receiver);
    flow.emit(row());
    assert!(flow.sender.is_none());
}
#[test]
fn actual_writer_caps_rows_and_bytes_and_retains_terminal_integrity_witness() {
    for (limit_rows, limit_bytes) in [(4, 8192), (100, 1600)] {
        let (sender, receiver) = mpsc::sync_channel(64);
        for _ in 0..20 {
            sender.send(row()).unwrap();
        }
        drop(sender);
        let mut bytes = Vec::new();
        write_rows(
            &mut bytes,
            receiver,
            Arc::new(AtomicU64::new(0)),
            limit_rows,
            limit_bytes,
            12345,
        )
        .unwrap();
        assert!(bytes.len() <= limit_bytes);
        let rows: Vec<serde_json::Value> = std::str::from_utf8(&bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(rows.len() <= limit_rows);
        assert_eq!(rows[0]["epochAnchorMicros"], 12345);
        assert_eq!(rows[0]["monotonicZeroMicros"], 0);
        let end = rows.last().unwrap();
        assert_eq!(end["event"], "diagnostic_stopped");
        assert_eq!(end["truncated"], true);
        assert!(end["droppedEventsObserved"].as_u64().unwrap() > 0);
    }
}
#[test]
fn writer_error_ends_observation_without_panicking() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    sender.send(row()).unwrap();
    drop(sender);
    assert!(write_rows(Broken, receiver, Arc::new(AtomicU64::new(0)), 4, 8192, 0).is_err());
}
