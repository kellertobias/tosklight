//! Authenticated action-socket ownership of raw software Playback control sources.
//! All claim/close mutations run under the application's ordered Playback operation.
use super::{ApiError, AppState};
use light_application::{PlaybackOperation, PlaybackUnitOfWork};
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Default)]
pub(in crate::runtime) struct SoftwarePlaybackSources {
    state: Mutex<Ownership>,
}
#[derive(Default)]
struct Ownership {
    next_generation: u64,
    open: HashSet<u64>,
    // Retained after close: a delayed old connection must never reclaim this source.
    watermark: HashMap<(Uuid, String), u64>,
    owner: HashMap<(Uuid, String), u64>,
}
#[derive(Clone)]
pub(in crate::runtime) struct SoftwarePlaybackSocket {
    resource: Arc<SoftwarePlaybackSources>,
    desk_id: Uuid,
    generation: u64,
}
impl SoftwarePlaybackSources {
    pub(in crate::runtime) fn open(self: &Arc<Self>, desk_id: Uuid) -> SoftwarePlaybackSocket {
        let mut state = self.state.lock();
        state.next_generation = state
            .next_generation
            .checked_add(1)
            .expect("software socket generation exhausted");
        let generation = state.next_generation;
        state.open.insert(generation);
        SoftwarePlaybackSocket {
            resource: self.clone(),
            desk_id,
            generation,
        }
    }
}
impl SoftwarePlaybackSocket {
    fn claim(
        &self,
        desk_id: Uuid,
        source: &light_playback::PlaybackControlSource,
    ) -> Result<bool, String> {
        source.validate()?;
        if desk_id != self.desk_id {
            return Err("Software Playback socket belongs to another desk".into());
        }
        let key = (desk_id, source.surface_id.clone());
        let mut state = self.resource.state.lock();
        if !state.open.contains(&self.generation) {
            return Err("Software Playback socket is closed".into());
        }
        if state
            .watermark
            .get(&key)
            .is_some_and(|generation| *generation > self.generation)
        {
            return Err("Software Playback source belongs to a newer connection; reconnect and read current state".into());
        }
        let replaced = state
            .owner
            .get(&key)
            .is_some_and(|generation| *generation != self.generation);
        state.watermark.insert(key.clone(), self.generation);
        state.owner.insert(key, self.generation);
        Ok(replaced)
    }
    fn close(&self) -> Vec<String> {
        let mut state = self.resource.state.lock();
        if !state.open.remove(&self.generation) {
            return Vec::new();
        }
        let owned = state
            .owner
            .iter()
            .filter(|(_, generation)| **generation == self.generation)
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in &owned {
            state.owner.remove(key);
        }
        let minimum_open = state.open.iter().copied().min();
        let live_keys = state.owner.keys().cloned().collect::<HashSet<_>>();
        state.watermark.retain(|key, generation| {
            live_keys.contains(key) || minimum_open.is_some_and(|minimum| *generation > minimum)
        });
        owned.into_iter().map(|(_, source)| source).collect()
    }
}

/// Called by ServerPlaybackPorts while its application Playback ordering lock is held.
pub(in crate::runtime) fn claim_in_order(
    state: &AppState,
    socket: &SoftwarePlaybackSocket,
    desk_id: Uuid,
    source: &light_playback::PlaybackControlSource,
) -> Result<(), ApiError> {
    if socket
        .claim(desk_id, source)
        .map_err(ApiError::bad_request)?
    {
        let scoped = source
            .scoped_to_desk(desk_id)
            .map_err(ApiError::bad_request)?;
        for event in
            super::playback_control_disconnect::cancel_scoped_in_order(state, &scoped.surface_id)?
        {
            state.events.publish(event);
        }
    }
    Ok(())
}

