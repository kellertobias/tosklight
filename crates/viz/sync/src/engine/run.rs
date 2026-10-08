//! The background task: connect, catch up, send the journal in order, follow the sync feed.

use super::remote::{CommitResult, Snapshot, on_document};
use super::{Answer, Shared};
use crate::desk::{DeskError, FeedMessage, FeedSocket, TransactionReply};
use crate::journal::{EntryOutcome, EntryState, JournalEntry};
use crate::status::Connection;
use light_wire::v2::show_sync::{
    ShowSyncApp, ShowSyncCommit, ShowSyncErrorKind, ShowSyncOrigin, ShowSyncStatus,
    ShowSyncTransactionOutcome, ShowSyncTransactionRequest,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use uuid::Uuid;

/// Why a connected session ended.
enum Leave {
    Stopped,
    Unreachable(String),
    NotActive(Option<Uuid>),
    /// A refusal the operator must see; reconnecting will not help until something changes.
    Error(String),
}

pub(crate) async fn run(shared: Arc<Shared>) {
    let mut backoff = Duration::from_millis(250);
    loop {
        if shared.stopped.load(Ordering::SeqCst) {
            return;
        }
        if shared.paused.load(Ordering::SeqCst) {
            shared.set_connection(Connection::Paused);
            shared.wake.notified().await;
            continue;
        }
        let leave = session(&shared).await;
        trace(&format!("left the desk: {}", describe(&leave)));
        let wait = match leave {
            Leave::Stopped => return,
            Leave::Unreachable(reason) => {
                shared.set_connection(Connection::Unreachable(reason));
                backoff = (backoff * 2).min(Duration::from_secs(5));
                backoff
            }
            Leave::NotActive(active) => {
                shared.set_connection(Connection::ShowNotActive(active));
                backoff = Duration::from_millis(250);
                Duration::from_secs(1)
            }
            Leave::Error(error) => {
                shared.set_connection(Connection::Refused(error));
                Duration::from_secs(5)
            }
        };
        let _ = tokio::time::timeout(wait, shared.wake.notified()).await;
    }
}

/// One connected stretch: verify the desk and the show, subscribe, catch up, then follow.
async fn session(shared: &Arc<Shared>) -> Leave {
    let (show_id, expected_desk) = {
        let binding = shared.binding.lock();
        (binding.show_id, binding.desk_identity)
    };
    let readiness = match shared.client.readiness().await {
        Ok(readiness) => readiness,
        Err(error) => return Leave::Unreachable(error.to_string()),
    };
    match (expected_desk, readiness.desk_identity) {
        (Some(expected), Some(actual)) if expected != actual => {
            return Leave::Error(format!(
                "A different Control desk answers at {}. This document stays bound to its own desk and sends nothing to this one.",
                shared.client.base()
            ));
        }
        (None, Some(actual)) => {
            let mut binding = shared.binding.lock();
            binding.desk_identity = Some(actual);
            let _ = shared.store.update(&binding);
        }
        _ => {}
    }
    trace(&format!("readiness {readiness:?}"));
    if readiness.active_show != Some(show_id) {
        return Leave::NotActive(readiness.active_show);
    }
    let mut socket = match shared.client.subscribe().await {
        Ok(socket) => socket,
        Err(error) => return Leave::Unreachable(error.to_string()),
    };
    // Subscribed first, read second: a commit between the two is in the feed or in the read.
    match shared.client.show_revision(show_id).await {
        Ok(Some(revision)) => {
            trace(&format!(
                "desk revision {revision}, mirror {}",
                shared.mirror.lock().show_revision()
            ));
            let behind = {
                let mirror = shared.mirror.lock();
                !mirror.trusted() || mirror.show_revision() != revision
            } || shared.rebuild_mirror.load(Ordering::SeqCst);
            if behind && let Err(leave) = resynchronize(shared, show_id).await {
                return leave;
            }
        }
        Ok(None) => return Leave::NotActive(None),
        Err(error) => return Leave::Unreachable(error.to_string()),
    }
    shared.set_connection(Connection::Connected);
    follow(shared, &mut socket, show_id).await
}

async fn follow(shared: &Arc<Shared>, socket: &mut FeedSocket, show_id: Uuid) -> Leave {
    loop {
        if shared.stopped.load(Ordering::SeqCst) {
            return Leave::Stopped;
        }
        if shared.paused.load(Ordering::SeqCst) {
            return Leave::Unreachable("working offline".into());
        }
        if let Err(leave) = flush(shared, show_id).await {
            return leave;
        }
        let message = tokio::select! {
            message = socket.next() => message,
            () = shared.wake.notified() => continue,
            () = tokio::time::sleep(Duration::from_secs(2)) => continue,
        };
        let Some(message) = message else {
            return Leave::Unreachable("the event stream closed".into());
        };
        let outcome = match message {
            FeedMessage::Committed(change) if change.show_id == show_id => {
                apply_commit(shared, show_id, *change).await
            }
            FeedMessage::Committed(_) => return Leave::NotActive(None),
            FeedMessage::Gap(gap) if gap.show_id == show_id => {
                if gap.show_revision > shared.mirror.lock().show_revision() {
                    resynchronize(shared, show_id).await
                } else {
                    Ok(())
                }
            }
            FeedMessage::Gap(gap) => return Leave::NotActive(Some(gap.show_id)),
            FeedMessage::StreamGap => resynchronize(shared, show_id).await,
            FeedMessage::Other => Ok(()),
        };
        if let Err(leave) = outcome {
            return leave;
        }
        shared.publish_status();
    }
}

async fn apply_commit(
    shared: &Arc<Shared>,
    show_id: Uuid,
    change: ShowSyncCommit,
) -> Result<(), Leave> {
    let mut fetched = BTreeMap::new();
    for object in change.objects.iter().filter(|object| object.body_omitted) {
        match shared
            .client
            .object(show_id, &object.kind, &object.id)
            .await
        {
            Ok(Some(found)) => {
                fetched.insert((object.kind.clone(), object.id.clone()), found);
            }
            Ok(None) => {}
            Err(error) => return Err(Leave::Unreachable(error.to_string())),
        }
    }
    let change = Arc::new(change);
    let applied = {
        let change = change.clone();
        on_document(shared, move |shared, document| {
            shared.apply_commit(document, &change, &fetched)
        })
        .await
    };
    match applied {
        Ok(CommitResult::NeedsSnapshot) => resynchronize(shared, show_id).await,
        Ok(_) => Ok(()),
        Err(error) => {
            shared.facts.lock().error = Some(error);
            Ok(())
        }
    }
}

/// Re-reads the desk's show as one consistent snapshot and rebuilds mirror and document from it.
async fn resynchronize(shared: &Arc<Shared>, show_id: Uuid) -> Result<(), Leave> {
    trace("re-reading the show");
    shared.facts.lock().resynchronizing = true;
    shared.snapshot_reads.fetch_add(1, Ordering::SeqCst);
    shared.publish_status();
    let result = async {
        let bytes = shared
            .client
            .download(show_id)
            .await
            .map_err(|error| desk_leave(&error))?;
        let path = shared
            .directory
            .join(format!(".snapshot-{}.show", Uuid::new_v4()));
        std::fs::write(&path, bytes).map_err(|error| Leave::Error(error.to_string()))?;
        let snapshot = Snapshot::read(&path);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
        let snapshot = Arc::new(snapshot.map_err(Leave::Error)?);
        on_document(shared, move |shared, document| {
            let changed = shared.adopt_snapshot(document, &snapshot)?;
            Ok(((), changed))
        })
        .await
        .map_err(Leave::Error)
    }
    .await;
    shared.facts.lock().resynchronizing = false;
    trace(&format!(
        "re-read finished: {}",
        result.as_ref().map_or_else(describe, |_| "ok".into())
    ));
    shared.publish_status();
    result
}

fn desk_leave(error: &DeskError) -> Leave {
    match error {
        DeskError::Unreachable(reason) => Leave::Unreachable(reason.clone()),
        DeskError::Http { status: 409, .. } => Leave::NotActive(None),
        other => Leave::Unreachable(other.to_string()),
    }
}

/// Sends every pending entry, oldest first, each with the identity it was journaled under.
async fn flush(shared: &Arc<Shared>, show_id: Uuid) -> Result<(), Leave> {
    let mut busy_retries = 0;
    loop {
        let entry = match shared.journal.lock().pending() {
            Ok(pending) => pending.into_iter().next(),
            Err(error) => return Err(Leave::Error(error)),
        };
        let Some(entry) = entry else {
            return Ok(());
        };
        let request = match request(shared, show_id, &entry) {
            Ok(request) => request,
            Err(error) => {
                reject(shared, &entry, error).await;
                continue;
            }
        };
        let mut reply = shared.client.transaction(&request).await;
        trace(&format!("transaction {} -> {reply:?}", entry.request_id));
        if reply.is_ok() && shared.lose_next_reply.swap(false, Ordering::SeqCst) {
            reply = Err(DeskError::Unreachable("the reply was lost".into()));
        }
        match reply {
            Ok(TransactionReply::Outcome(outcome)) => record(shared, &entry, *outcome).await,
            Ok(TransactionReply::Refused { error, .. }) => match error.kind {
                ShowSyncErrorKind::ShowNotActive => {
                    return Err(Leave::NotActive(error.active_show_id));
                }
                ShowSyncErrorKind::DeskMismatch => {
                    return Err(Leave::Error(format!(
                        "A different Control desk answers at {}. This document stays bound to its own desk and sends nothing to this one.",
                        shared.client.base()
                    )));
                }
                ShowSyncErrorKind::Unavailable | ShowSyncErrorKind::Internal
                    if error.retryable && busy_retries < 20 =>
                {
                    busy_retries += 1;
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                _ => reject(shared, &entry, error.error).await,
            },
            Err(error) => return Err(desk_leave(&error)),
        }
    }
}

fn request(
    shared: &Shared,
    show_id: Uuid,
    entry: &JournalEntry,
) -> Answer<ShowSyncTransactionRequest> {
    let binding = shared.binding.lock().clone();
    let known = shared.known_revisions.lock().clone();
    let mirror = shared.mirror.lock();
    let operations = entry
        .operations
        .iter()
        .map(|operation| {
            operation.to_wire(|key| {
                known
                    .get(key)
                    .copied()
                    .or_else(|| mirror.state().revisions.get(key).copied())
            })
        })
        .collect::<Answer<Vec<_>>>()?;
    Ok(ShowSyncTransactionRequest {
        request_id: entry.request_id.clone(),
        association_id: binding.association_id,
        show_id,
        base_show_revision: entry.base_show_revision,
        origin: ShowSyncOrigin {
            app: ShowSyncApp::Architect,
            desk_identity: binding.desk_identity,
            client_instance: Some(format!("architect:{}", binding.association_id)),
        },
        operations,
    })
}

async fn record(shared: &Arc<Shared>, entry: &JournalEntry, outcome: ShowSyncTransactionOutcome) {
    {
        let mut known = shared.known_revisions.lock();
        for object in &outcome.applied {
            let key = crate::state::ObjectKey::new(&object.kind, &object.id);
            match object.revision {
                Some(revision) if !object.deleted => {
                    known.insert(key, revision);
                }
                _ => {
                    known.remove(&key);
                }
            }
        }
    }
    let mut recorded = EntryOutcome::from_wire(&outcome);
    let state = match outcome.status {
        ShowSyncStatus::Accepted => EntryState::Accepted,
        ShowSyncStatus::Conflicted => {
            let shadow = shared.shadow.lock();
            for conflict in &outcome.conflicts {
                let key = crate::state::ObjectKey::new(&conflict.kind, &conflict.id);
                if let Some(body) = shadow.versioned.state.objects.get(&key) {
                    recorded
                        .drafts
                        .insert(format!("{}/{}", key.kind, key.id), body.clone());
                }
            }
            EntryState::Conflict
        }
    };
    if let Err(error) = shared.journal.lock().record(entry.seq, state, &recorded) {
        shared.facts.lock().error = Some(error);
        return;
    }
    if state == EntryState::Conflict {
        rebuild_entry(shared, entry.seq).await;
    } else {
        let mirrored = shared.mirror.lock().show_revision();
        let _ = shared.journal.lock().prune_accepted(mirrored, 64);
    }
}

async fn reject(shared: &Arc<Shared>, entry: &JournalEntry, error: String) {
    let outcome = EntryOutcome {
        error: Some(error),
        ..EntryOutcome::default()
    };
    if let Err(error) = shared
        .journal
        .lock()
        .record(entry.seq, EntryState::Rejected, &outcome)
    {
        shared.facts.lock().error = Some(error);
        return;
    }
    rebuild_entry(shared, entry.seq).await;
}

/// Shows the desk's values wherever an entry no longer overlays the document.
async fn rebuild_entry(shared: &Arc<Shared>, seq: i64) {
    let keys = match shared.keys_of(seq) {
        Ok(keys) => keys,
        Err(error) => {
            shared.facts.lock().error = Some(error);
            return;
        }
    };
    let result = on_document(shared, move |shared, document| {
        let changed = shared.rebase(document, Some(&keys), &BTreeMap::new())?;
        Ok(((), changed))
    })
    .await;
    if let Err(error) = result {
        shared.facts.lock().error = Some(error);
    }
    shared.publish_status();
}

/// Writes engine progress to stderr when `VIZ_SYNC_TRACE` is set.
pub(crate) fn trace(message: &str) {
    if std::env::var_os("VIZ_SYNC_TRACE").is_some() {
        eprintln!("viz-sync: {message}");
    }
}

fn describe(leave: &Leave) -> String {
    match leave {
        Leave::Stopped => "stopped".into(),
        Leave::Unreachable(reason) => format!("unreachable: {reason}"),
        Leave::NotActive(active) => format!("show not active (active {active:?})"),
        Leave::Error(error) => format!("error: {error}"),
    }
}
