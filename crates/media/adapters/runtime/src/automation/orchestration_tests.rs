use super::*;
use media_domain::playlist::{PlaylistEntry, ScheduleEvent, ScheduleTrigger};
use media_domain::{AssetId, CatalogItem, Command, ItemKind, LayerPersonality, MediaState, apply};

fn at(millis: u64) -> Timestamp {
    Timestamp::from_millis(millis)
}

struct Bench {
    engine: Engine,
    state: crate::SharedState,
    catalog: CatalogSnapshot,
    output: OutputId,
    playlist: Playlist,
    settings: AutomationOutput,
}

impl Bench {
    fn new() -> Self {
        let output = OutputId::new();
        let state = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(MediaState::with_outputs(
            vec![OutputState::new(output, LayerPersonality::EightLayers)],
        )));
        let asset = AssetId::new();
        let mut catalog = CatalogSnapshot::default();
        catalog
            .insert(
                1,
                CatalogItem {
                    id: asset,
                    file: 1,
                    name: "Still".into(),
                    kind: ItemKind::Image,
                    width: 4,
                    height: 4,
                    frames: None,
                    duration_millis: None,
                    intrinsic_bpm: None,
                    note: None,
                    enabled: true,
                },
            )
            .unwrap();
        let playlist = Playlist {
            entries: vec![PlaylistEntry {
                id: Playlist::default().id,
                asset_id: asset,
                plays: 1,
                hold_millis: Some(100),
                fade_millis: 0,
            }],
            ..Default::default()
        };
        let settings = AutomationOutput {
            output_id: output,
            enabled: true,
            fallback_playlist_id: Some(playlist.id),
            dmx_silence_seconds: 10,
        };
        Self {
            engine: Engine::new(None),
            state,
            catalog,
            output,
            playlist,
            settings,
        }
    }

    fn configure(&mut self, events: Vec<ScheduleEvent>, playlists: Vec<Playlist>) {
        self.engine.configure(&AutomationConfiguration {
            playlists,
            events,
            outputs: vec![self.settings.clone()],
        });
    }

    fn tick(&mut self, wall: i64, mono: u64, occurrences: &[Occurrence]) -> AutomationStatus {
        self.engine
            .tick(
                &self.state,
                &self.catalog,
                occurrences,
                &HashMap::new(),
                &HashSet::new(),
                wall,
                at(mono),
            )
            .remove(0)
    }

    fn command(&self, kind: CommandKind, source: CommandSource, mono: u64) {
        let mut state = self.state.load_full().as_ref().clone();
        assert!(apply(&mut state, &Command::new(kind, source, at(mono))).is_accepted());
        self.state.store(std::sync::Arc::new(state));
    }

    fn once(&self, start: i64, playlist: &Playlist) -> (ScheduleEvent, Occurrence) {
        let event = ScheduleEvent {
            id: Playlist::default().id,
            name: "One pass".into(),
            enabled: true,
            output_id: self.output,
            playlist_id: playlist.id,
            trigger: ScheduleTrigger::Fixed {
                start: "2026-11-22T10:00".into(),
                end: None,
            },
        };
        let occurrence = Occurrence {
            key: format!("{}:{start}", event.id),
            event_id: event.id,
            output_id: self.output,
            playlist_id: playlist.id,
            start_millis: start,
            end_millis: None,
        };
        (event, occurrence)
    }
}

#[test]
fn fallback_starts_without_dmx_and_resumes_only_after_the_complete_silence_timeout() {
    let mut bench = Bench::new();
    bench.configure(vec![], vec![bench.playlist.clone()]);
    assert_eq!(bench.tick(0, 0, &[]).owner, "fallback");
    bench.command(
        CommandKind::SetLayerDimmer {
            output: bench.output,
            layer: 1,
            dimmer: 0.3,
        },
        CommandSource::ArtNet,
        100,
    );
    let status = bench.tick(100, 100, &[]);
    assert_eq!(status.owner, "dmx");
    assert_eq!(status.resume_in_millis, Some(10_000));
    assert_eq!(
        bench.state.load().output(bench.output).unwrap().layers[1].dimmer,
        0.3,
        "scheduler restoration must not overwrite DMX"
    );
    assert_eq!(bench.tick(10_099, 10_099, &[]).owner, "dmx");
    let status = bench.tick(10_100, 10_100, &[]);
    assert_eq!(status.owner, "fallback");
    assert_eq!(status.entry_index, Some(0));
    assert_eq!(
        status.play, 0,
        "resumption restarts by loading the first clip"
    );
}

