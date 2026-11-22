//! Process-owned calendar evaluation and autonomous playback.

mod journal;
mod orchestration;
#[cfg(test)]
mod render_tests;
mod runner;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::{DateTime, Days, Local, TimeZone};
use media_domain::Timestamp;
use media_domain::playlist::{AutomationConfiguration, ScheduleTrigger};
use media_domain::schedule::{
    Occurrence, occurrences_between, parse_local_datetime, select_winner,
};
use media_http::automation::AutomationAccess;
use media_http::wire::{AutomationRuntimeView, UpcomingOccurrenceView};

use crate::Shutdown;
use crate::presentation::Shared;

pub(crate) fn spawn(
    runtime: &tokio::runtime::Runtime,
    shared: Shared,
    shutdown: Shutdown,
    started: Instant,
    access: AutomationAccess,
    journal_path: PathBuf,
    dmx_diagnostics: crate::dmx::SharedDiagnostics,
) -> tokio::task::JoinHandle<()> {
    runtime.spawn(async move {
        let mut engine = orchestration::Engine::new(Some(journal_path));
        let mut interval = tokio::time::interval(Duration::from_millis(33));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut watcher = shutdown.watcher();
        let mut calendar = Vec::new();
        let mut next_calendar = Instant::now();
        let mut view_revision = 0_u64;
        let mut last_upcoming = Vec::new();
        let mut last_zone = String::new();
        loop {
            tokio::select! {
                _ = watcher.wait() => {
                    engine.release_all(&shared.state, Timestamp::from_micros(started.elapsed().as_micros() as u64));
                    break;
                }
                _ = interval.tick() => {}
            }
            let local = Local::now();
            let now = Timestamp::from_micros(started.elapsed().as_micros() as u64);
            let configuration = shared.configuration.load_full();
            let mut effective = configuration.automation.clone();
            for output in &mut effective.outputs {
                output.enabled &= configuration.output(output.output_id).is_some_and(|physical| physical.enabled);
            }
            let changed = engine.configure(&effective);
            if changed || Instant::now() >= next_calendar {
                calendar = expand(&configuration.automation, local);
                next_calendar = Instant::now() + Duration::from_secs(1);
            }
            let apply_now = {
                let mut actions = access.apply_now.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                std::mem::take(&mut *actions)
            };
            let state = shared.state.load_full();
            let received: HashMap<_, _> = state.outputs.iter().filter_map(|output| {
                crate::dmx::last_received(&dmx_diagnostics, output.id).map(|at| (output.id, at))
            }).collect();
            let catalog = shared.catalog.load_full();
            let status = engine.tick(&shared.state, &catalog, &calendar, &received, &apply_now, local.timestamp_millis(), now);
            let upcoming = upcoming(&configuration.automation, &calendar, local.timestamp_millis());
            let time_zone = local.format("%Z (UTC%:z)").to_string();
            if changed || upcoming != last_upcoming || time_zone != last_zone || view_revision == 0 {
                view_revision = (view_revision + 1).min(9_007_199_254_740_991);
                last_upcoming.clone_from(&upcoming);
                last_zone.clone_from(&time_zone);
            }
            access.snapshot.store(std::sync::Arc::new(AutomationRuntimeView {
                time_zone,
                automation_revision: view_revision,
                status,
                upcoming,
            }));
        }
    })
}

fn expand(configuration: &AutomationConfiguration, now: DateTime<Local>) -> Vec<Occurrence> {
    let today = now.date_naive();
    let first = today.checked_sub_days(Days::new(7)).unwrap_or(today);
    let last = today.checked_add_days(Days::new(7)).unwrap_or(today);
    let mut occurrences = Vec::new();
    for event in &configuration.events {
        // A fixed window may have started months ago and still be active. Weekly windows can
        // extend at most seven days, so the bounded expansion is sufficient for those.
        let (from, until) = match &event.trigger {
            ScheduleTrigger::Fixed { start, .. } => {
                let Ok(start) = parse_local_datetime(start) else {
                    continue;
                };
                (start.date(), start.date())
            }
            ScheduleTrigger::Weekly { .. } => (first, last),
        };
        match occurrences_between(event, from, until, |local| {
            Local
                .from_local_datetime(&local)
                .earliest()
                .map(|time| time.timestamp_millis())
        }) {
            Ok(values) => occurrences.extend(values),
            Err(error) => {
                tracing::warn!(event = %event.id, %error, "cannot expand scheduled event")
            }
        }
    }
    occurrences.sort_by_key(|occurrence| (occurrence.start_millis, occurrence.event_id));
    occurrences
}

fn upcoming(
    configuration: &AutomationConfiguration,
    occurrences: &[Occurrence],
    now: i64,
) -> Vec<UpcomingOccurrenceView> {
    let empty = std::collections::HashSet::new();
    occurrences
        .iter()
        .filter(|occurrence| {
            occurrence.start_millis >= now || occurrence.end_millis.is_some_and(|end| end > now)
        })
        .take(100)
        .filter_map(|occurrence| {
            let event = configuration
                .events
                .iter()
                .find(|event| event.id == occurrence.event_id)?;
            let playlist = configuration
                .playlists
                .iter()
                .find(|playlist| playlist.id == occurrence.playlist_id)?;
            let instant = now.max(occurrence.start_millis);
            let winner = select_winner(
                occurrences,
                &configuration.playlists,
                occurrence.output_id,
                instant,
                Some(instant.saturating_sub(1)),
                &empty,
                None,
            );
            let winning = winner.is_some_and(|winner| winner.key == occurrence.key);
            Some(UpcomingOccurrenceView {
                event_id: occurrence.event_id.to_string(),
                playlist_id: occurrence.playlist_id.to_string(),
                output_id: occurrence.output_id.to_string(),
                name: event.name.clone(),
                start: local_string(occurrence.start_millis)?,
                end: occurrence.end_millis.and_then(local_string),
                priority: playlist.priority,
                winning,
                reason: if !playlist.enabled {
                    "Playlist disabled".into()
                } else if playlist.entries.is_empty() {
                    "Playlist has no clips".into()
                } else if winning {
                    "Highest eligible priority at this start time".into()
                } else {
                    "Overlaps an event with higher priority or an earlier equal-priority start"
                        .into()
                },
            })
        })
        .collect()
}

fn local_string(millis: i64) -> Option<String> {
    DateTime::from_timestamp_millis(millis).map(|time| {
        time.with_timezone(&Local)
            .format("%Y-%m-%dT%H:%M:%S%:z")
            .to_string()
    })
}