pub(in crate::runtime) fn close_socket(
    state: &AppState,
    socket: &SoftwarePlaybackSocket,
) -> Result<(), ApiError> {
    state
        .playback
        .run_unit_of_work(CloseSocket { state, socket })
        .output
}
struct CloseSocket<'a> {
    state: &'a AppState,
    socket: &'a SoftwarePlaybackSocket,
}
impl PlaybackUnitOfWork for CloseSocket<'_> {
    type Output = Result<(), ApiError>;
    fn execute(self) -> PlaybackOperation<Self::Output> {
        let mut events = Vec::new();
        for surface_id in self.socket.close() {
            let source = light_playback::PlaybackControlSource {
                surface_id,
                control_id: 0,
            };
            let scoped = match source.scoped_to_desk(self.socket.desk_id) {
                Ok(source) => source,
                Err(error) => {
                    return PlaybackOperation::with_events(
                        Err(ApiError::bad_request(error)),
                        events,
                    );
                }
            };
            match super::playback_control_disconnect::cancel_scoped_in_order(
                self.state,
                &scoped.surface_id,
            ) {
                Ok(changes) => events.extend(changes),
                Err(error) => return PlaybackOperation::with_events(Err(error), events),
            }
        }
        PlaybackOperation::with_events(Ok(()), events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(surface: &str) -> light_playback::PlaybackControlSource {
        light_playback::PlaybackControlSource {
            surface_id: surface.into(),
            control_id: 1,
        }
    }
    #[test]
    fn shared_session_sibling_sources_close_independently() {
        let resource = Arc::new(SoftwarePlaybackSources::default());
        let desk = Uuid::new_v4();
        let main = resource.open(desk);
        let screen = resource.open(desk);
        assert!(!main.claim(desk, &source("main-instance")).unwrap());
        assert!(!screen.claim(desk, &source("screen-instance")).unwrap());
        assert_eq!(main.close(), vec!["main-instance"]);
        assert!(!screen.claim(desk, &source("screen-instance")).unwrap());
        assert_eq!(screen.close(), vec!["screen-instance"]);
    }
    #[test]
    fn reconnect_takeover_survives_old_close_and_keeps_watermark_after_new_close() {
        let resource = Arc::new(SoftwarePlaybackSources::default());
        let desk = Uuid::new_v4();
        let old = resource.open(desk);
        let new = resource.open(desk);
        let control = source("instance");
        assert!(!old.claim(desk, &control).unwrap());
        assert!(new.claim(desk, &control).unwrap());
        assert!(old.claim(desk, &control).is_err());
        assert!(old.close().is_empty());
        assert_eq!(new.close(), vec!["instance"]);
        assert!(old.claim(desk, &control).is_err());
        let next = resource.open(desk);
        assert!(!next.claim(desk, &control).unwrap());
    }
    #[test]
    fn open_old_socket_cannot_reclaim_after_new_socket_closes() {
        let resource = Arc::new(SoftwarePlaybackSources::default());
        let desk = Uuid::new_v4();
        let old = resource.open(desk);
        let new = resource.open(desk);
        let control = source("instance");
        new.claim(desk, &control).unwrap();
        new.close();
        assert!(old.claim(desk, &control).is_err());
    }
    #[test]
    fn different_desks_can_use_equal_raw_source_names() {
        let resource = Arc::new(SoftwarePlaybackSources::default());
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let left = resource.open(a);
        let right = resource.open(b);
        assert!(!left.claim(a, &source("main")).unwrap());
        assert!(!right.claim(b, &source("main")).unwrap());
        assert!(left.claim(b, &source("main")).is_err());
        assert_eq!(left.close(), vec!["main"]);
        assert!(!right.claim(b, &source("main")).unwrap());
    }
    #[test]
    fn observer_close_and_repeated_close_own_no_sources() {
        let resource = Arc::new(SoftwarePlaybackSources::default());
        let socket = resource.open(Uuid::new_v4());
        assert!(socket.close().is_empty());
        assert!(socket.close().is_empty());
        assert!(socket.claim(socket.desk_id, &source("late")).is_err());
    }
    #[test]
    fn retiring_old_socket_prunes_safe_tombstones_but_preserves_live_owner() {
        let resource = Arc::new(SoftwarePlaybackSources::default());
        let desk = Uuid::new_v4();
        let old = resource.open(desk);
        let newer = resource.open(desk);
        newer.claim(desk, &source("live")).unwrap();
        newer.claim(desk, &source("retired")).unwrap();
        newer.close();
        assert_eq!(
            resource.state.lock().watermark.len(),
            2,
            "old open socket can still be stale"
        );
        let live = resource.open(desk);
        live.claim(desk, &source("live")).unwrap();
        old.close();
        let state = resource.state.lock();
        assert_eq!(state.watermark.len(), 1);
        assert!(state.owner.contains_key(&(desk, "live".into())));
        drop(state);
        assert!(!live.claim(desk, &source("live")).unwrap());
        live.close();
        assert!(resource.state.lock().watermark.is_empty());
    }
    #[test]
    fn idle_observer_keeps_only_tombstones_that_can_reject_it() {
        let resource = Arc::new(SoftwarePlaybackSources::default());
        let desk = Uuid::new_v4();
        let observer = resource.open(desk);
        let active = resource.open(desk);
        active.claim(desk, &source("instance")).unwrap();
        active.close();
        assert!(observer.claim(desk, &source("instance")).is_err());
        assert_eq!(resource.state.lock().watermark.len(), 1);
        observer.close();
        assert!(resource.state.lock().watermark.is_empty());
    }
}

#[cfg(test)]
#[path = "software_playback_sources/runtime_tests.rs"]
mod runtime_tests;
