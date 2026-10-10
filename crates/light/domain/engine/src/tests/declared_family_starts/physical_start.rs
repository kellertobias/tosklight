//! Accepted-frame activation seeds: real engine arbitration, distinct Cuelist identities.
use super::*;
use crate::{ContributionBatch, ContributionSample, PlaybackStartFrame};
use light_core::{MergeMode, TimedValue};

fn physical_rig() -> (Engine, FixtureId, Arc<ManualClock>) {
    let (engine, target, clock, _, _) = rig();
    let mut snapshot = (*engine.snapshot()).clone();
    snapshot.playbacks = snapshot
        .cue_lists
        .iter()
        .enumerate()
        .map(|(index, list)| {
            let mut definition = test_playback(index as u16 + 1, list.id);
            definition.auto_off = false;
            definition
        })
        .collect::<Vec<_>>()
        .into();
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::On);
    clock.advance_millis(3000);
    engine.set_control_timing([120.; 5], 0, 2000, 2000);
    (engine, target, clock)
}

fn publish(engine: &Engine, sampled: &[ContributionBatch]) -> PlaybackStartFrame {
    let rendered = engine
        .render_with_contribution_batches(Default::default(), sampled)
        .unwrap();
    PlaybackStartFrame::from_published(
        rendered.source_snapshot.clone(),
        rendered.resolved_values.clone(),
    )
}

fn on(engine: &Engine, number: u16, start: &PlaybackStartFrame) {
    engine
        .execute_playback_from_published(
            EnginePlaybackCommand::Pool {
                number,
                action: PoolPlaybackAction::On,
            },
            Some(start),
        )
        .unwrap();
}

fn green(engine: &Engine) -> AttributeValue {
    let snapshot = engine.snapshot();
    let profile = snapshot.fixtures[0]
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap();
    let mode = &profile.modes[0];
    let source = profile
        .native_color_identity(mode.id, mode.heads[0].id)
        .unwrap();
    let observation = NativeColorObservation {
        source,
        values: [0, 255, 0]
            .into_iter()
            .enumerate()
            .map(|(index, raw)| {
                let channel = &mode.channels[RED + index];
                NativeColorValue {
                    channel_id: channel.id,
                    function_id: channel.functions[0].id,
                    raw,
                }
            })
            .collect(),
    };
    AttributeValue::ColorProgram(Arc::new(
        snapshot
            .native_color_sources
            .capture_direct(observation)
            .unwrap()
            .program()
            .clone(),
    ))
}

#[test]
fn physical_on_starts_from_published_white_and_off_reveals_retained_base() {
    let (engine, target, clock) = physical_rig();
    let start = publish(&engine, &[]);
    on(&engine, 2, &start);
    clock.advance_millis(1000);
    let halfway = channels(&engine, target);
    assert!(
        halfway[1].abs_diff(128) <= 1,
        "physical ON must fade, not hold/snap: {halfway:?}"
    );
    assert!(
        engine
            .playback_runtime_status()
            .iter()
            .filter(|row| row.playback.enabled)
            .count()
            == 2
    );
    execute_pool(&engine, 2, PoolPlaybackAction::Off);
    assert_eq!(channels(&engine, target), vec![255, 255, 255]);
}

#[test]
fn physical_on_uses_winning_programmer_not_the_hidden_static_base() {
    let (engine, target, clock) = physical_rig();
    let session = SessionId::new();
    engine.programmers.start(session);
    engine.programmers.set(
        session,
        target,
        ProgrammingOwner::Color.key(),
        green(&engine),
    );
    let start = publish(&engine, &[]);
    on(&engine, 2, &start);
    assert_eq!(
        channels(&engine, target),
        vec![0, 255, 0],
        "programmer keeps its priority"
    );
    engine.programmers.clear(session);
    clock.advance_millis(1000);
    let halfway = channels(&engine, target);
    assert!(
        halfway[0].abs_diff(128) <= 1 && halfway[1].abs_diff(128) <= 1 && halfway[2] == 0,
        "start must be published green, not hidden white: {halfway:?}"
    );
}

#[test]
fn physical_on_uses_composed_external_sample_winner_not_static_peer() {
    let (engine, target, clock) = physical_rig();
    // Stateful producers, including Dynamics, supply semantic samples to this actual render.
    // This verifies arbitration/capture plumbing; a separate runtime test owns waveform proof.
    let sample = ContributionBatch::new([ContributionSample::independent(TimedValue {
        fixture_id: target,
        attribute: ProgrammingOwner::Color.key(),
        value: green(&engine),
        priority: 100,
        changed_at: clock.now(),
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })]);
    let start = publish(&engine, &[sample]);
    on(&engine, 2, &start);
    clock.advance_millis(1000);
    let halfway = channels(&engine, target);
    assert!(
        halfway[0].abs_diff(128) <= 1 && halfway[1].abs_diff(128) <= 1 && halfway[2] == 0,
        "{halfway:?}"
    );
}

#[test]
fn published_seed_does_not_steal_higher_priority_output() {
    let (engine, target, clock) = physical_rig();
    let mut snapshot = (*engine.snapshot()).clone();
    Arc::make_mut(&mut snapshot.cue_lists)[0].priority = 50;
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    let start = publish(&engine, &[]);
    on(&engine, 2, &start);
    clock.advance_millis(1000);
    assert_eq!(channels(&engine, target), vec![255, 255, 255]);
}

#[test]
fn stale_published_seed_cannot_cross_generation() {
    let (engine, _, _) = physical_rig();
    let start = publish(&engine, &[]);
    let mut snapshot = (*engine.snapshot()).clone();
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    on(&engine, 2, &start);
    assert!(
        engine
            .playback_runtime()
            .iter()
            .find(|row| row.playback_number == Some(2))
            .unwrap()
            .deleted_cue_transition_source
            .is_none()
    );
}

#[test]
fn zero_fade_and_ordinary_off_keep_immediate_semantics_with_seed() {
    let (engine, target, _) = physical_rig();
    engine.set_control_timing([120.; 5], 0, 0, 2000);
    let start = publish(&engine, &[]);
    on(&engine, 2, &start);
    assert_eq!(channels(&engine, target), vec![255, 0, 0]);
    execute_pool(&engine, 2, PoolPlaybackAction::Off);
    assert_eq!(channels(&engine, target), vec![255, 255, 255]);
}

#[test]
fn prepared_activation_uses_published_seed_and_rejection_leaves_live_untouched() {
    let (engine, target, clock) = physical_rig();
    let start = publish(&engine, &[]);
    let command = crate::PlaybackBatchCommand {
        number: 2,
        page: None,
        action: crate::PlaybackBatchAction::On,
        exclusion_zones: Arc::default(),
        activation_origin: None,
    };
    let rejected = engine
        .prepare_playback_batch_from_published(&[command.clone()], clock.now(), 2000, Some(&start))
        .unwrap();
    assert!(
        engine
            .install_prepared_playback_batch_with(
                rejected,
                |_| Err::<(), _>("dependent state rejected".to_string()),
                |_| ()
            )
            .is_err()
    );
    assert!(
        engine
            .playback_runtime()
            .iter()
            .all(|row| row.playback_number != Some(2))
    );
    let prepared = engine
        .prepare_playback_batch_from_published(&[command], clock.now(), 2000, Some(&start))
        .unwrap();
    engine.install_prepared_playback_batch(prepared).unwrap();
    clock.advance_millis(1000);
    let halfway = channels(&engine, target);
    assert!(halfway[1].abs_diff(128) <= 1, "{halfway:?}");
}
