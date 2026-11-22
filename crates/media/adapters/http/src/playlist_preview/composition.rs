//! Computes a preview composition through the same typed reducer commands as playback.
use crate::ApiError;
use media_domain::catalog::{CatalogSnapshot, ItemKind};
use media_domain::playlist::{AuxiliaryMode, LayerPreset, Playlist};
use media_domain::{
    Command, CommandKind, CommandSource, MasterControls, MediaAddress, MediaState, OutputState,
    PlayMode, SourceStatus, Timestamp,
};

pub(super) fn normalized_base(output: &OutputState, now: Timestamp) -> OutputState {
    let commands = takeover(output.id)
        .into_iter()
        .map(|kind| Command::new(kind, CommandSource::Web, now))
        .collect::<Vec<_>>();
    let state = MediaState::with_outputs(vec![output.clone()]);
    media_domain::state::reduce_commands(&state, &commands)
        .expect("takeover is always permitted")
        .unwrap_or(state)
        .outputs
        .remove(0)
}

pub(super) fn takeover(output: media_domain::OutputId) -> Vec<CommandKind> {
    // A new temporary owner needs a new generation even when manual takeover was already on.
    vec![
        CommandKind::TakeOverPlayback {
            output,
            take_over: false,
        },
        CommandKind::TakeOverPlayback {
            output,
            take_over: true,
        },
    ]
}

pub(super) fn commands(
    current: &OutputState,
    base: &OutputState,
    playlist: &Playlist,
    index: usize,
    catalog: &CatalogSnapshot,
    restart_step: bool,
    restart_auxiliary: bool,
) -> Result<Vec<CommandKind>, ApiError> {
    let entry = &playlist.entries[index];
    let (address, kind) = resolve(catalog, entry.asset_id)?;
    if kind == ItemKind::Image && !entry.hold_millis.is_some_and(|value| value > 0) {
        return Err(invalid("A still image needs a positive display duration"));
    }
    let selected = playlist.layers[index % 2] - 1;
    let mut desired: Vec<_> = base
        .layers
        .iter()
        .map(|layer| (layer.address, LayerPreset::from(layer), false))
        .collect();
    for layer in playlist.layers {
        let target = desired
            .get_mut(layer - 1)
            .ok_or_else(|| invalid("The playback layers do not exist on this output"))?;
        *target = (MediaAddress::BLANK, cleared(), false);
    }
    let mut main = entry.transform.layer_preset();
    main.play_mode = PlayMode::Loop;
    main.dimmer = 1.0;
    main.volume = 1.0;
    let failed = |layer: usize| current.layers[layer].source_status.is_failed();
    desired[selected] = (address, main, restart_step || failed(selected));
    for auxiliary in &playlist.auxiliary_layers {
        let target = desired
            .get_mut(auxiliary.layer - 1)
            .ok_or_else(|| invalid("An auxiliary layer does not exist on this output"))?;
        match auxiliary.mode {
            AuxiliaryMode::Unchanged => {}
            AuxiliaryMode::Off => *target = (MediaAddress::BLANK, cleared(), false),
            AuxiliaryMode::Fixed => {
                let id = auxiliary
                    .asset_id
                    .ok_or_else(|| invalid("Fixed content needs an asset"))?;
                *target = (
                    resolve(catalog, id)?.0,
                    auxiliary.controls.clone(),
                    restart_auxiliary || failed(auxiliary.layer - 1),
                );
            }
        }
    }
    let mut result = Vec::new();
    for (layer, (address, preset, restart)) in desired.into_iter().enumerate() {
        let was = &current.layers[layer];
        if was.address != address || LayerPreset::from(was) != preset {
            result.push(CommandKind::SetLayerControls {
                output: current.id,
                layer,
                controls: Box::new(preset.controls(address)),
            });
        }
        if restart || was.address != address {
            reset(current.id, layer, &mut result);
        }
    }
    let mut master = base.master;
    master.dimmer = playlist.master_dimmer;
    master.volume = playlist.master_volume;
    if master != current.master {
        result.push(CommandKind::SetMasterControls {
            output: current.id,
            controls: Box::new(master_controls(master)),
        });
    }
    Ok(result)
}

pub(super) fn restore(current: &OutputState, before: &OutputState) -> Vec<CommandKind> {
    let mut result = Vec::new();
    for (layer, original) in before.layers.iter().enumerate() {
        let Some(was) = current.layers.get(layer) else {
            continue;
        };
        if original.address != was.address || LayerPreset::from(original) != LayerPreset::from(was)
        {
            result.push(CommandKind::SetLayerControls {
                output: current.id,
                layer,
                controls: Box::new(LayerPreset::from(original).controls(original.address)),
            });
        }
        if original.address != was.address {
            reset(current.id, layer, &mut result);
        }
    }
    if current.master != before.master {
        result.push(CommandKind::SetMasterControls {
            output: current.id,
            controls: Box::new(master_controls(before.master)),
        });
    }
    if !before.ownership.web_takeover {
        result.push(CommandKind::TakeOverPlayback {
            output: current.id,
            take_over: false,
        });
    }
    result
}

fn cleared() -> LayerPreset {
    LayerPreset {
        play_mode: PlayMode::Stop,
        dimmer: 0.0,
        volume: 0.0,
        ..Default::default()
    }
}
fn reset(output: media_domain::OutputId, layer: usize, commands: &mut Vec<CommandKind>) {
    commands.push(CommandKind::ResetLayer { output, layer });
    commands.push(CommandKind::ReportSourceStatus {
        output,
        layer,
        status: SourceStatus::Loading,
    });
}
fn resolve(
    catalog: &CatalogSnapshot,
    id: media_domain::AssetId,
) -> Result<(MediaAddress, ItemKind), ApiError> {
    let (_, item) = catalog
        .item(id)
        .ok_or_else(|| invalid("The preview media is missing"))?;
    if !item.enabled {
        return Err(invalid("The preview media is disabled"));
    }
    Ok((
        catalog
            .address_of(id)
            .ok_or_else(|| invalid("The preview media is parked"))?,
        item.kind,
    ))
}
fn invalid(message: &str) -> ApiError {
    ApiError::bad_request("playlist-preview-invalid", message)
}
fn master_controls(master: media_domain::MasterState) -> MasterControls {
    MasterControls {
        dimmer: Some(master.dimmer),
        volume: Some(master.volume),
        tint: Some(master.tint),
        flip_mirror: Some(master.flip_mirror),
        mask: Some(master.mask),
        mask_position_x: Some(master.mask_position_x),
        mask_position_y: Some(master.mask_position_y),
        scale_x: Some(master.scale_x),
        scale_y: Some(master.scale_y),
        scaling_mode: Some(master.scaling_mode),
        position_x: Some(master.position_x),
        position_y: Some(master.position_y),
        rotation: Some(master.rotation),
        shaper: Some(master.shaper),
        opacity_cycle: Some(master.opacity_cycle),
    }
}
