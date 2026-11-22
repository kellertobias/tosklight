//! Calendar occurrence expansion and deterministic selection. Adapters resolve local civil
//! times in the computer's time zone; this module never reads the operating system or a clock.

use std::collections::HashSet;

use chrono::{Datelike, Days, NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::OutputId;
use crate::playlist::{Playlist, ScheduleEvent, ScheduleTrigger};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Occurrence {
    pub key: String,
    pub event_id: Uuid,
    pub output_id: OutputId,
    pub playlist_id: Uuid,
    pub start_millis: i64,
    pub end_millis: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ScheduleError(pub String);

pub fn parse_local_datetime(value: &str) -> Result<NaiveDateTime, ScheduleError> {
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M"]
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
        .ok_or_else(|| {
            ScheduleError(format!(
                "invalid local date/time '{value}'; use YYYY-MM-DDTHH:mm:ss"
            ))
        })
}

pub fn parse_local_time(value: &str) -> Result<NaiveTime, ScheduleError> {
    ["%H:%M:%S%.f", "%H:%M"]
        .iter()
        .find_map(|format| NaiveTime::parse_from_str(value, format).ok())
        .ok_or_else(|| ScheduleError(format!("invalid local time '{value}'; use HH:mm:ss")))
}

fn date(value: &str) -> Result<NaiveDate, ScheduleError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| ScheduleError(format!("invalid calendar date '{value}'; use YYYY-MM-DD")))
}

pub fn validate_trigger(trigger: &ScheduleTrigger) -> Result<(), ScheduleError> {
    match trigger {
        ScheduleTrigger::Fixed { start, end } => {
            let start = parse_local_datetime(start)?;
            if let Some(end) = end {
                if parse_local_datetime(end)? <= start {
                    return Err(ScheduleError("end must be after start".into()));
                }
            }
        }
        ScheduleTrigger::Weekly {
            weekdays,
            start_time,
            end_time,
            end_day_offset,
            first_date,
            last_date,
        } => {
            if weekdays.is_empty() || weekdays.iter().any(|day| *day > 6) {
                return Err(ScheduleError(
                    "weekdays must select Monday (0) through Sunday (6)".into(),
                ));
            }
            let start = parse_local_time(start_time)?;
            if *end_day_offset > 7 {
                return Err(ScheduleError("endDayOffset must be between 0 and 7".into()));
            }
            if let Some(end) = end_time {
                if *end_day_offset == 0 && parse_local_time(end)? <= start {
                    return Err(ScheduleError("weekly end must be after start; choose the next day for an overnight window".into()));
                }
                parse_local_time(end)?;
            } else if *end_day_offset != 0 {
                return Err(ScheduleError("endDayOffset needs an endTime".into()));
            }
            let first = first_date.as_deref().map(date).transpose()?;
            let last = last_date.as_deref().map(date).transpose()?;
            if first.zip(last).is_some_and(|(first, last)| first > last) {
                return Err(ScheduleError(
                    "lastDate must not be before firstDate".into(),
                ));
            }
        }
    }
    Ok(())
}

