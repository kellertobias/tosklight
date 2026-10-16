use super::*;
use media_domain::{MediaAddress, SpeedMultiplier};

fn at(millis: u64) -> Timestamp {
    Timestamp::from_millis(millis)
}

fn setup(intrinsic: Option<f64>, mode: PlayMode) -> (PlaybackSession, LayerState) {
    let mut timing = MediaTiming::from_frames(100, 10.0);
    timing.intrinsic_bpm = intrinsic;
    let session = PlaybackSession::new(
        AssetId::new(),
        timing,
        Arc::from((0..100).map(|n| n * 100_000).collect::<Vec<u64>>()),
        at(0),
        mode,
    );
    let layer = LayerState {
        address: MediaAddress::new(150, 6),
        play_mode: mode,
        ..Default::default()
    };
    (session, layer)
}

#[test]
fn paused_speed_changes_and_stale_loss_resume_without_adding_paused_time() {
    for intrinsic in [None, Some(60.0)] {
        let (mut session, mut layer) = setup(intrinsic, PlayMode::LoopSynced);
        let live = ResolvedTempo::Live { bpm: 60.0 };
        session.deliver(&layer, live, at(0));
        assert_eq!(session.deliver(&layer, live, at(300)).frame, Some(3));
        let paused = ResolvedTempo::Paused {
            bpm: 120.0,
            stale: false,
        };
        assert_eq!(session.deliver(&layer, paused, at(300)).frame, Some(3));
        layer.speed_multiplier = SpeedMultiplier::Multiply(2);
        let changed = ResolvedTempo::Paused {
            bpm: 240.0,
            stale: false,
        };
        assert_eq!(session.deliver(&layer, changed, at(1_000)).frame, Some(3));
        let lost = ResolvedTempo::Paused {
            bpm: 240.0,
            stale: true,
        };
        assert_eq!(session.deliver(&layer, lost, at(5_000)).frame, Some(3));
        let resumed = ResolvedTempo::Live { bpm: 120.0 };
        assert_eq!(session.deliver(&layer, resumed, at(6_000)).frame, Some(3));
        assert_eq!(
            session.deliver(&layer, resumed, at(6_100)).frame,
            Some(if intrinsic.is_some() { 7 } else { 5 })
        );
    }
}

#[test]
fn a_ranged_synced_pass_also_honors_pause_before_its_beat_fit() {
    let (mut session, mut layer) = setup(Some(60.0), PlayMode::LoopSynced);
    layer.in_point = 10;
    layer.out_point = 80;
    session.reconcile(&layer, at(0));
    let live = ResolvedTempo::Live { bpm: 60.0 };
    session.deliver(&layer, live, at(0));
    let before = session.deliver(&layer, live, at(300)).frame;
    let paused = ResolvedTempo::Paused {
        bpm: 120.0,
        stale: false,
    };
    assert_eq!(session.deliver(&layer, paused, at(300)).frame, before);
    assert_eq!(session.deliver(&layer, paused, at(3_300)).frame, before);
    assert_eq!(session.deliver(&layer, live, at(3_300)).frame, before);
    assert_ne!(session.deliver(&layer, live, at(3_600)).frame, before);
}

#[test]
fn running_signal_loss_keeps_last_rate_and_recovery_is_continuous() {
    let (mut session, layer) = setup(Some(60.0), PlayMode::LoopSynced);
    let live = ResolvedTempo::Live { bpm: 60.0 };
    session.deliver(&layer, live, at(0));
    let stale = ResolvedTempo::Stale { bpm: 60.0 };
    assert_eq!(session.deliver(&layer, stale, at(2_000)).frame, Some(20));
    assert_eq!(session.deliver(&layer, stale, at(2_500)).frame, Some(25));
    let recovered = ResolvedTempo::Live { bpm: 120.0 };
    assert_eq!(
        session.deliver(&layer, recovered, at(2_500)).frame,
        Some(25)
    );
    assert_eq!(
        session.deliver(&layer, recovered, at(2_600)).frame,
        Some(27)
    );
}

#[test]
fn initial_pause_has_a_frame_and_unsynced_playback_ignores_group_transport() {
    let paused = ResolvedTempo::Paused {
        bpm: 120.0,
        stale: false,
    };
    for intrinsic in [None, Some(60.0)] {
        let (mut synced, layer) = setup(intrinsic, PlayMode::LoopSynced);
        assert_eq!(synced.deliver(&layer, paused, at(0)).frame, Some(0));
        assert_eq!(synced.deliver(&layer, paused, at(2_000)).frame, Some(0));
        let live = ResolvedTempo::Live { bpm: 60.0 };
        assert_eq!(synced.deliver(&layer, live, at(2_000)).frame, Some(0));
        assert_eq!(synced.deliver(&layer, live, at(2_100)).frame, Some(1));
        let (mut free, layer) = setup(intrinsic, PlayMode::Loop);
        free.deliver(&layer, paused, at(0));
        assert_eq!(free.deliver(&layer, paused, at(500)).frame, Some(5));
    }
}
