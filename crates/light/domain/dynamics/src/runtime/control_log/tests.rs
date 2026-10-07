use super::*;

fn log(capacity: usize) -> ControlLog<String> {
    ControlLog::new(NonZeroUsize::new(capacity).unwrap())
}

fn values(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn batches_preserve_order_and_are_independent_of_later_storage_changes() {
    let mut log = log(4);
    let start = log.cursor();
    log.append(values(&["start", "pause"]));
    let after_pause = log.cursor();
    let batch = log.read(start).unwrap();
    assert_eq!(batch.from, start);
    assert_eq!(batch.to, after_pause);
    log.append(values(&["resume"]));
    assert_eq!(batch.entries, values(&["start", "pause"]));
    assert_eq!(
        log.read(start).unwrap().entries,
        values(&["start", "pause", "resume"])
    );
    assert_eq!(log.read(after_pause).unwrap().entries, values(&["resume"]));
    log.acknowledge(log.cursor()).unwrap();
    log.reset();
    assert_eq!(batch.entries, values(&["start", "pause"]));
}

#[test]
fn current_cursor_and_empty_append_leave_an_empty_batch() {
    let mut log = log(2);
    let cursor = log.cursor();
    log.append(vec![]);
    let batch = log.read(cursor).unwrap();
    assert_eq!(batch.from, cursor);
    assert_eq!(batch.to, cursor);
    assert!(batch.entries.is_empty());
    log.acknowledge(cursor).unwrap();
}

#[test]
fn acknowledgement_releases_only_records_through_the_cursor() {
    let mut log = log(4);
    let start = log.cursor();
    log.append(values(&["start"]));
    let first = log.cursor();
    log.append(values(&["pause", "resume"]));
    log.acknowledge(first).unwrap();
    assert_eq!(log.read(start).unwrap_err(), ControlLogError::HistoryLost);
    assert_eq!(
        log.read(first).unwrap().entries,
        values(&["pause", "resume"])
    );
    assert_eq!(log.acknowledge(start), Err(ControlLogError::HistoryLost));
    log.acknowledge(first).unwrap();
    let end = log.cursor();
    log.acknowledge(end).unwrap();
    assert!(log.entries.is_empty());
    assert!(log.read(end).unwrap().entries.is_empty());
}

#[test]
fn capacity_drops_old_history_without_blocking_new_controls() {
    let mut log = log(2);
    let start = log.cursor();
    log.append(values(&["start"]));
    let first = log.cursor();
    log.append(values(&["pause", "resume"]));
    assert_eq!(log.read(start).unwrap_err(), ControlLogError::HistoryLost);
    assert_eq!(
        log.read(first).unwrap().entries,
        values(&["pause", "resume"])
    );
    assert_eq!(log.entries.len(), 2);
}

#[test]
fn oversized_batch_keeps_its_suffix_and_advances_for_every_record() {
    let mut log = log(2);
    log.append(values(&["before"]));
    let before = log.cursor();
    log.append(values(&["one", "two", "three", "four"]));
    assert_eq!(log.cursor().sequence, before.sequence + 4);
    assert_eq!(log.read(before).unwrap_err(), ControlLogError::HistoryLost);
    let suffix = ControlCursor {
        epoch: before.epoch,
        sequence: before.sequence + 2,
    };
    assert_eq!(
        log.read(suffix).unwrap().entries,
        values(&["three", "four"])
    );
}

#[test]
fn wrong_epoch_and_future_cursors_are_rejected_without_mutation() {
    let mut log = log(2);
    log.append(values(&["start"]));
    let cursor = log.cursor();
    let future = ControlCursor {
        sequence: cursor.sequence + 1,
        ..cursor
    };
    let wrong = ControlCursor {
        epoch: Uuid::new_v4(),
        ..cursor
    };
    assert_eq!(log.read(future).unwrap_err(), ControlLogError::FutureCursor);
    assert_eq!(log.acknowledge(future), Err(ControlLogError::FutureCursor));
    assert_eq!(log.read(wrong).unwrap_err(), ControlLogError::WrongEpoch);
    assert_eq!(log.acknowledge(wrong), Err(ControlLogError::WrongEpoch));
    assert_eq!(log.cursor(), cursor);
    assert_eq!(log.entries.len(), 1);
}

#[test]
fn reset_rejects_old_cursors_even_when_the_sequence_is_equal() {
    let mut log = log(2);
    let old = log.cursor();
    log.reset();
    assert_eq!(log.cursor().sequence, old.sequence);
    assert_ne!(log.cursor().epoch, old.epoch);
    assert_eq!(log.read(old).unwrap_err(), ControlLogError::WrongEpoch);
    assert!(log.read(log.cursor()).unwrap().entries.is_empty());
}

#[test]
fn sequence_overflow_starts_a_new_epoch_and_retains_the_new_batch() {
    let mut log = log(3);
    log.cursor.sequence = u64::MAX - 1;
    let old = log.cursor();
    log.append(values(&["start", "pause"]));
    assert_ne!(log.cursor().epoch, old.epoch);
    assert_eq!(log.cursor().sequence, 2);
    assert_eq!(log.read(old).unwrap_err(), ControlLogError::WrongEpoch);
    let start = ControlCursor {
        sequence: 0,
        ..log.cursor()
    };
    assert_eq!(
        log.read(start).unwrap().entries,
        values(&["start", "pause"])
    );
}

#[test]
fn cloned_log_mutations_do_not_publish_into_the_original() {
    let mut live = log(2);
    let start = live.cursor();
    live.append(values(&["start"]));
    let mut candidate = live.clone();
    candidate.append(values(&["pause"]));
    candidate.acknowledge(candidate.cursor()).unwrap();
    assert_eq!(live.read(start).unwrap().entries, values(&["start"]));
    assert_eq!(live.cursor().sequence, 1);
}