/// Expand occurrences whose local start date falls within the inclusive calendar range.
/// A resolver returns the first instant for an ambiguous civil time and None for a missing time.
pub fn occurrences_between(
    event: &ScheduleEvent,
    first: NaiveDate,
    last: NaiveDate,
    resolve: impl Fn(NaiveDateTime) -> Option<i64>,
) -> Result<Vec<Occurrence>, ScheduleError> {
    validate_trigger(&event.trigger)?;
    if last < first || (last - first).num_days() > 3660 {
        return Err(ScheduleError(
            "occurrence range must be ordered and no longer than ten years".into(),
        ));
    }
    if !event.enabled {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    let mut add = |start: NaiveDateTime, end: Option<NaiveDateTime>| {
        if start.date() < first || start.date() > last {
            return;
        }
        let Some(start_millis) = resolve(start) else {
            return;
        };
        let end_millis = match end {
            Some(end) => match resolve(end) {
                Some(end) if end > start_millis => Some(end),
                _ => return,
            },
            None => None,
        };
        values.push(Occurrence {
            // The local start identity stays stable across backward clock changes and DST folds.
            key: format!("{}:{}", event.id, start.format("%Y-%m-%dT%H:%M:%S%.f")),
            event_id: event.id,
            output_id: event.output_id,
            playlist_id: event.playlist_id,
            start_millis,
            end_millis,
        });
    };
    match &event.trigger {
        ScheduleTrigger::Fixed { start, end } => add(
            parse_local_datetime(start)?,
            end.as_deref().map(parse_local_datetime).transpose()?,
        ),
        ScheduleTrigger::Weekly {
            weekdays,
            start_time,
            end_time,
            end_day_offset,
            first_date,
            last_date,
        } => {
            let start = parse_local_time(start_time)?;
            let end = end_time.as_deref().map(parse_local_time).transpose()?;
            let valid_first = first_date.as_deref().map(date).transpose()?;
            let valid_last = last_date.as_deref().map(date).transpose()?;
            let mut day = first;
            loop {
                if weekdays.contains(&(day.weekday().num_days_from_monday() as u8))
                    && valid_first.is_none_or(|first| day >= first)
                    && valid_last.is_none_or(|last| day <= last)
                {
                    let end = end.and_then(|time| {
                        day.checked_add_days(Days::new(u64::from(*end_day_offset)))
                            .map(|day| day.and_time(time))
                    });
                    add(day.and_time(start), end);
                }
                if day == last {
                    break;
                }
                let Some(next) = day.succ_opt() else {
                    break;
                };
                day = next;
            }
        }
    }
    values.sort_by_key(|value| (value.start_millis, value.event_id));
    Ok(values)
}

/// Choose the scheduled winner. None on startup suppresses old single-pass triggers. The adapter
/// clears due_since after a forward clock jump or a suspension, so missed starts are not replayed.
pub fn select_winner<'a>(
    occurrences: &'a [Occurrence],
    playlists: &[Playlist],
    output_id: OutputId,
    now_millis: i64,
    due_since: Option<i64>,
    consumed: &HashSet<String>,
    current_once: Option<&str>,
) -> Option<&'a Occurrence> {
    occurrences
        .iter()
        .filter_map(|occurrence| {
            if occurrence.output_id != output_id || occurrence.start_millis > now_millis {
                return None;
            }
            let playlist = playlists.iter().find(|playlist| {
                playlist.id == occurrence.playlist_id
                    && playlist.enabled
                    && !playlist.entries.is_empty()
            })?;
            let eligible = match occurrence.end_millis {
                Some(end) => now_millis < end,
                None => {
                    current_once == Some(occurrence.key.as_str())
                        || (!consumed.contains(&occurrence.key)
                            && due_since.is_some_and(|previous| {
                                previous < occurrence.start_millis
                                    && occurrence.start_millis <= now_millis
                            }))
                }
            };
            eligible.then_some((occurrence, playlist.priority))
        })
        .min_by(|(a, priority_a), (b, priority_b)| {
            priority_b
                .cmp(priority_a)
                .then(a.start_millis.cmp(&b.start_millis))
                .then(a.event_id.cmp(&b.event_id))
        })
        .map(|(occurrence, _)| occurrence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AssetId, playlist::PlaylistEntry};

    fn instant(value: &str) -> i64 {
        parse_local_datetime(value)
            .unwrap()
            .and_utc()
            .timestamp_millis()
    }
    fn event(trigger: ScheduleTrigger) -> ScheduleEvent {
        ScheduleEvent {
            id: Uuid::new_v4(),
            name: "Scheduled".into(),
            enabled: true,
            output_id: OutputId::new(),
            playlist_id: Uuid::new_v4(),
            trigger,
        }
    }
    fn expand(event: &ScheduleEvent) -> Vec<Occurrence> {
        occurrences_between(
            event,
            date("2026-11-20").unwrap(),
            date("2026-11-30").unwrap(),
            |local| Some(local.and_utc().timestamp_millis()),
        )
        .unwrap()
    }
    fn playlist(id: Uuid, priority: i32) -> Playlist {
        Playlist {
            id,
            priority,
            entries: vec![PlaylistEntry {
                transform: Default::default(),
                id: Uuid::new_v4(),
                asset_id: AssetId::new(),
                plays: 1,
                hold_millis: None,
                fade_millis: 0,
            }],
            ..Playlist::default()
        }
    }

    #[test]
    fn fixed_once_only_fires_at_due_crossing_and_never_catches_up_on_startup() {
        let event = event(ScheduleTrigger::Fixed {
            start: "2026-11-22T10:00:00".into(),
            end: None,
        });
        let occurrences = expand(&event);
        let playlists = vec![playlist(event.playlist_id, 0)];
        let now = instant("2026-11-22T10:00:00");
        let mut consumed = HashSet::new();
        assert!(
            select_winner(
                &occurrences,
                &playlists,
                event.output_id,
                now,
                None,
                &consumed,
                None
            )
            .is_none()
        );
        assert!(
            select_winner(
                &occurrences,
                &playlists,
                event.output_id,
                now,
                Some(now - 1),
                &consumed,
                None
            )
            .is_some()
        );
        consumed.insert(occurrences[0].key.clone());
        assert!(
            select_winner(
                &occurrences,
                &playlists,
                event.output_id,
                now,
                Some(now - 1),
                &consumed,
                None
            )
            .is_none()
        );
        assert!(
            select_winner(
                &occurrences,
                &playlists,
                event.output_id,
                now + 1000,
                Some(now),
                &consumed,
                Some(&occurrences[0].key)
            )
            .is_some()
        );
    }

    #[test]
    fn fixed_window_is_start_inclusive_and_end_exclusive() {
        let event = event(ScheduleTrigger::Fixed {
            start: "2026-11-22T10:00:00".into(),
            end: Some("2026-11-22T11:00:00".into()),
        });
        let occurrences = expand(&event);
        let playlists = vec![playlist(event.playlist_id, 0)];
        assert!(
            select_winner(
                &occurrences,
                &playlists,
                event.output_id,
                instant("2026-11-22T10:00:00"),
                None,
                &HashSet::new(),
                None
            )
            .is_some()
        );
        assert!(
            select_winner(
                &occurrences,
                &playlists,
                event.output_id,
                instant("2026-11-22T11:00:00"),
                None,
                &HashSet::new(),
                None
            )
            .is_none()
        );
    }

    #[test]
    fn weekly_once_and_overnight_windows_obey_weekdays_and_calendar_limits() {
        let mut event = event(ScheduleTrigger::Weekly {
            weekdays: vec![0, 4],
            start_time: "22:00".into(),
            end_time: None,
            end_day_offset: 0,
            first_date: Some("2026-11-23".into()),
            last_date: Some("2026-11-27".into()),
        });
        let once = expand(&event);
        assert_eq!(once.len(), 2);
        assert!(once.iter().all(|item| item.end_millis.is_none()));
        if let ScheduleTrigger::Weekly {
            end_time,
            end_day_offset,
            ..
        } = &mut event.trigger
        {
            *end_time = Some("02:00".into());
            *end_day_offset = 1;
        }
        let windows = expand(&event);
        assert_eq!(windows.len(), 2);
        assert_eq!(
            windows[0].end_millis.unwrap() - windows[0].start_millis,
            4 * 60 * 60 * 1000
        );
    }

    #[test]
    fn higher_priority_wins_and_ties_use_start_then_stable_identity() {
        let event = event(ScheduleTrigger::Fixed {
            start: "2026-11-22T10:00".into(),
            end: Some("2026-11-22T12:00".into()),
        });
        let mut second = event.clone();
        second.id = Uuid::from_u128(1);
        second.playlist_id = Uuid::new_v4();
        let mut occurrences = expand(&event);
        occurrences.extend(expand(&second));
        let mut playlists = vec![
            playlist(event.playlist_id, 1),
            playlist(second.playlist_id, 10),
        ];
        assert_eq!(
            select_winner(
                &occurrences,
                &playlists,
                event.output_id,
                instant("2026-11-22T11:00"),
                None,
                &HashSet::new(),
                None
            )
            .unwrap()
            .event_id,
            second.id
        );
        playlists[0].priority = 10;
        assert_eq!(
            select_winner(
                &occurrences,
                &playlists,
                event.output_id,
                instant("2026-11-22T11:00"),
                None,
                &HashSet::new(),
                None
            )
            .unwrap()
            .event_id,
            second.id.min(event.id)
        );
    }

    #[test]
    fn missing_local_times_are_skipped_and_ambiguous_times_have_one_identity() {
        let event = event(ScheduleTrigger::Fixed {
            start: "2026-11-22T10:00".into(),
            end: None,
        });
        let first = date("2026-11-22").unwrap();
        assert!(
            occurrences_between(&event, first, first, |_| None)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            occurrences_between(&event, first, first, |_| Some(123))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn invalid_windows_and_weekday_values_are_refused() {
        assert!(
            validate_trigger(&ScheduleTrigger::Fixed {
                start: "2026-11-22T10:00".into(),
                end: Some("2026-11-22T09:00".into())
            })
            .is_err()
        );
        assert!(
            validate_trigger(&ScheduleTrigger::Weekly {
                weekdays: vec![7],
                start_time: "10:00".into(),
                end_time: None,
                end_day_offset: 0,
                first_date: None,
                last_date: None
            })
            .is_err()
        );
    }
}
