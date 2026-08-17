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
                // address must not reset a layer that has already failed or completed. Effect
                // identity and typed parameters are configured on the Media Server; DMX carries
                // only each configured slot's mix byte, so a frame must not clear that contract.
                let mut effects = existing.effects.clone();
                for (configured, wire) in effects.iter_mut().zip(&incoming.effects) {
                    configured.mix = wire.mix;
                }
                let incoming = LayerState {
                    source_status: existing.source_status,
                    reset_trigger_id: existing.reset_trigger_id,
                    effects,
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
        CommandKind::SetLayerControls {
            layer, controls, ..
        } => {
            let Some(target) = output.layers.get_mut(*layer) else {
                return Applied::RejectedUnknownLayer;
            };
            let mut changed = false;
            macro_rules! assign {
                ($field:ident) => {
                    if let Some(value) = controls.$field {
                        changed |= replace(&mut target.$field, value);
                    }
                };
            }
            assign!(address);
            assign!(play_mode);
            assign!(scale_x);
            assign!(scale_y);
            assign!(scaling_mode);
            assign!(position_x);
            assign!(position_y);
            assign!(rotation);
            assign!(dimmer);
            assign!(volume);
            assign!(tint);
            assign!(grayscale);
            assign!(speed_multiplier);
            assign!(playback_bpm);
            if let Some(value) = controls.blur {
                changed |= replace(&mut target.blur, value.clamp(0.0, 1.0));
            }
            if let Some(value) = controls.effects.clone() {
                changed |= replace(&mut target.effects, value);
            }
            if let Some(value) = controls.mask_address {
                changed |= replace(&mut target.mask.address, value);
            }
            if let Some(value) = controls.mask_scale_x {
                changed |= replace(&mut target.mask.scale_x, value);
            }
            if let Some(value) = controls.mask_scale_y {
                changed |= replace(&mut target.mask.scale_y, value);
            }
            if let Some(value) = controls.mask_position_x {
                changed |= replace(&mut target.mask.position_x, value.clamp(-2.0, 2.0));
            }
            if let Some(value) = controls.mask_position_y {
                changed |= replace(&mut target.mask.position_y, value.clamp(-2.0, 2.0));
            }
            if let Some(value) = controls.mask_invert {
                changed |= replace(&mut target.mask.invert, value);
            }
            if let Some(value) = controls.mask_opacity {
                changed |= replace(&mut target.mask.opacity, value);
            }
            changed_or_not(changed)
        }
        CommandKind::ConfigureLayerEffects { layer, effects, .. } => {
            let Some(target) = output.layers.get_mut(*layer) else {
                return Applied::RejectedUnknownLayer;
            };
            changed_or_not(replace(&mut target.effects, effects.as_ref().clone()))
        }
        CommandKind::SetMasterControls { controls, .. } => {
            let mut next: MasterState = output.master;
            if let Some(value) = controls.dimmer {
                next.dimmer = value;
            }
            if let Some(value) = controls.volume {
                next.volume = value;
            }
            if let Some(value) = controls.tint {
                next.tint = value;
            }
            if let Some(value) = controls.flip_mirror {
                next.flip_mirror = value;
            }
            if let Some(value) = controls.mask {
                next.mask = value;
            }
            if let Some(value) = controls.mask_position_x {
                next.mask_position_x = value.clamp(-2.0, 2.0);
            }
            if let Some(value) = controls.mask_position_y {
                next.mask_position_y = value.clamp(-2.0, 2.0);
            }
            if let Some(value) = controls.scale_x {
                next.scale_x = value;
            }
            if let Some(value) = controls.scale_y {
                next.scale_y = value;
            }
            if let Some(value) = controls.scaling_mode {
                next.scaling_mode = value;
            }
            if let Some(value) = controls.position_x {
                next.position_x = value;
            }
            if let Some(value) = controls.position_y {
                next.position_y = value;
            }
            if let Some(value) = controls.rotation {
                next.rotation = value;
            }
            if let Some(value) = controls.shaper {
                next.shaper = value;
            }
            changed_or_not(replace(&mut output.master, next))
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
    use crate::layer::{EffectSlot, SourceFailure, SourceStatus};
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
    fn the_web_ui_selects_media_only_during_explicit_takeover() {
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

        assert_eq!(apply(&mut media, &select(0)), Applied::RejectedNotOwner);
        assert_eq!(
            apply(
                &mut media,
                &command(
                    CommandKind::TakeOverPlayback {
                        output: id,
                        take_over: true,
                    },
                    CommandSource::Web,
                    0,
                ),
            ),
            Applied::Changed
        );
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
        assert_eq!(apply(&mut media, &dmx), Applied::RejectedNotOwner);
        assert_eq!(
            apply(
                &mut media,
                &command(
                    CommandKind::TakeOverPlayback {
                        output: id,
                        take_over: false,
                    },
                    CommandSource::Web,
                    999,
                ),
            ),
            Applied::Changed
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
    fn a_dmx_frame_controls_effect_mix_without_erasing_the_configured_effect() {
        let (mut media, id) = state(LayerPersonality::TwoLayers);
        let configured = EffectSlot::analog_tv();
        media.output_mut(id).unwrap().layers[0].effects[1] = configured.clone();
        let mut incoming = LayerState::default();
        incoming.effects[1].mix = 0.4;

        assert_eq!(
            apply(
                &mut media,
                &command(
                    CommandKind::SetDmxFrame {
                        output: id,
                        frame: Box::new(DecodedFrame {
                            layers: vec![incoming, LayerState::default()],
                            master: MasterState::default(),
                        }),
                    },
                    CommandSource::ArtNet,
                    1_000,
                ),
            ),
            Applied::Changed
        );

        let effect = &media.output(id).unwrap().layers[0].effects[1];
        assert_eq!(effect.effect_type, configured.effect_type);
        assert_eq!(effect.parameters, configured.parameters);
        assert_eq!(effect.mix, 0.4);
    }

    #[test]
    fn resetting_a_layer_always_changes_the_trigger_without_touching_the_address() {
        let (mut media, id) = state(LayerPersonality::TwoLayers);
        apply(
            &mut media,
            &command(
                CommandKind::TakeOverPlayback {
                    output: id,
                    take_over: true,
                },
                CommandSource::Web,
                0,
            ),
        );
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
                CommandKind::TakeOverPlayback {
                    output: id,
                    take_over: true,
                },
                CommandSource::Web,
                0,
            ),
        );
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
        apply(
            &mut media,
            &command(
                CommandKind::TakeOverPlayback {
                    output: second,
                    take_over: true,
                },
                CommandSource::Web,
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
            "takeover is scoped to the selected output"
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
