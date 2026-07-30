use super::{
    SHOW_SCHEMA_VERSION, ScheduleOccurrenceClaim, ScheduleOccurrenceClaimResult,
    ScheduleOccurrenceResolution, ScheduleOccurrenceStatus, SkippedScheduleOccurrence,
};
use crate::{ShowStore, StoreError};
use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Barrier},
};
use uuid::Uuid;

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("light-schedule-{name}-{}.show", Uuid::new_v4()))
}

fn create(name: &str) -> (PathBuf, ShowStore) {
    let path = temporary(name);
    let (store, _) = ShowStore::create(&path, "Schedule test").unwrap();
    (path, store)
}

fn at(second: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 30, 20, 0, second)
        .single()
        .unwrap()
}

fn claim(schedule_id: &str, occurrence_id: &str, second: u32) -> ScheduleOccurrenceClaim {
    ScheduleOccurrenceClaim {
        schedule_id: schedule_id.into(),
        occurrence_id: occurrence_id.into(),
        scheduled_for: at(second),
        target_action: json!({
            "type": "playback",
            "playback_number": 41,
            "action": "go",
            "future": {"retain": true}
        }),
        claimed_at: at(second),
    }
}

#[test]
fn claim_is_durable_and_duplicate_identity_never_reopens_dispatch() {
    let (path, show) = create("claim");
    let first = show
        .claim_schedule_occurrence(&claim("doors", "local:2026-07-30T20:00:01", 1))
        .unwrap();
    let ScheduleOccurrenceClaimResult::Claimed(first) = first else {
        panic!("first write must claim the occurrence");
    };
    assert_eq!(first.status, ScheduleOccurrenceStatus::Claimed);
    assert_eq!(first.target_action["future"]["retain"], true);

    let mut changed = claim("doors", "local:2026-07-30T20:00:01", 2);
    changed.target_action = json!({"type":"playback","playback_number":99});
    let duplicate = show.claim_schedule_occurrence(&changed).unwrap();
    let ScheduleOccurrenceClaimResult::AlreadyRecorded(duplicate) = duplicate else {
        panic!("duplicate identity must return its durable claim");
    };
    assert_eq!(duplicate.sequence, first.sequence);
    assert_eq!(duplicate.scheduled_for, first.scheduled_for);
    assert_eq!(duplicate.target_action, first.target_action);
    drop(show);

    let reopened = ShowStore::open(&path).unwrap();
    assert_eq!(
        reopened
            .schedule_occurrence("doors", "local:2026-07-30T20:00:01")
            .unwrap(),
        Some(first)
    );
    drop(reopened);
    let _ = fs::remove_file(path);
}

#[test]
fn concurrent_claimers_commit_exactly_one_dispatch_claim() {
    let (path, show) = create("concurrent-claim");
    drop(show);
    let barrier = Arc::new(Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let show = ShowStore::open(path).unwrap();
                barrier.wait();
                show.claim_schedule_occurrence(&claim("doors", "same-occurrence", 1))
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, ScheduleOccurrenceClaimResult::Claimed(_)))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, ScheduleOccurrenceClaimResult::AlreadyRecorded(_)))
            .count(),
        1
    );
    let reopened = ShowStore::open(&path).unwrap();
    assert_eq!(
        reopened.schedule_occurrence_history("doors").unwrap().len(),
        1
    );
    drop(reopened);
    let _ = fs::remove_file(path);
}

