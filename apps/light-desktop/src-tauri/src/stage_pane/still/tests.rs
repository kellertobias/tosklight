use super::*;

fn status() -> Status {
    Status {
        job_id: StillJobId("owned-job".into()),
        request_id: Some("owned-request".into()),
        pane_id: "pane".into(),
        follow_preload: true,
        scene_revision: 7,
        values_frame: 12,
        time_seconds: 5.5,
        width: 3840,
        height: 2160,
        framing: "16:9".into(),
        engine: None,
        notice: None,
        phase: Phase::Queued,
    }
}

#[test]
fn unsolicited_and_stale_capture_observations_cannot_establish_ownership() {
    let mut incoming = status();
    assert!(accept_status(Some("owned-request"), "pane", None, &incoming).unwrap());
    incoming.request_id = None;
    assert!(
        !accept_status(None, "pane", None, &incoming).unwrap(),
        "None==None is not a capture request"
    );
    assert!(!accept_status(Some("owned-request"), "pane", None, &incoming).unwrap());
    incoming.request_id = Some("earlier-request".into());
    assert!(!accept_status(Some("owned-request"), "pane", None, &incoming).unwrap());
    incoming = status();
    let mut previous = incoming.clone();
    previous.job_id = StillJobId("other-job".into());
    assert!(!accept_status(Some("owned-request"), "pane", Some(&previous), &incoming).unwrap());
}

#[test]
fn capture_acknowledgements_require_valid_job_pane_time_and_dimensions() {
    let base = status();
    let mut invalid = vec![];
    let mut s = base.clone();
    s.job_id.0.clear();
    invalid.push(s);
    let mut s = base.clone();
    s.job_id.0 = "x".repeat(129);
    invalid.push(s);
    let mut s = base.clone();
    s.pane_id = "other-pane".into();
    invalid.push(s);
    let mut s = base.clone();
    s.width = 1920;
    invalid.push(s);
    let mut s = base.clone();
    s.height = 1080;
    invalid.push(s);
    let mut s = base.clone();
    s.time_seconds = -1.0;
    invalid.push(s);
    let mut s = base.clone();
    s.time_seconds = f32::NAN;
    invalid.push(s);
    for incoming in invalid {
        assert!(accept_status(Some("owned-request"), "pane", None, &incoming).is_err());
    }
}

#[test]
fn later_phases_keep_the_original_frozen_scene_values_time_and_lane() {
    let previous = status();
    let mut incoming = previous.clone();
    incoming.phase = Phase::Rendering {
        detail: "Rendering".into(),
    };
    assert!(accept_status(Some("owned-request"), "pane", Some(&previous), &incoming).unwrap());
    let mut changed = vec![];
    let mut s = incoming.clone();
    s.scene_revision += 1;
    changed.push(s);
    let mut s = incoming.clone();
    s.values_frame += 1;
    changed.push(s);
    let mut s = incoming.clone();
    s.time_seconds += 1.0;
    changed.push(s);
    let mut s = incoming.clone();
    s.follow_preload = false;
    changed.push(s);
    for s in changed {
        assert!(accept_status(Some("owned-request"), "pane", Some(&previous), &s).is_err());
    }
    assert!(!accept_status(Some("owned-request"), "pane", Some(&previous), &previous).unwrap());
    let mut terminal = previous.clone();
    terminal.phase = Phase::Cancelled;
    assert!(
        !accept_status(Some("owned-request"), "pane", Some(&terminal), &incoming).unwrap(),
        "a late worker observation cannot reopen a terminal capture"
    );
}

fn registered_capture_pane(panes: &StagePanes, key: &str) -> Arc<StagePane> {
    let pane = Arc::new(StagePane::default());
    pane.drawing
        .store(true, std::sync::atomic::Ordering::Release);
    panes.panes.lock().unwrap().insert(
        key.into(),
        RegisteredPane {
            pane: pane.clone(),
            live_3d: false,
        },
    );
    pane
}

#[test]
fn capture_admission_across_panes_is_atomic_and_does_not_hold_the_registry() {
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };
    let panes = Arc::new(StagePanes::default());
    let a = registered_capture_pane(&panes, "window:a");
    let b = registered_capture_pane(&panes, "window:b");
    let barrier = Arc::new(Barrier::new(2));
    let admitted = Arc::new(AtomicUsize::new(0));
    let workers: Vec<_> = [a, b]
        .into_iter()
        .enumerate()
        .map(|(index, pane)| {
            let panes = panes.clone();
            let barrier = barrier.clone();
            let admitted = admitted.clone();
            std::thread::spawn(move || {
                barrier.wait();
                panes.with_capture_start(|| {
                    assert!(
                        panes.panes.try_lock().is_ok(),
                        "registry must be released before pane start"
                    );
                    pane.capture.lock().unwrap().request = Some(format!("request-{index}"));
                    admitted.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
            })
        })
        .collect();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(admitted.load(Ordering::SeqCst), 1);
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .find_map(|r| r.as_ref().err())
            .unwrap()
            .contains("Another Stage image job")
    );
}

#[test]
fn capture_admission_releases_on_terminal_closed_and_interrupted_panes() {
    let panes = StagePanes::default();
    let pane = registered_capture_pane(&panes, "window:a");
    pane.capture.lock().unwrap().request = Some("owned-request".into());
    assert!(
        panes.with_capture_start(|| Ok(())).is_err(),
        "awaiting first acknowledgement is busy"
    );
    let mut terminal = status();
    terminal.phase = Phase::Cancelled;
    pane.capture.lock().unwrap().latest = Some(terminal);
    assert!(panes.with_capture_start(|| Ok(())).is_ok());
    pane.capture.lock().unwrap().latest = None;
    pane.drawing
        .store(false, std::sync::atomic::Ordering::Release);
    assert!(
        panes.with_capture_start(|| Ok(())).is_ok(),
        "closed panes cannot leave a global lease"
    );
    pane.capture_interrupted("renderer stopped");
    assert!(!pane.capture.lock().unwrap().pending());
    pane.drawing
        .store(true, std::sync::atomic::Ordering::Release);
    assert!(
        panes.with_capture_start(|| Ok(())).is_ok(),
        "reopened pane has no abandoned request"
    );
}
