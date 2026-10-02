//! Admission of the desk's accepted Pending (Preload) frames by their published identity.
//!
//! A server that reads Preload from its accepted Pending publication stamps the Preload lane
//! with that publication's own identity: `preload_status.episode` names the Pending episode and
//! `preload.frame.sequence` is the episode's attempt ticket. Tickets restart with every
//! episode, so they are ordered only within one episode. This mirrors the Live admission:
//!
//! - within one episode, an older ticket is a replay and never replaces the newer Preload
//!   picture; the very frame already applied is a duplicate;
//! - one ticket naming two different frames, a published state without an episode or a stamped
//!   lane, or a passive state carrying a lane, is incoherent;
//! - a new episode resets the order (the previous episode is retired, so a late snapshot from it
//!   is a replay too);
//! - every connection and scene boundary forgets all of it, like Live.
//!
//! A passive state (nothing published) carries no lane and proves nothing; it neither moves the
//! order nor is filled from Live by the provider. Preload without a Pending status (legacy, or a
//! stamp without its episode) stays latest-wins. Rejection is passive: the snapshot is dropped
//! and the presented picture stays.
use light_wire::v2::output_control::{OutputDmxSnapshot, OutputFrameIdentity, OutputPreloadState};
use std::collections::VecDeque;
use uuid::Uuid;

/// Retired episodes remembered so that a late snapshot of an earlier episode is still a replay.
const RETIRED_EPISODES: usize = 8;

/// Where one snapshot's Preload identity stands against what this epoch already applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PreloadAdmission {
    /// No Pending stamp: a legacy Preload projection, or no Preload lane at all.
    Unstamped,
    /// A passive Pending state with no lane: nothing published to present.
    Passive,
    /// The first frame of the epoch, or a newer ticket of the current episode.
    Newer,
    /// The first frame of another, never-retired episode.
    Reset,
    /// The very Pending frame already applied.
    Same,
    /// An older ticket of the current episode, or any frame of a retired episode.
    Older,
    /// The Pending stamps contradict each other or the applied frame.
    Incoherent,
}

/// The Pending identity a snapshot claims for its Preload lane.
pub(super) fn preload_identity(
    output: &OutputDmxSnapshot,
) -> Result<Option<(Uuid, &OutputFrameIdentity)>, ()> {
    let frame = output.preload.as_ref().and_then(|lane| lane.frame.as_ref());
    let Some(status) = output.preload_status.as_ref() else {
        // No Pending status: not a Pending publication. A stamp without its episode cannot be
        // ordered (tickets restart per episode), so it stays latest-wins like any legacy lane.
        return Ok(None);
    };
    match (status.state, status.episode, frame) {
        (OutputPreloadState::Published, Some(episode), Some(frame)) => Ok(Some((episode, frame))),
        (OutputPreloadState::Published, ..) => Err(()),
        // A passive state never carries a lane.
        (_, _, _) if output.preload.is_some() => Err(()),
        _ => Ok(None),
    }
}

/// What the provider has proven about the desk's stamped Pending frames in this epoch.
#[derive(Debug, Default)]
pub(super) struct PreloadFrameAdmission {
    newest: Option<(Uuid, OutputFrameIdentity)>,
    retired: VecDeque<Uuid>,
    /// Older Pending tickets of the current episode, or frames of a retired episode.
    pub(super) stale: u64,
    /// Snapshots whose Pending stamps contradict each other or the applied frame.
    pub(super) incoherent: u64,
    /// New episodes admitted mid-epoch.
    pub(super) resets: u64,
}

impl PreloadFrameAdmission {
    pub(super) fn classify(&self, output: &OutputDmxSnapshot) -> PreloadAdmission {
        let (episode, frame) = match preload_identity(output) {
            Err(()) => return PreloadAdmission::Incoherent,
            Ok(Some(identity)) => identity,
            Ok(None) if output.preload_status.is_some() => return PreloadAdmission::Passive,
            Ok(None) => return PreloadAdmission::Unstamped,
        };
        if self.retired.contains(&episode) {
            return PreloadAdmission::Older;
        }
        let Some((newest_episode, newest)) = &self.newest else {
            return PreloadAdmission::Newer;
        };
        if *newest_episode != episode {
            return PreloadAdmission::Reset;
        }
        match frame.sequence.cmp(&newest.sequence) {
            std::cmp::Ordering::Less => PreloadAdmission::Older,
            std::cmp::Ordering::Greater => PreloadAdmission::Newer,
            std::cmp::Ordering::Equal if frame == newest => PreloadAdmission::Same,
            std::cmp::Ordering::Equal => PreloadAdmission::Incoherent,
        }
    }

    /// Count and drop a replayed or incoherent Pending frame; every other snapshot proceeds.
    pub(super) fn admits(&mut self, admission: PreloadAdmission) -> bool {
        match admission {
            PreloadAdmission::Older => self.stale += 1,
            PreloadAdmission::Incoherent => self.incoherent += 1,
            _ => return true,
        }
        false
    }

    /// Record a snapshot's Pending frame as applied. Only a newer ticket or a new episode moves
    /// the order; a new episode retires the previous one.
    pub(super) fn applied(&mut self, admission: PreloadAdmission, output: &OutputDmxSnapshot) {
        if !matches!(admission, PreloadAdmission::Newer | PreloadAdmission::Reset) {
            return;
        }
        let Ok(Some((episode, frame))) = preload_identity(output) else {
            return;
        };
        if admission == PreloadAdmission::Reset {
            self.resets += 1;
            if let Some((previous, _)) = self.newest.take() {
                if self.retired.len() == RETIRED_EPISODES {
                    self.retired.pop_front();
                }
                self.retired.push_back(previous);
            }
        }
        self.newest = Some((episode, frame.clone()));
    }

    /// Forget every proof. Called at each connection and scene boundary, never mid-epoch.
    pub(super) fn reset(&mut self) {
        self.newest = None;
        self.retired.clear();
    }
}