#[test]
fn completion_failure_interruption_and_skip_are_terminal_statuses() {
    let (path, show) = create("statuses");
    for (index, id) in ["complete", "fail", "interrupt"].into_iter().enumerate() {
        show.claim_schedule_occurrence(&claim("status", id, index as u32))
            .unwrap();
    }

    let completed = show
        .resolve_schedule_occurrence(
            "status",
            "complete",
            ScheduleOccurrenceResolution::Completed,
            at(10),
        )
        .unwrap();
    assert_eq!(completed.status, ScheduleOccurrenceStatus::Completed);
    assert_eq!(completed.resolved_at, Some(at(10)));
    assert_eq!(completed.result_detail, None);

    let failed = show
        .resolve_schedule_occurrence(
            "status",
            "fail",
            ScheduleOccurrenceResolution::Failed {
                reason: "Playback target disappeared".into(),
            },
            at(11),
        )
        .unwrap();
    assert_eq!(failed.status, ScheduleOccurrenceStatus::Failed);
    assert_eq!(
        failed.result_detail.as_deref(),
        Some("Playback target disappeared")
    );

    let interrupted = show
        .resolve_schedule_occurrence(
            "status",
            "interrupt",
            ScheduleOccurrenceResolution::Interrupted {
                reason: "runtime stopped".into(),
            },
            at(12),
        )
        .unwrap();
    assert_eq!(interrupted.status, ScheduleOccurrenceStatus::Interrupted);

    let skipped = show
        .record_skipped_schedule_occurrence(&SkippedScheduleOccurrence {
            schedule_id: "status".into(),
            occurrence_id: "skipped".into(),
            scheduled_for: at(3),
            target_action: json!({"type":"playback","action":"off"}),
            skipped_at: at(13),
            reason: "owning show was inactive".into(),
        })
        .unwrap();
    assert_eq!(skipped.status, ScheduleOccurrenceStatus::Skipped);
    assert_eq!(skipped.recorded_at, at(13));
    assert_eq!(skipped.resolved_at, Some(at(13)));
    assert_eq!(
        skipped.result_detail.as_deref(),
        Some("owning show was inactive")
    );

    let completed_again = show
        .resolve_schedule_occurrence(
            "status",
            "complete",
            ScheduleOccurrenceResolution::Completed,
            at(14),
        )
        .unwrap();
    assert_eq!(completed_again, completed);
    assert!(matches!(
        show.resolve_schedule_occurrence(
            "status",
            "complete",
            ScheduleOccurrenceResolution::Failed {
                reason: "late rewrite".into()
            },
            at(15)
        ),
        Err(StoreError::Invalid(message))
            if message == "Schedule occurrence is already completed"
    ));
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn startup_interrupts_unfinished_claims_without_reopening_terminal_rows() {
    let (path, show) = create("startup-interrupted");
    for id in ["unfinished-a", "unfinished-b", "completed"] {
        show.claim_schedule_occurrence(&claim("startup", id, 1))
            .unwrap();
    }
    show.resolve_schedule_occurrence(
        "startup",
        "completed",
        ScheduleOccurrenceResolution::Completed,
        at(2),
    )
    .unwrap();

    assert_eq!(
        show.interrupt_claimed_schedule_occurrences(at(3), "server restarted")
            .unwrap(),
        2
    );
    assert_eq!(
        show.schedule_occurrence("startup", "unfinished-a")
            .unwrap()
            .unwrap()
            .status,
        ScheduleOccurrenceStatus::Interrupted
    );
    assert_eq!(
        show.schedule_occurrence("startup", "completed")
            .unwrap()
            .unwrap()
            .status,
        ScheduleOccurrenceStatus::Completed
    );
    assert_eq!(
        show.interrupt_claimed_schedule_occurrences(at(4), "server restarted")
            .unwrap(),
        0
    );
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn history_retains_exactly_the_latest_hundred_rows_per_schedule() {
    let (path, show) = create("retention");
    for index in 0..105 {
        let second = u32::try_from(index % 60).unwrap();
        show.claim_schedule_occurrence(&claim(
            "bounded",
            &format!("occurrence-{index:03}"),
            second,
        ))
        .unwrap();
    }
    show.claim_schedule_occurrence(&claim("other", "only", 0))
        .unwrap();

    let history = show.schedule_occurrence_history("bounded").unwrap();
    assert_eq!(history.len(), 100);
    assert_eq!(history[0].occurrence_id, "occurrence-104");
    assert_eq!(history[99].occurrence_id, "occurrence-005");
    assert_eq!(
        show.latest_schedule_occurrence("bounded").unwrap().unwrap(),
        history[0]
    );
    assert!(
        show.schedule_occurrence("bounded", "occurrence-004")
            .unwrap()
            .is_none()
    );
    assert_eq!(show.schedule_occurrence_history("other").unwrap().len(), 1);
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn occurrence_writes_do_not_change_show_revision_or_object_undo_history() {
    let (path, show) = create("no-object-churn");
    show.put_object(
        "schedule",
        "doors",
        &json!({"name":"Doors","future":{"retain":true}}),
        0,
    )
    .unwrap();
    let revision = show.portable_revision().unwrap();
    let object_history = row_count(&show.conn, "object_history");

    show.claim_schedule_occurrence(&claim("doors", "one", 1))
        .unwrap();
    show.resolve_schedule_occurrence(
        "doors",
        "one",
        ScheduleOccurrenceResolution::Completed,
        at(2),
    )
    .unwrap();

    assert_eq!(show.portable_revision().unwrap(), revision);
    assert_eq!(row_count(&show.conn, "object_history"), object_history);
    assert_eq!(
        show.objects("schedule").unwrap()[0].body,
        json!({"name":"Doors","future":{"retain":true}})
    );
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn schema_four_show_migrates_without_rewriting_unknown_or_schedule_objects() {
    let (path, show) = create("v4-migration");
    show.put_object(
        "schedule",
        "future",
        &json!({"schema":99,"opaque":{"retain":[3,1,2]}}),
        0,
    )
    .unwrap();
    show.put_object(
        "future_extension",
        "opaque",
        &json!({"bytes":[0,255],"unknown":true}),
        0,
    )
    .unwrap();
    let revision = show.portable_revision().unwrap();
    show.conn
        .execute_batch("DROP TABLE schedule_occurrences; UPDATE schema_info SET version=4;")
        .unwrap();
    drop(show);

    let migrated = ShowStore::open(&path).unwrap();
    assert_eq!(schema_version(&migrated.conn), SHOW_SCHEMA_VERSION);
    assert_eq!(migrated.portable_revision().unwrap(), revision);
    assert_eq!(
        migrated.objects("schedule").unwrap()[0].body,
        json!({"schema":99,"opaque":{"retain":[3,1,2]}})
    );
    assert_eq!(
        migrated.objects("future_extension").unwrap()[0].body,
        json!({"bytes":[0,255],"unknown":true})
    );
    assert!(
        migrated
            .schedule_occurrence_history("future")
            .unwrap()
            .is_empty()
    );
    drop(migrated);

    let reopened = ShowStore::open(&path).unwrap();
    assert_eq!(schema_version(&reopened.conn), SHOW_SCHEMA_VERSION);
    assert!(
        reopened
            .schedule_occurrence_history("future")
            .unwrap()
            .is_empty()
    );
    drop(reopened);
    let _ = fs::remove_file(path);
}

#[test]
fn backup_and_reopen_preserve_bounded_occurrence_status() {
    let (source, show) = create("backup");
    let backup = temporary("backup-copy");
    show.claim_schedule_occurrence(&claim("doors", "one", 1))
        .unwrap();
    show.resolve_schedule_occurrence(
        "doors",
        "one",
        ScheduleOccurrenceResolution::Failed {
            reason: "authoritative Playback rejected the action".into(),
        },
        at(2),
    )
    .unwrap();

    show.backup_to(&backup).unwrap();
    let copied = ShowStore::open(&backup).unwrap();
    let history = copied.schedule_occurrence_history("doors").unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, ScheduleOccurrenceStatus::Failed);
    assert_eq!(
        history[0].result_detail.as_deref(),
        Some("authoritative Playback rejected the action")
    );
    drop(copied);
    drop(show);
    let _ = fs::remove_file(source);
    let _ = fs::remove_file(backup);
}

fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn schema_version(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap()
}