#[test]
fn relevant_unchanged_packets_refresh_timeout_and_ignored_manual_packets_also_delay_resume() {
    let mut bench = Bench::new();
    bench.configure(vec![], vec![bench.playlist.clone()]);
    bench.tick(0, 0, &[]);
    for mono in [100, 1000, 2000] {
        bench.command(
            CommandKind::SetLayerDimmer {
                output: bench.output,
                layer: 1,
                dimmer: 0.3,
            },
            CommandSource::Sacn,
            mono,
        );
        assert_eq!(
            bench.tick(mono as i64, mono, &[]).resume_in_millis,
            Some(10_000)
        );
    }
    bench.command(
        CommandKind::TakeOverPlayback {
            output: bench.output,
            take_over: true,
        },
        CommandSource::Web,
        3000,
    );
    assert_eq!(bench.tick(3000, 3000, &[]).owner, "manual");
    bench.command(
        CommandKind::TakeOverPlayback {
            output: bench.output,
            take_over: false,
        },
        CommandSource::Web,
        15_000,
    );
    let recent = HashMap::from([(bench.output, at(14_999))]);
    let statuses = bench.engine.tick(
        &bench.state,
        &bench.catalog,
        &[],
        &recent,
        &HashSet::new(),
        15_000,
        at(15_000),
    );
    assert_eq!(statuses[0].owner, "dmx");
    assert_eq!(statuses[0].resume_in_millis, Some(9999));
}

#[test]
fn source_loss_does_not_replay_single_pass_events_that_started_during_dmx() {
    let mut bench = Bench::new();
    let (event, occurrence) = bench.once(500, &bench.playlist);
    bench.configure(vec![event], vec![bench.playlist.clone()]);
    bench.tick(0, 0, std::slice::from_ref(&occurrence));
    bench.command(
        CommandKind::SetLayerDimmer {
            output: bench.output,
            layer: 1,
            dimmer: 0.4,
        },
        CommandSource::ArtNet,
        100,
    );
    assert_eq!(
        bench
            .tick(100, 100, std::slice::from_ref(&occurrence))
            .owner,
        "dmx"
    );
    assert_eq!(
        bench
            .tick(500, 500, std::slice::from_ref(&occurrence))
            .owner,
        "dmx"
    );
    assert!(bench.engine.journal.consumed.contains(&occurrence.key));
    let status = bench.tick(10_100, 10_100, &[occurrence]);
    assert_eq!(status.owner, "fallback");
    assert_eq!(status.event_id, None);
}

#[test]
fn one_pass_is_not_duplicated_after_completion_or_backward_time_changes() {
    let mut bench = Bench::new();
    bench.settings.fallback_playlist_id = None;
    let (event, occurrence) = bench.once(100, &bench.playlist);
    bench.configure(vec![event], vec![bench.playlist.clone()]);
    assert_eq!(
        bench.tick(0, 0, std::slice::from_ref(&occurrence)).owner,
        "idle"
    );
    assert_eq!(
        bench
            .tick(100, 100, std::slice::from_ref(&occurrence))
            .owner,
        "schedule"
    );
    bench.command(
        CommandKind::ReportSourceStatus {
            output: bench.output,
            layer: 1,
            status: media_domain::SourceStatus::Ready,
        },
        CommandSource::Internal,
        101,
    );
    bench.tick(101, 101, std::slice::from_ref(&occurrence));
    bench.command(
        CommandKind::ReportSourceStatus {
            output: bench.output,
            layer: 1,
            status: media_domain::SourceStatus::Ready,
        },
        CommandSource::Internal,
        102,
    );
    assert_eq!(
        bench
            .tick(201, 201, std::slice::from_ref(&occurrence))
            .owner,
        "idle"
    );
    assert_eq!(
        bench.tick(0, 300, std::slice::from_ref(&occurrence)).owner,
        "idle"
    );
    assert_eq!(bench.tick(100, 400, &[occurrence]).owner, "idle");
}

