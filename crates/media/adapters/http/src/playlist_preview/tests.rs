use super::*;
use crate::routes::bench::{Bench, bench, post, send};
use crate::wire::PlaylistView;
use media_domain::catalog::{CatalogItem, ItemKind};
use media_domain::playlist::{
    AuxiliaryLayer, AuxiliaryMode, LayerPreset, PlaylistEntry, PlaylistTransform,
};
use media_domain::{AssetId, CommandKind, LayerPersonality, MediaAddress};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn fixture() -> (Bench, PreviewPlaylistStep) {
    let bench = bench();
    let mut configuration = (**bench.configuration.load()).clone();
    configuration.outputs[0].personality = LayerPersonality::EightLayers;
    bench.configuration.store(Arc::new(configuration));
    let mut output = OutputState::new(bench.output, LayerPersonality::EightLayers);
    output.master.volume = 0.4;
    output.master.rotation = 23.0;
    output.master.mask_position_x = 0.25;
    output.layers[0].scale_x = 1.7;
    output.layers[0].rotation = 47.0;
    output.layers[0].mask.address = MediaAddress::new(1, 3);
    output.layers[0].mask.opacity = 0.5;
    output.layers[0].effects[0].effect_type = Some("preserved-future-effect".into());
    output.layers[0].effects[0].enabled = true;
    output.layers[0].effects[0].mix = 0.45;
    output.layers[0].effects[0].parameters = vec![0.2, 0.7];
    output.layers[0].effect_banks[0].select = 9;
    bench
        .state
        .store(Arc::new(MediaState::with_outputs(vec![output])));
    let mut catalog = CatalogSnapshot::default();
    let mut assets = Vec::new();
    for file in 1..=3 {
        let id = AssetId::new();
        assets.push(id);
        catalog
            .insert(
                1,
                CatalogItem {
                    id,
                    file,
                    name: format!("Clip {file}"),
                    kind: ItemKind::Video,
                    width: 640,
                    height: 360,
                    frames: Some(250),
                    duration_millis: Some(10000),
                    intrinsic_bpm: None,
                    note: None,
                    enabled: true,
                },
            )
            .unwrap();
    }
    bench.api.catalog.store(Arc::new(catalog));
    let playlist = Playlist {
        layers: [2, 4],
        master_dimmer: 0.8,
        master_volume: 0.3,
        entries: assets[..2]
            .iter()
            .map(|id| PlaylistEntry {
                id: Uuid::new_v4(),
                asset_id: *id,
                plays: 1,
                hold_millis: None,
                fade_millis: 500,
                transform: PlaylistTransform {
                    scale_x: 1.2,
                    scale_y: 0.8,
                    position_x: 0.4,
                    position_y: -0.3,
                    rotation: 36.0,
                },
            })
            .collect(),
        auxiliary_layers: vec![
            AuxiliaryLayer {
                layer: 1,
                mode: AuxiliaryMode::Fixed,
                asset_id: Some(assets[2]),
                controls: LayerPreset::default(),
            },
            AuxiliaryLayer {
                layer: 3,
                mode: AuxiliaryMode::Off,
                asset_id: None,
                controls: LayerPreset::default(),
            },
        ],
        ..Default::default()
    };
    let entry_id = playlist.entries[1].id.to_string();
    let playlist: PlaylistView =
        serde_json::from_value(serde_json::to_value(playlist).unwrap()).unwrap();
    let request = PreviewPlaylistStep {
        output_id: bench.output.to_string(),
        preview_id: Uuid::new_v4().to_string(),
        sequence: 1,
        expected_ownership_generation: 0,
        playlist,
        entry_id,
    };
    (bench, request)
}
fn show(
    bench: &Bench,
    manager: &mut PreviewSessions,
    body: &PreviewPlaylistStep,
) -> Result<(PlaylistPreviewState, OutputState), ApiError> {
    manager.show(
        &bench.state,
        &bench.configuration.load(),
        &bench.api.catalog.load(),
        Timestamp::from_millis(10000),
        body,
    )
}
fn stop(body: &PreviewPlaylistStep) -> StopPlaylistPreview {
    StopPlaylistPreview {
        output_id: body.output_id.clone(),
        preview_id: body.preview_id.clone(),
        sequence: body.sequence + 1,
    }
}

