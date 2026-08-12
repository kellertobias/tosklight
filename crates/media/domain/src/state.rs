//! The authoritative state model and its reducer.
//!
//! The reducer is the only writer. Readers take immutable snapshots; nothing else mutates state,
//! and no adapter holds a lock on it.

use serde::{Deserialize, Serialize};

use crate::command::{Command, CommandKind, CommandSource, ControlOwnership};
use crate::layer::LayerState;
use crate::master::MasterState;
use crate::output::OutputId;
use crate::personality::LayerPersonality;

/// One output's layers, master, and control ownership.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputState {
    pub id: OutputId,
    pub personality: LayerPersonality,
    /// Exactly as many layers as the personality controls. The legacy renderer always held eight
    /// while a flag changed how many DMX updated; here rendering, the API, the UI, CITP, and
    /// GDTF all read the same count.
    pub layers: Vec<LayerState>,
    pub master: MasterState,
    pub ownership: ControlOwnership,
}

impl OutputState {
    pub fn new(id: OutputId, personality: LayerPersonality) -> Self {
        Self {
            id,
            personality,
            layers: vec![LayerState::default(); usize::from(personality.layer_count())],
            master: MasterState::default(),
            ownership: ControlOwnership::default(),
        }
    }

    pub fn layer(&self, index: usize) -> Option<&LayerState> {
        self.layers.get(index)
    }
}

/// Every output this process hosts.
///
/// The first release ships one output and this is still a collection, so adding output two never
/// means replacing singleton state.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaState {
    pub outputs: Vec<OutputState>,
}

impl MediaState {
    pub fn with_outputs(outputs: Vec<OutputState>) -> Self {
        Self { outputs }
    }

    pub fn output(&self, id: OutputId) -> Option<&OutputState> {
        self.outputs.iter().find(|output| output.id == id)
    }

    fn output_mut(&mut self, id: OutputId) -> Option<&mut OutputState> {
        self.outputs.iter_mut().find(|output| output.id == id)
    }
}

/// What the reducer did with a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The command changed state.
    Changed,
    /// The command was valid and accepted, but state already said this.
    Unchanged,
    /// A live external DMX source owns the values this command wanted to write.
    RejectedNotOwner,
    /// The command addresses an output this process does not host.
    RejectedUnknownOutput,
    /// The command addresses a layer this output's personality does not have.
    RejectedUnknownLayer,
}

impl Applied {
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Changed | Self::Unchanged)
    }
}

/// Applies one command. The single write path into [`MediaState`].
pub fn apply(state: &mut MediaState, command: &Command) -> Applied {
    let id = command.kind.output();

    // Ownership is decided before the state is touched, so a rejected command leaves no trace.
    let Some(output) = state.output(id) else {
        return Applied::RejectedUnknownOutput;
    };
    if !output.ownership.accepts(command) {
        return Applied::RejectedNotOwner;
    }

    let output = state.output_mut(id).expect("looked up immediately above");

    if command.source.is_external_dmx() {
        output.ownership.observe_dmx(command.source, command.at);
    }

    match &command.kind {
        CommandKind::SetDmxFrame { frame, .. } => {
            // A frame carries exactly the personality's layer count. A desk cannot add layers by
            // sending a longer one.
            let mut changed = false;
            for (existing, incoming) in output.layers.iter_mut().zip(&frame.layers) {
                // Runtime status belongs to playback, not to the wire. A desk repeating the same
                // address must not reset a layer that has already failed or completed.
                let incoming = LayerState {
                    source_status: existing.source_status,
                    reset_trigger_id: existing.reset_trigger_id,
                    ..incoming.clone()
                };
                changed |= replace(existing, incoming);
            }
            changed |= replace(&mut output.master, frame.master);
            changed_or_not(changed)
        }
        CommandKind::SelectMedia { layer, address, .. } => {
            let Some(target) = output.layers.get_mut(*layer) else {
                return Applied::RejectedUnknownLayer;
            };
            changed_or_not(replace(&mut target.address, *address))
        }
        CommandKind::SetLayerDimmer { layer, dimmer, .. } => {
            let Some(target) = output.layers.get_mut(*layer) else {
                return Applied::RejectedUnknownLayer;
            };
            changed_or_not(replace(&mut target.dimmer, dimmer.clamp(0.0, 1.0)))
        }
        CommandKind::ResetLayer { layer, .. } => {
            let Some(target) = output.layers.get_mut(*layer) else {
                return Applied::RejectedUnknownLayer;
            };
            target.reset_trigger_id = target.reset_trigger_id.wrapping_add(1);
            Applied::Changed
        }
        CommandKind::TakeOverPlayback { take_over, .. } => {
            changed_or_not(replace(&mut output.ownership.web_takeover, *take_over))
        }
        CommandKind::ReportSourceStatus { layer, status, .. } => {
            let Some(target) = output.layers.get_mut(*layer) else {
                return Applied::RejectedUnknownLayer;
            };
            changed_or_not(replace(&mut target.source_status, *status))
        }
    }
}

fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        false
    } else {
        *slot = value;
        true
    }
}

const fn changed_or_not(changed: bool) -> Applied {
    if changed {
        Applied::Changed
    } else {
        Applied::Unchanged
    }
}

/// Marks the source unused when nothing external is sending, so the reducer's caller can tell a
/// quiet desk from a missing one.
pub const fn describe_source(source: CommandSource) -> &'static str {
    match source {
        CommandSource::ArtNet => "art-net",
        CommandSource::Sacn => "sacn",
        CommandSource::Web => "web",
        CommandSource::Internal => "internal",
        CommandSource::Recovery => "recovery",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::MediaAddress;
    use crate::command::Timestamp;
    use crate::layer::{SourceFailure, SourceStatus};
    use crate::personality::decode::DecodedFrame;

    fn state(personality: LayerPersonality) -> (MediaState, OutputId) {
        let id = OutputId::new();
        (
            MediaState::with_outputs(vec![OutputState::new(id, personality)]),
            id,
        )
    }

    fn command(kind: CommandKind, source: CommandSource, millis: u64) -> Command {
        Command::new(kind, source, Timestamp::from_millis(millis))
    }

    #[test]
    fn an_output_holds_exactly_the_layers_its_personality_controls() {
        assert_eq!(
            OutputState::new(OutputId::new(), LayerPersonality::TwoLayers)
                .layers
                .len(),
            2
        );
        assert_eq!(
            OutputState::new(OutputId::new(), LayerPersonality::EightLayers)
                .layers
                .len(),
            8
        );
    }

    #[test]
    fn a_command_for_an_unknown_output_is_refused() {
        let (mut media, _) = state(LayerPersonality::TwoLayers);
        let applied = apply(
            &mut media,
            &command(
                CommandKind::ResetLayer {
                    output: OutputId::new(),
                    layer: 0,
                },
                CommandSource::Web,
                0,
            ),
        );
        assert_eq!(applied, Applied::RejectedUnknownOutput);
    }

    #[test]
    fn a_command_for_a_layer_the_personality_lacks_is_refused() {
        let (mut media, id) = state(LayerPersonality::TwoLayers);
        let applied = apply(
            &mut media,
            &command(
                CommandKind::ResetLayer {
                    output: id,
                    layer: 5,
                },
                CommandSource::Web,
                0,
            ),
        );
        assert_eq!(applied, Applied::RejectedUnknownLayer);
    }

    #[test]
    fn the_web_ui_selects_media_until_a_desk_takes_over() {
        let (mut media, id) = state(LayerPersonality::TwoLayers);
        let select = |millis| {
            command(
                CommandKind::SelectMedia {
                    output: id,
                    layer: 0,
                    address: MediaAddress::new(1, 4),
                },
                CommandSource::Web,
                millis,
            )
        };

        assert_eq!(apply(&mut media, &select(0)), Applied::Changed);
        assert_eq!(
            apply(&mut media, &select(1)),
            Applied::Unchanged,
            "already selected"
        );
        assert_eq!(
            media.output(id).unwrap().layers[0].address,
            MediaAddress::new(1, 4)
        );

        let frame = DecodedFrame {
            layers: vec![LayerState::default(); 2],
            master: MasterState::default(),
        };
        let dmx = command(
            CommandKind::SetDmxFrame {
                output: id,
                frame: Box::new(frame),
            },
            CommandSource::ArtNet,
            1_000,
        );
        assert_eq!(apply(&mut media, &dmx), Applied::Changed);

        let blocked = command(
            CommandKind::SelectMedia {
                output: id,
                layer: 0,
                address: MediaAddress::new(2, 2),
            },
            CommandSource::Web,
            1_100,
        );
        assert_eq!(apply(&mut media, &blocked), Applied::RejectedNotOwner);
        assert_eq!(
            media.output(id).unwrap().layers[0].address,
            MediaAddress::BLANK
        );
    }

    #[test]
    fn a_rejected_command_leaves_no_trace() {
        let (mut media, id) = state(LayerPersonality::TwoLayers);
        let frame = DecodedFrame {
            layers: vec![LayerState::default(); 2],
            master: MasterState::default(),
        };
        apply(
            &mut media,
            &command(
                CommandKind::SetDmxFrame {
                    output: id,
                    frame: Box::new(frame),
                },
                CommandSource::Sacn,
                1_000,
            ),
        );
        let before = media.clone();

        apply(
            &mut media,
            &command(
                CommandKind::SetLayerDimmer {
                    output: id,
                    layer: 0,
                    dimmer: 0.25,
                },
                CommandSource::Web,
                1_100,
            ),
        );
        assert_eq!(media, before);
    }

    #[test]
    fn a_dmx_frame_does_not_overwrite_runtime_status_or_the_reset_trigger() {
        let (mut media, id) = state(LayerPersonality::TwoLayers);
        apply(
            &mut media,
            &command(
                CommandKind::ReportSourceStatus {
                    output: id,
                    layer: 0,
                    status: SourceStatus::Failed {
                        failure: SourceFailure::MissingFile,
                    },
                },
                CommandSource::Internal,
                0,
            ),
        );
        apply(
            &mut media,
            &command(
                CommandKind::ResetLayer {
                    output: id,
                    layer: 0,
                },
                CommandSource::Web,
                1,
            ),
        );
        let trigger = media.output(id).unwrap().layers[0].reset_trigger_id;

        let frame = DecodedFrame {
            layers: vec![LayerState::default(); 2],
            master: MasterState::default(),
        };
        apply(
            &mut media,
            &command(
                CommandKind::SetDmxFrame {
                    output: id,
                    frame: Box::new(frame),
                },
                CommandSource::ArtNet,
                2,
            ),
        );

        let layer = &media.output(id).unwrap().layers[0];
        assert!(
            layer.source_status.is_failed(),
            "the wire must not clear a failure"
        );
        assert_eq!(layer.reset_trigger_id, trigger);
    }

    #[test]
    fn resetting_a_layer_always_changes_the_trigger_without_touching_the_address() {
        let (mut media, id) = state(LayerPersonality::TwoLayers);
        apply(
            &mut media,
            &command(
                CommandKind::SelectMedia {
                    output: id,
                    layer: 0,
                    address: MediaAddress::new(3, 3),
                },
                CommandSource::Web,
                0,
            ),
        );
        let reset = command(
            CommandKind::ResetLayer {
                output: id,
                layer: 0,
            },
            CommandSource::Web,
            1,
        );
        assert_eq!(apply(&mut media, &reset), Applied::Changed);
        assert_eq!(
            apply(&mut media, &reset),
            Applied::Changed,
            "a second reset restarts again"
        );

        let layer = &media.output(id).unwrap().layers[0];
        assert_eq!(layer.reset_trigger_id, 2);
        assert_eq!(layer.address, MediaAddress::new(3, 3));
    }

    #[test]
    fn dimmer_values_from_the_web_are_clamped() {
        let (mut media, id) = state(LayerPersonality::TwoLayers);
        apply(
            &mut media,
            &command(
                CommandKind::SetLayerDimmer {
                    output: id,
                    layer: 0,
                    dimmer: 4.0,
                },
                CommandSource::Web,
                0,
            ),
        );
        assert_eq!(media.output(id).unwrap().layers[0].dimmer, 1.0);

        apply(
            &mut media,
            &command(
                CommandKind::SetLayerDimmer {
                    output: id,
                    layer: 0,
                    dimmer: -1.0,
                },
                CommandSource::Web,
                1,
            ),
        );
        assert_eq!(media.output(id).unwrap().layers[0].dimmer, 0.0);
    }

    #[test]
    fn a_frame_never_grows_or_shrinks_the_layer_collection() {
        let (mut media, id) = state(LayerPersonality::TwoLayers);
        let frame = DecodedFrame {
            layers: vec![LayerState::default(); 8],
            master: MasterState::default(),
        };
        apply(
            &mut media,
            &command(
                CommandKind::SetDmxFrame {
                    output: id,
                    frame: Box::new(frame),
                },
                CommandSource::ArtNet,
                0,
            ),
        );
        assert_eq!(media.output(id).unwrap().layers.len(), 2);
    }

    #[test]
    fn two_outputs_keep_separate_state_and_separate_ownership() {
        let first = OutputId::new();
        let second = OutputId::new();
        let mut media = MediaState::with_outputs(vec![
            OutputState::new(first, LayerPersonality::TwoLayers),
            OutputState::new(second, LayerPersonality::TwoLayers),
        ]);

        let frame = DecodedFrame {
            layers: vec![LayerState::default(); 2],
            master: MasterState::default(),
        };
        apply(
            &mut media,
            &command(
                CommandKind::SetDmxFrame {
                    output: first,
                    frame: Box::new(frame),
                },
                CommandSource::ArtNet,
                1_000,
            ),
        );

        let web = command(
            CommandKind::SelectMedia {
                output: second,
                layer: 0,
                address: MediaAddress::new(5, 5),
            },
            CommandSource::Web,
            1_000,
        );
        assert_eq!(
            apply(&mut media, &web),
            Applied::Changed,
            "a desk on one output owns only it"
        );
        assert_eq!(
            media.output(second).unwrap().layers[0].address,
            MediaAddress::new(5, 5)
        );
        assert_eq!(
            media.output(first).unwrap().layers[0].address,
            MediaAddress::BLANK
        );
    }

    #[test]
    fn every_source_has_a_stable_log_name() {
        for source in [
            CommandSource::ArtNet,
            CommandSource::Sacn,
            CommandSource::Web,
            CommandSource::Internal,
            CommandSource::Recovery,
        ] {
            assert!(!describe_source(source).is_empty());
        }
    }
}