#[test]
fn higher_priority_window_preempts_and_lower_window_restarts_from_entry_one() {
    let mut bench = Bench::new();
    bench.settings.fallback_playlist_id = None;
    let mut higher = bench.playlist.clone();
    higher.id = Playlist::default().id;
    higher.priority = 10;
    let (low_event, mut low) = bench.once(0, &bench.playlist);
    low.end_millis = Some(1000);
    let (high_event, mut high) = bench.once(100, &higher);
    high.end_millis = Some(200);
    bench.configure(
        vec![low_event, high_event],
        vec![bench.playlist.clone(), higher.clone()],
    );
    let occurrences = [low, high];
    assert_eq!(
        bench.tick(0, 0, &occurrences).playlist_id,
        Some(bench.playlist.id.to_string())
    );
    assert_eq!(
        bench.tick(100, 100, &occurrences).playlist_id,
        Some(higher.id.to_string())
    );
    let status = bench.tick(200, 200, &occurrences);
    assert_eq!(status.playlist_id, Some(bench.playlist.id.to_string()));
    assert_eq!(status.entry_index, Some(0));
    assert_eq!(status.phase, "loading");
}

#[test]
fn stopping_restores_snapshot_but_manual_owner_changes_prevent_stale_restore() {
    let mut bench = Bench::new();
    bench.command(
        CommandKind::SetLayerDimmer {
            output: bench.output,
            layer: 1,
            dimmer: 0.7,
        },
        CommandSource::Internal,
        0,
    );
    bench.configure(vec![], vec![bench.playlist.clone()]);
    bench.tick(0, 0, &[]);
    bench.settings.enabled = false;
    bench.configure(vec![], vec![bench.playlist.clone()]);
    bench.tick(100, 100, &[]);
    assert_eq!(
        bench.state.load().output(bench.output).unwrap().layers[1].dimmer,
        0.7
    );
    assert!(
        !bench
            .state
            .load()
            .output(bench.output)
            .unwrap()
            .ownership
            .scheduler_active
    );
    bench.settings.enabled = true;
    bench.configure(vec![], vec![bench.playlist.clone()]);
    bench.tick(200, 200, &[]);
    bench.command(
        CommandKind::TakeOverPlayback {
            output: bench.output,
            take_over: true,
        },
        CommandSource::Web,
        201,
    );
    bench.command(
        CommandKind::SetLayerDimmer {
            output: bench.output,
            layer: 1,
            dimmer: 0.2,
        },
        CommandSource::Web,
        202,
    );
    bench.tick(203, 203, &[]);
    assert_eq!(
        bench.state.load().output(bench.output).unwrap().layers[1].dimmer,
        0.2
    );
}

#[test]
fn failed_winner_and_failed_fallback_are_attempted_once_until_configuration_changes() {
    let mut bench = Bench::new();
    bench.playlist.entries[0].asset_id = AssetId::new();
    bench.settings.fallback_playlist_id = Some(bench.playlist.id);
    bench.configure(vec![], vec![bench.playlist.clone()]);
    let first = bench.tick(0, 0, &[]);
    assert_eq!(first.phase, "failed");
    assert_eq!(first.owner, "error");
    let generation = bench
        .state
        .load()
        .output(bench.output)
        .unwrap()
        .ownership
        .generation;
    for millis in [100, 200, 300] {
        assert_eq!(bench.tick(millis, millis as u64, &[]).phase, "failed");
        assert_eq!(
            bench
                .state
                .load()
                .output(bench.output)
                .unwrap()
                .ownership
                .generation,
            generation
        );
    }
}

#[test]
fn a_single_pass_that_cannot_activate_is_reported_failed_instead_of_started() {
    let mut bench = Bench::new();
    bench.playlist.entries[0].asset_id = AssetId::new();
    bench.settings.fallback_playlist_id = None;
    let (event, occurrence) = bench.once(500, &bench.playlist);
    bench.configure(vec![event], vec![bench.playlist.clone()]);
    bench.tick(0, 0, std::slice::from_ref(&occurrence));
    let result = bench.tick(500, 500, std::slice::from_ref(&occurrence));
    assert_eq!(result.owner, "error");
    assert_eq!(result.last_occurrence_outcome.as_deref(), Some("failed"));
}
