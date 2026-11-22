use super::*;
use media_domain::playlist::{AuxiliaryLayer, PlaylistEntry};
use media_domain::{AssetId, Command, CommandSource, LayerPersonality, MediaState, apply};

fn at(millis: u64) -> Timestamp {
    Timestamp::from_millis(millis)
}

fn asset(
    catalog: &mut CatalogSnapshot,
    file: u8,
    kind: ItemKind,
    duration: Option<u64>,
) -> AssetId {
    let id = AssetId::new();
    catalog
        .insert(
            1,
            CatalogItem {
                id,
                file,
                name: format!("Clip {file}"),
                kind,
                width: 4,
                height: 4,
                frames: duration.map(|_| 25),
                duration_millis: duration,
                intrinsic_bpm: None,
                note: None,
                enabled: true,
            },
        )
        .unwrap();
    id
}

fn entry(asset_id: AssetId, hold_millis: Option<u64>, fade_millis: u64) -> PlaylistEntry {
    PlaylistEntry {
        id: Playlist::default().id,
        asset_id,
        plays: 1,
        hold_millis,
        fade_millis,
    }
}

fn output() -> OutputState {
    OutputState::new(OutputId::new(), LayerPersonality::EightLayers)
}

fn submit(output: &mut OutputState, update: &RunnerUpdate, now: u64) {
    let mut state = MediaState::with_outputs(vec![output.clone()]);
    for kind in &update.commands {
        assert!(
            apply(
                &mut state,
                &Command::new(kind.clone(), CommandSource::Internal, at(now))
            )
            .is_accepted()
        );
    }
    *output = state.outputs.remove(0);
}

fn ready(output: &mut OutputState, layer: usize) {
    output.layers[layer].source_status = SourceStatus::Ready;
}

fn begin(
    playlist: Playlist,
    output: &mut OutputState,
    catalog: &CatalogSnapshot,
    looping: bool,
) -> PlaylistRunner {
    let (mut runner, update) =
        PlaylistRunner::activate(&playlist, output, catalog, at(0), looping).unwrap();
    submit(output, &update, 0);
    assert_eq!(output.layers[1].play_mode, PlayMode::Stop);
    assert_eq!(output.layers[1].dimmer, 0.0);
    assert_eq!(output.layers[1].source_status, SourceStatus::Loading);
    ready(output, 1);
    let update = runner.tick(output, catalog, at(1));
    submit(output, &update, 1);
    ready(output, 1);
    runner
}

#[test]
fn source_over_fade_keeps_opaque_sources_full_at_every_step_in_both_directions() {
    // Real layer shader uses straight-alpha source-over. The lower source stays opaque, making
    // the foreground's effective alpha precisely one, including the midpoint.
    for (outgoing, incoming) in [(1, 2), (2, 1)] {
        for step in 0..=100 {
            let progress = step as f32 / 100.0;
            let (a, b) = crossfade_levels(outgoing, incoming, progress);
            let (lower, upper) = if outgoing < incoming { (a, b) } else { (b, a) };
            let alpha = upper + lower * (1.0 - upper);
            assert!((alpha - 1.0).abs() < 1e-6);
            let red = if outgoing < incoming {
                a * (1.0 - b)
            } else {
                a
            };
            let blue = if outgoing < incoming {
                b
            } else {
                b * (1.0 - a)
            };
            assert!((red - (1.0 - progress)).abs() < 1e-6);
            assert!((blue - progress).abs() < 1e-6);
        }
    }
}

#[test]
fn transparent_sources_and_overlays_retain_source_over_semantics() {
    // Half-transparent source alpha multiplies the dimmer, and a fixed top layer still composes
    // over the pair. The fade must never manufacture opaque alpha for a cut-out source.
    let (outgoing, incoming) = crossfade_levels(1, 2, 0.5);
    let lower_alpha = outgoing * 0.5;
    let upper_alpha = incoming * 0.5;
    let pair_alpha = upper_alpha + lower_alpha * (1.0 - upper_alpha);
    assert_eq!(pair_alpha, 0.625);
    let fixed_overlay_alpha = 0.25;
    let result = fixed_overlay_alpha + pair_alpha * (1.0 - fixed_overlay_alpha);
    assert_eq!(result, 0.71875);
}

