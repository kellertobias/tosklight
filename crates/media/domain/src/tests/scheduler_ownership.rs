use crate::{
    Applied, Command, CommandKind, CommandSource, LayerPersonality, MediaState, OutputId,
    OutputState, Timestamp, apply,
};

fn command(output: OutputId, source: CommandSource, millis: u64, active: bool) -> Command {
    Command::new(
        CommandKind::SetSchedulerOwnership {
            output,
            active,
            dmx_silence_millis: 10_000,
        },
        source,
        Timestamp::from_millis(millis),
    )
}

#[test]
fn dmx_preempts_autonomous_playback_and_unchanged_packets_refresh_its_deadline() {
    let output = OutputId::new();
    let mut state =
        MediaState::with_outputs(vec![OutputState::new(output, LayerPersonality::TwoLayers)]);
    assert_eq!(
        apply(
            &mut state,
            &command(output, CommandSource::Scheduler, 0, true)
        ),
        Applied::Changed
    );
    let generation = state.output(output).unwrap().ownership.generation;
    let dmx = |millis| {
        Command::new(
            CommandKind::SetLayerDimmer {
                output,
                layer: 0,
                dimmer: 0.5,
            },
            CommandSource::ArtNet,
            Timestamp::from_millis(millis),
        )
    };
    apply(&mut state, &dmx(100));
    assert!(!state.output(output).unwrap().ownership.scheduler_active);
    assert!(state.output(output).unwrap().ownership.generation > generation);
    apply(&mut state, &dmx(9000));
    assert_eq!(
        apply(
            &mut state,
            &command(output, CommandSource::Scheduler, 18999, true)
        ),
        Applied::RejectedNotOwner
    );
    assert_eq!(
        apply(
            &mut state,
            &command(output, CommandSource::Scheduler, 19000, true)
        ),
        Applied::Changed
    );
}

#[test]
fn a_manual_owner_blocks_scheduler_writes_and_a_failed_batch_changes_nothing() {
    let output = OutputId::new();
    let mut state =
        MediaState::with_outputs(vec![OutputState::new(output, LayerPersonality::TwoLayers)]);
    apply(
        &mut state,
        &command(output, CommandSource::Scheduler, 0, true),
    );
    apply(
        &mut state,
        &Command::new(
            CommandKind::TakeOverPlayback {
                output,
                take_over: true,
            },
            CommandSource::Web,
            Timestamp::ZERO,
        ),
    );
    assert!(!state.output(output).unwrap().ownership.scheduler_active);
    assert_eq!(
        apply(
            &mut state,
            &command(output, CommandSource::Scheduler, 20000, true)
        ),
        Applied::RejectedNotOwner
    );
    let before = state.clone();
    let commands = [
        Command::new(
            CommandKind::SetLayerDimmer {
                output,
                layer: 0,
                dimmer: 0.4,
            },
            CommandSource::Web,
            Timestamp::ZERO,
        ),
        Command::new(
            CommandKind::SetLayerDimmer {
                output,
                layer: 99,
                dimmer: 0.2,
            },
            CommandSource::Web,
            Timestamp::ZERO,
        ),
    ];
    assert_eq!(
        crate::state::reduce_commands(&state, &commands),
        Err(Applied::RejectedUnknownLayer)
    );
    assert_eq!(state, before);
}

#[test]
fn a_scheduler_cannot_write_before_acquisition_or_use_a_shorter_dmx_lease() {
    let output = OutputId::new();
    let mut state =
        MediaState::with_outputs(vec![OutputState::new(output, LayerPersonality::TwoLayers)]);
    let write = Command::new(
        CommandKind::SetLayerDimmer {
            output,
            layer: 0,
            dimmer: 0.2,
        },
        CommandSource::Scheduler,
        Timestamp::ZERO,
    );
    assert_eq!(apply(&mut state, &write), Applied::RejectedNotOwner);
    apply(
        &mut state,
        &Command::new(write.kind, CommandSource::Sacn, Timestamp::ZERO),
    );
    let acquire = Command::new(
        CommandKind::SetSchedulerOwnership {
            output,
            active: true,
            dmx_silence_millis: 1,
        },
        CommandSource::Scheduler,
        Timestamp::from_millis(2499),
    );
    assert_eq!(apply(&mut state, &acquire), Applied::RejectedNotOwner);
}

#[test]
fn autonomous_playback_reclaims_usable_homes_after_an_empty_desk_universe() {
    let output = OutputId::new();
    let mut state =
        MediaState::with_outputs(vec![OutputState::new(output, LayerPersonality::TwoLayers)]);
    let frame =
        crate::personality::decode::frame(LayerPersonality::TwoLayers, 1, &vec![0; 512]).unwrap();
    apply(
        &mut state,
        &Command::new(
            CommandKind::SetDmxFrame {
                output,
                frame: Box::new(frame),
            },
            CommandSource::ArtNet,
            Timestamp::ZERO,
        ),
    );
    assert_eq!(state.output(output).unwrap().master.scale_x, -4.0);
    apply(
        &mut state,
        &command(output, CommandSource::Scheduler, 10_000, true),
    );
    let restored = state.output(output).unwrap();
    assert_eq!(restored.master.scale_x, 1.0);
    assert_eq!(restored.layers[0].scale_x, 1.0);
    assert!(restored.ownership.scheduler_active);
}
