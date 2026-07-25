use super::*;
use light_control::FrameRate;

#[test]
fn frame_selection_applies_overrides_and_reuses_held_frames() {
    let mut control = OutputControl::default();
    control.raw_overrides.insert((1, 2), 77);
    let live = output_frames(&mut control, frames_with_value(1, 10));
    assert_eq!(live[&1][0], 10);
    assert_eq!(live[&1][1], 77);

    control.hold = true;
    let held = output_frames(&mut control, frames_with_value(1, 99));
    assert_eq!(held, live);
}

#[test]
fn timecode_frame_uses_the_nominal_frame_rate() {
    let timecode = SmpteTimecode {
        hours: 1,
        minutes: 2,
        seconds: 3,
        frames: 4,
        rate: FrameRate::Fps25,
        source: "test".into(),
        received_at: chrono::Utc::now(),
    };
    assert_eq!(timecode_frame(&timecode), 93_079);
}

#[tokio::test]
async fn scheduler_gate_waits_for_start_and_cancels_without_deadlock() {
    let cancellation = CancellationToken::new();
    let (start, ready) = tokio::sync::oneshot::channel();
    let waiting = tokio::spawn({
        let cancellation = cancellation.clone();
        async move { await_start(ready, &cancellation).await }
    });
    tokio::task::yield_now().await;
    assert!(!waiting.is_finished());
    start.send(()).unwrap();
    assert!(waiting.await.unwrap());

    let (_start, ready) = tokio::sync::oneshot::channel();
    cancellation.cancel();
    assert!(!await_start(ready, &cancellation).await);
}

#[tokio::test]
async fn scheduler_gate_closes_when_the_start_owner_is_dropped() {
    let cancellation = CancellationToken::new();
    let (start, ready) = tokio::sync::oneshot::channel();
    drop(start);

    let allowed = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        await_start(ready, &cancellation),
    )
    .await
    .expect("a dropped start owner must not leave the scheduler gated");

    assert!(!allowed);
}

fn frames_with_value(universe: Universe, value: u8) -> HashMap<Universe, DmxFrame> {
    let mut frame = [0; light_output::DMX_SLOTS];
    frame[0] = value;
    HashMap::from([(universe, frame)])
}