#[test]
fn stills_hold_for_each_play_then_complete_without_looping() {
    let mut catalog = CatalogSnapshot::default();
    let id = asset(&mut catalog, 1, ItemKind::Image, None);
    let mut clip = entry(id, Some(100), 0);
    clip.plays = 2;
    let playlist = Playlist {
        entries: vec![clip],
        ..Default::default()
    };
    let mut output = output();
    let mut runner = begin(playlist, &mut output, &catalog, false);
    let reset = output.layers[1].reset_trigger_id;
    let update = runner.tick(&output, &catalog, at(100));
    assert_eq!(update.status.play, 1);
    let update = runner.tick(&output, &catalog, at(101));
    assert_eq!(update.status.play, 2);
    submit(&mut output, &update, 101);
    assert!(output.layers[1].reset_trigger_id > reset);
    ready(&mut output, 1);
    let update = runner.tick(&output, &catalog, at(201));
    assert_eq!(update.status.phase, RunnerPhase::Completed);
    submit(&mut output, &update, 201);
    assert!(output.layers[1].address.is_blank());
    assert!(runner.tick(&output, &catalog, at(300)).commands.is_empty());
}

#[test]
fn videos_repeat_only_on_transport_completion_and_reset_the_same_selection() {
    let mut catalog = CatalogSnapshot::default();
    let id = asset(&mut catalog, 1, ItemKind::Video, Some(100));
    let mut clip = entry(id, None, 0);
    clip.plays = 2;
    let playlist = Playlist {
        entries: vec![clip],
        ..Default::default()
    };
    let mut output = output();
    let mut runner = begin(playlist, &mut output, &catalog, false);
    let update = runner.tick(&output, &catalog, at(500));
    assert_eq!(
        update.status.play, 1,
        "a catalog estimate cannot declare a video finished"
    );
    output.layers[1].source_status = SourceStatus::Completed;
    let update = runner.tick(&output, &catalog, at(501));
    assert_eq!(update.status.play, 2);
    assert!(
        update
            .commands
            .iter()
            .any(|command| matches!(command, CommandKind::ResetLayer { layer: 1, .. }))
    );
    submit(&mut output, &update, 501);
    assert_eq!(output.layers[1].source_status, SourceStatus::Loading);
    ready(&mut output, 1);
    runner.tick(&output, &catalog, at(502));
    output.layers[1].source_status = SourceStatus::Completed;
    assert_eq!(
        runner.tick(&output, &catalog, at(601)).status.phase,
        RunnerPhase::Completed
    );
}

