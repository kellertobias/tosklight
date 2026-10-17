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
    };
    assert!(disabled.next().is_none());
    assert_eq!(disabled.admitted, 0);
    assert!(!disabled.cap_reported);
    let mut active = Budget {
        enabled: true,
        admitted: 0,
        cap_reported: false,
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
