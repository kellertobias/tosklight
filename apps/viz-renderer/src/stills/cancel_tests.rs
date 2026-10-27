//! CPU lifecycle proof: cancellation retains single-worker ownership through cleanup.
use super::*;
use std::{fs, sync::mpsc, time::Instant};

#[test]
fn cancelled_owned_staging_is_cleaned_before_retry_and_never_publishes() {
    let root = std::env::var_os("LIGHT_TMP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.artifacts/tmp"));
    let directory = root.join(format!("native-cancel-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    let destination = directory.join("operator.png");
    let mut jobs = StillJobs::default();
    let (ready, started) = mpsc::channel();
    let (release, cleanup) = mpsc::channel();
    let frozen = super::tests::frozen();
    let id = jobs
        .launch(
            frozen.clone(),
            StillOptions {
                destination: Some(destination.clone()),
                ..Default::default()
            },
            move |job| {
                let workspace = storage::Workspace::new(job)?;
                fs::write(
                    workspace.directory.join("native.png"),
                    b"incomplete owned pixels",
                )?;
                job.phase(Phase::Rendering {
                    detail: "native work submitted".into(),
                });
                ready.send(()).unwrap();
                let deadline = Instant::now() + Duration::from_secs(3);
                while !job.cancel.load(Ordering::Acquire) {
                    assert!(Instant::now() < deadline);
                    std::thread::yield_now();
                }
                cleanup.recv_timeout(Duration::from_secs(3)).unwrap();
                drop(workspace);
                job.check_cancel()?;
                unreachable!("cancelled worker cannot publish")
            },
        )
        .unwrap();
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    jobs.cancel(&id).unwrap();
    assert!(jobs.is_busy());
    assert!(
        jobs.launch(frozen.clone(), StillOptions::default(), |_| unreachable!())
            .is_err()
    );
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while jobs.is_busy() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(jobs.status().unwrap().phase, Phase::Cancelled);
    assert!(!destination.exists());
    assert_eq!(
        fs::read_dir(&directory).unwrap().count(),
        0,
        "owned staging must be gone"
    );
    let retry = jobs
        .launch(frozen, StillOptions::default(), |_| {
            Err(ImageError::Cancelled)
        })
        .unwrap();
    assert_ne!(retry, id);
    assert!(jobs.cancel(&id).is_err());
    fs::remove_dir(directory).unwrap();
}