#[test]
fn preloaded_clip_is_stopped_until_fade_and_auxiliary_content_is_not_restarted() {
    let mut catalog = CatalogSnapshot::default();
    let first = asset(&mut catalog, 1, ItemKind::Image, None);
    let second = asset(&mut catalog, 2, ItemKind::Image, None);
    let logo = asset(&mut catalog, 3, ItemKind::Video, Some(500));
    let playlist = Playlist {
        entries: vec![entry(first, Some(100), 20), entry(second, Some(100), 20)],
        auxiliary_layers: vec![AuxiliaryLayer {
            layer: 8,
            mode: AuxiliaryMode::Fixed,
            asset_id: Some(logo),
            controls: LayerPreset::default(),
        }],
        ..Default::default()
    };
    let mut output = output();
    let mut runner = begin(playlist, &mut output, &catalog, true);
    let logo_reset = output.layers[7].reset_trigger_id;
    let update = runner.tick(&output, &catalog, at(2));
    submit(&mut output, &update, 2);
    assert_eq!(output.layers[2].play_mode, PlayMode::Stop);
    assert_eq!(output.layers[2].dimmer, 0.0);
    ready(&mut output, 2);
    let update = runner.tick(&output, &catalog, at(81));
    assert_eq!(update.status.phase, RunnerPhase::Fading);
    submit(&mut output, &update, 81);
    ready(&mut output, 2);
    assert_eq!(output.layers[1].dimmer, 1.0);
    assert_eq!(output.layers[2].dimmer, 0.0);
    let update = runner.tick(&output, &catalog, at(91));
    submit(&mut output, &update, 91);
    assert_eq!(output.layers[1].dimmer, 1.0);
    assert_eq!(output.layers[2].dimmer, 0.5);
    let update = runner.tick(&output, &catalog, at(101));
    submit(&mut output, &update, 101);
    assert!(output.layers[1].address.is_blank());
    assert_eq!(output.layers[2].dimmer, 1.0);
    assert_eq!(output.layers[7].reset_trigger_id, logo_reset);
    assert_eq!(output.layers[7].address, MediaAddress::new(1, 3));

    let update = runner.tick(&output, &catalog, at(102));
    submit(&mut output, &update, 102);
    ready(&mut output, 1);
    let update = runner.tick(&output, &catalog, at(161));
    submit(&mut output, &update, 161);
    ready(&mut output, 1);
    let update = runner.tick(&output, &catalog, at(171));
    submit(&mut output, &update, 171);
    assert_eq!(output.layers[2].dimmer, 0.5);
    assert_eq!(output.layers[1].dimmer, 1.0);
    assert_eq!(output.layers[7].reset_trigger_id, logo_reset);
}

#[test]
fn cold_loading_does_not_consume_a_still_display_duration() {
    let mut catalog = CatalogSnapshot::default();
    let id = asset(&mut catalog, 1, ItemKind::Image, None);
    let playlist = Playlist {
        entries: vec![entry(id, Some(100), 0)],
        ..Default::default()
    };
    let mut output = output();
    let (mut runner, update) =
        PlaylistRunner::activate(&playlist, &output, &catalog, at(0), false).unwrap();
    submit(&mut output, &update, 0);
    assert_eq!(
        runner.tick(&output, &catalog, at(999)).status.phase,
        RunnerPhase::Loading
    );
    ready(&mut output, 1);
    let update = runner.tick(&output, &catalog, at(1000));
    submit(&mut output, &update, 1000);
    ready(&mut output, 1);
    assert_eq!(
        runner.tick(&output, &catalog, at(1099)).status.phase,
        RunnerPhase::Playing
    );
    assert_eq!(
        runner.tick(&output, &catalog, at(1100)).status.phase,
        RunnerPhase::Completed
    );
}

#[test]
fn all_failed_entries_exhaust_once_without_a_retry_loop() {
    let mut catalog = CatalogSnapshot::default();
    let id = asset(&mut catalog, 1, ItemKind::Video, Some(100));
    let playlist = Playlist {
        entries: vec![entry(id, None, 0), entry(id, None, 0)],
        ..Default::default()
    };
    let mut output = output();
    let (mut runner, update) =
        PlaylistRunner::activate(&playlist, &output, &catalog, at(0), true).unwrap();
    submit(&mut output, &update, 0);
    let update = runner.tick(&output, &catalog, at(10_000));
    submit(&mut output, &update, 10_000);
    assert_eq!(update.status.entry_index, Some(1));
    let update = runner.tick(&output, &catalog, at(20_000));
    assert_eq!(update.status.phase, RunnerPhase::Failed);
    assert!(update.status.error.is_some());
    submit(&mut output, &update, 20_000);
    assert!(
        runner
            .tick(&output, &catalog, at(30_000))
            .commands
            .is_empty()
    );
}

