//! The Control → Architect sync feed.
//!
//! Every accepted commit of the active show passes `ServerActiveShowUnitOfWork::commit`, which
//! publishes it here — so desk UI edits, OSC, imports, patch changes and sync transactions all
//! reach a bound Architect the same way. Paths that change the show without that commit (a file
//! replaced on open, rollback or overwrite; a metadata write) publish a gap instead, and a commit
//! that finds the show moved since the last announcement publishes the missing gap first.
//!
//! The feed travels on `/api/v2/events` as the opt-in `show_sync` topic. It is published only
//! while a subscription has opted in, so a desk without a bound Architect keeps exactly the
//! event stream it had — one event per commit. A client that was not subscribed for a while
//! notices through `previous_show_revision` (or a gap) and re-reads snapshots; the show revision,
//! not the event stream, is the durable cursor.

use super::{ActiveShowRepository, AppState};
use light_application::{
    EventDraft,
    show_sync::{ShowSyncGapChange, ShowSyncGapReason, ShowSyncPublication},
};
use light_core::ShowId;
use light_show::{PortableShowCommit, PortableShowRevision};

/// Announces one committed active-show transaction.
pub(super) fn publish_commit(
    state: &AppState,
    show_id: ShowId,
    previous: PortableShowRevision,
    commit: &PortableShowCommit,
) {
    if commit.revision() == previous {
        return;
    }
    let announced = state
        .active_show
        .advance_sync_watermark(show_id, commit.revision().value());
    if !has_subscriber(state) {
        return;
    }
    if announced.is_some_and(|announced| announced != previous.value()) {
        publish_gap_at(
            state,
            show_id,
            previous.value(),
            ShowSyncGapReason::OutOfBandWrite,
        );
    }
    state.events.publish(EventDraft::show_sync_published(
        ShowSyncPublication::for_commit(show_id, previous, commit),
    ));
}

/// Announces that the active show moved outside the incremental commit path. Mirrors re-read
/// their synchronized kinds before trusting their state again.
pub(super) fn publish_gap(state: &AppState, show_id: ShowId, reason: ShowSyncGapReason) {
    let Some(entry) = state
        .active_show
        .current()
        .filter(|entry| entry.id == show_id)
    else {
        return;
    };
    let revision =
        match ActiveShowRepository::open(&entry.path).and_then(|store| store.portable_revision()) {
            Ok(revision) => revision.value(),
            Err(error) => {
                tracing::warn!(%error, "could not read the active show revision for the sync feed");
                return;
            }
        };
    state.active_show.advance_sync_watermark(show_id, revision);
    publish_gap_at(state, show_id, revision, reason);
}

/// What identifies the active show's file on disk: the show, and the file's size, modification
/// time and (on Unix) inode, or `None` when the file is missing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ActiveShowFile {
    show_id: ShowId,
    path: std::path::PathBuf,
    stamp: Option<(u64, Option<std::time::SystemTime>, u64)>,
}

/// Records the active show's file before a path that may replace or delete it.
pub(super) fn active_show_file(state: &AppState) -> Option<ActiveShowFile> {
    let entry = state.active_show.current()?;
    let path = std::path::PathBuf::from(&entry.path);
    Some(ActiveShowFile {
        show_id: entry.id,
        stamp: file_stamp(&path),
        path,
    })
}

fn file_stamp(path: &std::path::Path) -> Option<(u64, Option<std::time::SystemTime>, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    #[cfg(unix)]
    let inode = std::os::unix::fs::MetadataExt::ino(&metadata);
    #[cfg(not(unix))]
    let inode = 0;
    Some((metadata.len(), metadata.modified().ok(), inode))
}

/// Announces a gap when the active show's file was replaced or deleted since `before` without
/// passing through activation — the file manager can do that to any file under the shows root.
pub(super) fn announce_if_active_show_file_changed(
    state: &AppState,
    before: Option<ActiveShowFile>,
) {
    let Some(before) = before else {
        return;
    };
    let now = file_stamp(&before.path);
    if now == before.stamp {
        return;
    }
    if now.is_some() {
        publish_gap(state, before.show_id, ShowSyncGapReason::ShowReplaced);
        return;
    }
    // The file is gone: there is no revision to read, so the gap names the last one announced.
    let revision = state
        .active_show
        .sync_watermark(before.show_id)
        .unwrap_or_default();
    publish_gap_at(
        state,
        before.show_id,
        revision,
        ShowSyncGapReason::ShowReplaced,
    );
}

/// Announces a gap for whichever show is active, after the active show was replaced.
pub(super) fn publish_active_show_replaced(state: &AppState) {
    if let Some(entry) = state.active_show.current() {
        publish_gap(state, entry.id, ShowSyncGapReason::ShowReplaced);
    }
}

fn has_subscriber(state: &AppState) -> bool {
    state
        .events
        .has_subscriber_for(light_application::EventTopic::ShowSync)
}

fn publish_gap_at(state: &AppState, show_id: ShowId, revision: u64, reason: ShowSyncGapReason) {
    if !has_subscriber(state) {
        return;
    }
    state
        .events
        .publish(EventDraft::show_sync_published(ShowSyncPublication::Gap(
            ShowSyncGapChange {
                show_id,
                show_revision: revision,
                reason,
            },
        )));
}