#[test]
fn preview_uses_entry_frame_parity_and_complete_playlist_composition() {
    let (bench, body) = fixture();
    let mut manager = PreviewSessions::default();
    let (view, output) = show(&bench, &mut manager, &body).unwrap();
    assert_eq!(view.ownership_generation, 1);
    assert!(output.ownership.web_takeover);
    let selected = &output.layers[3];
    assert_eq!(selected.address, MediaAddress::new(1, 2));
    assert_eq!(
        (
            selected.scale_x,
            selected.scale_y,
            selected.position_x,
            selected.position_y,
            selected.rotation
        ),
        (1.2, 0.8, 0.4, -0.3, 36.0)
    );
    assert_eq!(output.layers[1].address, MediaAddress::BLANK);
    assert_eq!(output.layers[1].dimmer, 0.0);
    assert_eq!(output.layers[2].address, MediaAddress::BLANK);
    assert_eq!(output.layers[0].address, MediaAddress::new(1, 3));
    assert_eq!(output.master.dimmer, 0.8);
    assert_eq!(output.master.volume, 0.3);
    assert!(bench.stored.lock().unwrap().is_empty());
}

#[test]
fn frame_edits_and_step_switches_do_not_restart_fixed_video() {
    let (bench, mut body) = fixture();
    let mut manager = PreviewSessions::default();
    let (view, before) = show(&bench, &mut manager, &body).unwrap();
    body.expected_ownership_generation = view.ownership_generation;
    body.sequence += 1;
    body.playlist.entries[1].transform.scale_x = 2.5;
    let (_, edited) = show(&bench, &mut manager, &body).unwrap();
    assert_eq!(
        edited.layers[0].reset_trigger_id,
        before.layers[0].reset_trigger_id
    );
    assert_eq!(
        edited.layers[3].reset_trigger_id,
        before.layers[3].reset_trigger_id
    );
    assert_eq!(edited.layers[3].scale_x, 2.5);
    body.sequence += 1;
    body.entry_id = body.playlist.entries[0].id.clone();
    let (_, switched) = show(&bench, &mut manager, &body).unwrap();
    assert_eq!(
        switched.layers[0].reset_trigger_id,
        before.layers[0].reset_trigger_id
    );
    assert_eq!(switched.layers[3].address, MediaAddress::BLANK);
    assert_eq!(switched.layers[1].address, MediaAddress::new(1, 1));
}

#[test]
fn explicit_retry_resets_failed_step_and_fixed_source_without_changing_their_addresses() {
    let (bench, mut body) = fixture();
    let mut manager = PreviewSessions::default();
    let (view, before) = show(&bench, &mut manager, &body).unwrap();
    let mut failed = (**bench.state.load()).clone();
    for layer in [0, 3] {
        failed.outputs[0].layers[layer].source_status = media_domain::SourceStatus::Failed {
            failure: media_domain::SourceFailure::DecodeFailed,
        };
    }
    bench.state.store(Arc::new(failed));
    body.sequence += 1;
    body.expected_ownership_generation = view.ownership_generation;
    let (_, retried) = show(&bench, &mut manager, &body).unwrap();
    for layer in [0, 3] {
        assert_eq!(retried.layers[layer].address, before.layers[layer].address);
        assert!(retried.layers[layer].reset_trigger_id > before.layers[layer].reset_trigger_id);
        assert_eq!(
            retried.layers[layer].source_status,
            media_domain::SourceStatus::Loading
        );
    }
}

#[test]
fn stop_restores_full_editable_presets_and_prior_manual_takeover() {
    for manual in [false, true] {
        let (bench, body) = fixture();
        let mut before = (**bench.state.load()).clone();
        before.outputs[0].ownership.web_takeover = manual;
        bench.state.store(Arc::new(before.clone()));
        let mut manager = PreviewSessions::default();
        show(&bench, &mut manager, &body).unwrap();
        let restored = manager
            .stop(&bench.state, Timestamp::from_millis(10001), &stop(&body))
            .unwrap();
        assert_eq!(restored.master, before.outputs[0].master);
        for (actual, expected) in restored.layers.iter().zip(&before.outputs[0].layers) {
            assert_eq!(LayerPreset::from(actual), LayerPreset::from(expected));
            assert_eq!(actual.address, expected.address);
        }
        assert_eq!(restored.ownership.web_takeover, manual);
    }
}