#[test]
fn moved_asset_is_resolved_by_identity_and_disabled_entries_are_skipped() {
    let mut catalog = CatalogSnapshot::default();
    let first = asset(&mut catalog, 1, ItemKind::Image, None);
    let second = asset(&mut catalog, 2, ItemKind::Image, None);
    let playlist = Playlist {
        entries: vec![entry(first, Some(100), 0), entry(second, Some(100), 0)],
        ..Default::default()
    };
    catalog.set_item_enabled(first, false).unwrap();
    let mut output = output();
    let (mut runner, update) =
        PlaylistRunner::activate(&playlist, &output, &catalog, at(0), false).unwrap();
    submit(&mut output, &update, 0);
    assert_eq!(update.status.entry_index, Some(1));
    catalog.swap_items(first, second).unwrap();
    let update = runner.tick(&output, &catalog, at(1));
    submit(&mut output, &update, 1);
    assert_eq!(output.layers[1].address, MediaAddress::new(1, 1));
    ready(&mut output, 1);
    assert_eq!(
        runner.tick(&output, &catalog, at(2)).status.phase,
        RunnerPhase::Playing
    );
}

#[test]
fn two_layer_output_requires_explicit_valid_pair_and_stills_require_duration() {
    let mut catalog = CatalogSnapshot::default();
    let id = asset(&mut catalog, 1, ItemKind::Image, None);
    let output = OutputState::new(OutputId::new(), LayerPersonality::TwoLayers);
    let mut playlist = Playlist {
        entries: vec![entry(id, Some(100), 0)],
        ..Default::default()
    };
    assert!(PlaylistRunner::activate(&playlist, &output, &catalog, at(0), false).is_err());
    playlist.layers = [1, 2];
    assert!(PlaylistRunner::activate(&playlist, &output, &catalog, at(0), false).is_ok());
    playlist.entries[0].hold_millis = None;
    assert!(PlaylistRunner::activate(&playlist, &output, &catalog, at(0), false).is_err());
    playlist.entries[0].hold_millis = Some(100);
    playlist.entries[0].fade_millis = 101;
    assert!(PlaylistRunner::activate(&playlist, &output, &catalog, at(0), false).is_err());
}

#[test]
fn stop_clears_only_owned_layers_and_excludes_unchanged_auxiliary_layers() {
    let mut catalog = CatalogSnapshot::default();
    let id = asset(&mut catalog, 1, ItemKind::Image, None);
    let playlist = Playlist {
        entries: vec![entry(id, Some(100), 0)],
        auxiliary_layers: vec![
            AuxiliaryLayer {
                layer: 1,
                mode: AuxiliaryMode::Unchanged,
                asset_id: None,
                controls: LayerPreset::default(),
            },
            AuxiliaryLayer {
                layer: 8,
                mode: AuxiliaryMode::Off,
                asset_id: None,
                controls: LayerPreset::default(),
            },
        ],
        ..Default::default()
    };
    let output = output();
    let (runner, _) = PlaylistRunner::activate(&playlist, &output, &catalog, at(0), false).unwrap();
    assert_eq!(runner.owned_layers(), vec![1, 2, 7]);
    assert_eq!(runner.stop().len(), 3);
}

#[test]
fn zero_fade_video_waits_for_natural_completion_even_after_the_catalog_estimate() {
    let mut catalog = CatalogSnapshot::default();
    let first = asset(&mut catalog, 1, ItemKind::Video, Some(100));
    let second = asset(&mut catalog, 2, ItemKind::Video, Some(100));
    let playlist = Playlist {
        entries: vec![entry(first, None, 0), entry(second, None, 0)],
        ..Default::default()
    };
    let mut output = output();
    let mut runner = begin(playlist, &mut output, &catalog, false);
    let update = runner.tick(&output, &catalog, at(2));
    submit(&mut output, &update, 2);
    ready(&mut output, 2);
    let update = runner.tick(&output, &catalog, at(500));
    assert_eq!(update.status.entry_index, Some(0));
    assert_eq!(output.layers[1].source_status, SourceStatus::Ready);
    output.layers[1].source_status = SourceStatus::Completed;
    let update = runner.tick(&output, &catalog, at(501));
    assert_eq!(update.status.entry_index, Some(1));
}
