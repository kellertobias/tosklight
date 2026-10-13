//! Output-only platform thread ownership, joined by the existing blocking bridge.

#[cfg(target_os = "macos")]
use light_output_thread_policy as macos;

pub(super) fn run(
    task: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use anyhow::Context;
        let worker = std::thread::Builder::new()
            .name("light-output-scheduler".into())
            .spawn(move || {
                match macos::request_interactive() {
                    Ok(()) => tracing::info!(requested_qos = macos::requested_class(), "private output scheduler requested interactive macOS QoS"),
                    Err(error) => tracing::warn!(%error, "macOS refused interactive output scheduling; output continues with inherited policy"),
                }
                task()
            })
            .context("failed to create private output scheduler thread")?;
        match worker.join() {
            Ok(outcome) => outcome,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
    #[cfg(not(target_os = "macos"))]
    task()
}

#[cfg(test)]
mod tests {
    use super::run;

    #[cfg(target_os = "macos")]
    #[test]
    fn actual_output_worker_requests_interactive_without_changing_caller() {
        let caller = super::macos::requested_class();
        let caller_id = std::thread::current().id();
        let (sample_tx, sample_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let ordinary = std::thread::spawn(move || {
            let before = super::macos::requested_class();
            sample_tx.send(()).expect("ordinary worker started");
            let _ = done_rx.recv();
            assert_eq!(
                super::macos::requested_class(),
                before,
                "unopted worker unchanged"
            );
        });
        sample_rx.recv().expect("ordinary worker sample");
        run(move || {
            assert_ne!(std::thread::current().id(), caller_id);
            assert_eq!(
                std::thread::current().name(),
                Some("light-output-scheduler")
            );
            assert_eq!(
                super::macos::requested_class(),
                super::macos::USER_INTERACTIVE
            );
            Ok(())
        })
        .expect("output worker completes");
        assert_eq!(super::macos::requested_class(), caller);
        done_tx.send(()).expect("release ordinary worker");
        ordinary.join().expect("unopted worker unchanged");
    }

    #[test]
    fn cancellation_finishes_and_joins_the_private_runtime() {
        let cancellation = tokio_util::sync::CancellationToken::new();
        let child = cancellation.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let bridge = std::thread::spawn(move || {
            run(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?
                    .block_on(async move {
                        started_tx.send(()).expect("scheduler runtime starts");
                        child.cancelled().await;
                        Ok(())
                    })
            })
        });
        started_rx.recv().expect("private runtime startup");
        cancellation.cancel();
        bridge
            .join()
            .expect("bridge joins private worker")
            .expect("cancelled runtime completes");
    }

    #[test]
    fn result_and_panic_payload_are_preserved() {
        let result = run(|| Err(anyhow::anyhow!("scheduler refused")));
        assert_eq!(
            result.expect_err("original failure").to_string(),
            "scheduler refused"
        );
        let panic = std::panic::catch_unwind(|| run(|| std::panic::panic_any(731_u32)))
            .expect_err("original panic");
        assert_eq!(panic.downcast_ref::<u32>(), Some(&731));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn portable_output_keeps_existing_blocking_bridge_thread() {
        let caller = std::thread::current().id();
        run(move || {
            assert_eq!(std::thread::current().id(), caller);
            Ok(())
        })
        .expect("direct worker");
    }
}