#[test]
fn newer_manual_owner_is_not_overwritten_by_stop_or_stale_show() {
    let (bench, mut body) = fixture();
    let mut manager = PreviewSessions::default();
    let (view, _) = show(&bench, &mut manager, &body).unwrap();
    crate::control::submit_commands(
        &bench.state,
        &[
            CommandKind::TakeOverPlayback {
                output: bench.output,
                take_over: false,
            },
            CommandKind::TakeOverPlayback {
                output: bench.output,
                take_over: true,
            },
            CommandKind::SetLayerControls {
                output: bench.output,
                layer: 0,
                controls: Box::new(media_domain::LayerControls {
                    rotation: Some(121.0),
                    ..Default::default()
                }),
            },
        ],
        CommandSource::Web,
        Timestamp::from_millis(10002),
    )
    .unwrap();
    let newer = bench.state.load_full();
    body.sequence = 2;
    body.expected_ownership_generation = view.ownership_generation;
    assert!(show(&bench, &mut manager, &body).is_err());
    manager
        .stop(&bench.state, Timestamp::from_millis(10003), &stop(&body))
        .unwrap();
    assert_eq!(**bench.state.load(), *newer);
}

#[test]
fn delayed_start_after_stop_and_stale_sequences_cannot_revive_or_replace_preview() {
    let (bench, body) = fixture();
    let mut manager = PreviewSessions::default();
    manager
        .stop(&bench.state, Timestamp::from_millis(1), &stop(&body))
        .unwrap();
    assert!(show(&bench, &mut manager, &body).is_err());
    assert!(!bench.state.load().outputs[0].ownership.web_takeover);
    let mut fresh = body.clone();
    fresh.preview_id = Uuid::new_v4().to_string();
    let (_, output) = show(&bench, &mut manager, &fresh).unwrap();
    assert!(show(&bench, &mut manager, &fresh).is_err());
    let mut another_stop = stop(&body);
    another_stop.sequence = 500;
    manager
        .stop(&bench.state, Timestamp::from_millis(2), &another_stop)
        .unwrap();
    assert_eq!(bench.state.load().outputs[0], output);
    assert_eq!(manager.active.len(), 1);
}

#[test]
fn invalid_drafts_and_missing_assets_have_no_partial_takeover() {
    let (bench, body) = fixture();
    for change in 0..3 {
        let mut bad = body.clone();
        match change {
            0 => bad.playlist.entries[1].transform.scale_x = -1.0,
            1 => bad.playlist.layers = [2, 2],
            _ => bad.playlist.entries[1].asset_id = Uuid::new_v4().to_string(),
        }
        let mut manager = PreviewSessions::default();
        let before = bench.state.load_full();
        assert!(show(&bench, &mut manager, &bad).is_err());
        assert_eq!(**bench.state.load(), *before);
        assert!(manager.active.is_empty());
    }
}

#[test]
fn closed_preview_sequence_memory_is_bounded() {
    let (bench, body) = fixture();
    let mut manager = PreviewSessions::default();
    for _ in 0..CLOSED_LIMIT + 10 {
        let mut closed = stop(&body);
        closed.preview_id = Uuid::new_v4().to_string();
        manager
            .stop(&bench.state, Timestamp::ZERO, &closed)
            .unwrap();
    }
    assert_eq!(manager.closed.len(), CLOSED_LIMIT);
}

#[test]
fn accepted_snapshot_is_from_the_successful_cas_candidate() {
    let (bench, _) = fixture();
    let injected = AtomicBool::new(false);
    let (before, _) = crate::control::submit_output_transaction(
        &bench.state,
        bench.output,
        0,
        CommandSource::Web,
        Timestamp::from_millis(1),
        |current| {
            if !injected.swap(true, Ordering::SeqCst) {
                let mut concurrent = (**bench.state.load()).clone();
                concurrent.outputs[0].layers[0].rotation = 87.0;
                bench.state.store(Arc::new(concurrent));
            }
            assert!(current.layers[0].rotation == 47.0 || current.layers[0].rotation == 87.0);
            Ok::<_, ApiError>(composition::takeover(bench.output))
        },
        refusal,
    )
    .unwrap();
    assert_eq!(before.layers[0].rotation, 87.0);
}

#[tokio::test]
async fn http_preview_is_typed_nonpersistent_and_no_store() {
    let (bench, body) = fixture();
    let mut json = serde_json::to_value(&body).unwrap();
    json["futureField"] = true.into();
    let response = tower::ServiceExt::oneshot(
        bench.router.clone(),
        post("/api/v2/automation/preview-step".into(), &json.to_string()),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let (_, stopped) = send(
        &bench.router,
        post(
            "/api/v2/automation/preview-stop".into(),
            &serde_json::to_string(&stop(&body)).unwrap(),
        ),
    )
    .await;
    assert!(stopped["active"].is_null());
    assert_eq!(stopped["output"]["playbackTakeover"], false);
    assert!(bench.stored.lock().unwrap().is_empty());
}
