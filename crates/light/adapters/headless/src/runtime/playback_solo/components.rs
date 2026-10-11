//! Component boundary shared by operator dispatch and the ordered output scheduler.
use super::{ApiError, SoloTarget, aliases, target, timecode_peer_ids};
use light_application::timeline::{TimecodeRuntimeOutcome, TimecodeRuntimeService};
use light_engine::{Engine, EnginePlaybackCommand, EnginePlaybackOutcome};
use light_playback::{
    PlaybackAddress, TimecodeId, TimecodeTransportAction, TimecodeTransportState,
};

pub(in crate::runtime) struct SoloCoordinator<'a> {
    pub engine: &'a Engine,
    pub timecodes: &'a TimecodeRuntimeService,
}

pub(in crate::runtime) struct SoloComponentOutcome<T> {
    pub output: T,
    pub affected_addresses: Vec<PlaybackAddress>,
    pub stopped_timecodes: Vec<TimecodeId>,
}

impl SoloCoordinator<'_> {
    pub fn related_addresses(&self, incoming: PlaybackAddress) -> Vec<PlaybackAddress> {
        let snapshot = self.engine.snapshot();
        let mut addresses = super::solo_members(&snapshot, incoming);
        let targets = addresses
            .iter()
            .chain(std::iter::once(&incoming))
            .filter_map(|address| {
                snapshot
                    .playback_pages
                    .iter()
                    .find(|page| page.number == address.page())
                    .and_then(|page| page.assignments.get(&address.position()))
                    .and_then(|definition| target(&definition.target))
            })
            .collect::<Vec<_>>();
        for target in targets {
            addresses.extend(aliases(&snapshot, target));
        }
        addresses.sort_unstable();
        addresses.dedup();
        addresses
    }

    /// Validate all external authorities before the incoming activation can mutate.
    pub fn prepare_peer_releases(
        &self,
        incoming: PlaybackAddress,
    ) -> Result<Vec<TimecodeId>, ApiError> {
        timecode_peer_ids(&self.engine.snapshot(), incoming)
            .into_iter()
            .filter_map(|id| match self.timecodes.snapshot(id) {
                Ok(runtime) if runtime.transport != TimecodeTransportState::Stopped => Some(Ok(id)),
                Ok(_) => None,
                Err(error) => Some(Err(ApiError::internal(format!(
                    "Solo Timecode peer unavailable: {}",
                    error.message
                )))),
            })
            .collect()
    }

    /// Never reconcile clips here: the timeline executor may hold its ownership lock.
    /// A failed Stop leaves actual earlier changes accepted and is returned explicitly.
    pub fn stop_peer_releases(&self, peers: &[TimecodeId]) -> Result<(), ApiError> {
        for id in peers {
            self.timecodes
                .handle(*id, TimecodeTransportAction::Stop)
                .map_err(|error| {
                    ApiError::internal(format!(
                        "Solo could not stop Timecode {}: {}",
                        id.0, error.message
                    ))
                })?;
        }
        Ok(())
    }

    pub fn command_target(&self, command: &EnginePlaybackCommand) -> Option<SoloTarget> {
        match command {
            EnginePlaybackCommand::CueList { .. } => None,
            EnginePlaybackCommand::Playback { address, .. } => self
                .engine
                .snapshot()
                .playback_pages
                .iter()
                .find(|page| page.number == address.page())
                .and_then(|page| page.assignments.get(&address.position()))
                .and_then(|definition| target(&definition.target)),
            _ => None,
        }
    }

    pub fn enabled(&self, incoming: PlaybackAddress) -> bool {
        let snapshot = self.engine.snapshot();
        let definition = snapshot
            .playback_pages
            .iter()
            .find(|page| page.number == incoming.page())
            .and_then(|page| page.assignments.get(&incoming.position()));
        match definition.and_then(|definition| target(&definition.target)) {
            Some(SoloTarget::Timecode(id)) => self
                .timecodes
                .snapshot(id)
                .is_ok_and(|runtime| runtime.transport != TimecodeTransportState::Stopped),
            _ => self.engine.permanent_playback_enabled(incoming),
        }
    }

    /// Caller owns the shared Playback ordering boundary. No events or nested UOW are emitted.
    pub fn execute(
        &self,
        command: EnginePlaybackCommand,
    ) -> Result<SoloComponentOutcome<EnginePlaybackOutcome>, ApiError> {
        let incoming = match &command {
            EnginePlaybackCommand::Playback { address, .. } => Some(*address),
            _ => None,
        };
        let before = incoming.is_some_and(|incoming| self.enabled(incoming));
        let peers = if !before && may_activate(&command) {
            incoming
                .map(|incoming| self.prepare_peer_releases(incoming))
                .transpose()?
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let output = self
            .engine
            .execute_playback(command)
            .map_err(ApiError::bad_request)?;
        let stopped_timecodes =
            if !before && incoming.is_some_and(|incoming| self.enabled(incoming)) {
                self.stop_peer_releases(&peers)?;
                peers
            } else {
                Vec::new()
            };
        Ok(SoloComponentOutcome {
            output,
            affected_addresses: incoming
                .map(|incoming| self.related_addresses(incoming))
                .unwrap_or_default(),
            stopped_timecodes,
        })
    }

    /// A direct transport action is not a Playback activation and never arbitrates Solo.
    pub fn transport(
        &self,
        id: TimecodeId,
        action: TimecodeTransportAction,
    ) -> Result<SoloComponentOutcome<TimecodeRuntimeOutcome>, ApiError> {
        let output = self
            .timecodes
            .handle(id, action)
            .map_err(|error| ApiError::bad_request(error.message))?;
        Ok(SoloComponentOutcome {
            output,
            affected_addresses: aliases(&self.engine.snapshot(), SoloTarget::Timecode(id)),
            stopped_timecodes: Vec::new(),
        })
    }

    /// Only an explicitly addressed member Playback gives a Timecode transport Solo semantics.
    pub fn transport_playback(
        &self,
        address: PlaybackAddress,
        action: TimecodeTransportAction,
    ) -> Result<SoloComponentOutcome<TimecodeRuntimeOutcome>, ApiError> {
        let snapshot = self.engine.snapshot();
        let definition = snapshot
            .playback_pages
            .iter()
            .find(|page| page.number == address.page())
            .and_then(|page| page.assignments.get(&address.position()));
        let Some(SoloTarget::Timecode(id)) =
            definition.and_then(|definition| target(&definition.target))
        else {
            return Err(ApiError::bad_request("Playback does not target a Timecode"));
        };
        let before = self
            .timecodes
            .snapshot(id)
            .map_err(|error| ApiError::bad_request(error.message))?;
        let peers = if before.transport == TimecodeTransportState::Stopped
            && matches!(action, TimecodeTransportAction::Go)
        {
            self.prepare_peer_releases(address)?
        } else {
            Vec::new()
        };
        let output = self
            .timecodes
            .handle(id, action)
            .map_err(|error| ApiError::bad_request(error.message))?;
        let stopped_timecodes = if before.transport == TimecodeTransportState::Stopped
            && output.snapshot.transport != TimecodeTransportState::Stopped
        {
            self.stop_peer_releases(&peers)?;
            self.engine
                .execute_playback(EnginePlaybackCommand::EnforceSoloForTarget { address })
                .map_err(ApiError::internal)?;
            peers
        } else {
            Vec::new()
        };
        Ok(SoloComponentOutcome {
            output,
            affected_addresses: self.related_addresses(address),
            stopped_timecodes,
        })
    }
}

fn may_activate(command: &EnginePlaybackCommand) -> bool {
    match command {
        EnginePlaybackCommand::CueList { .. } => false,
        EnginePlaybackCommand::Playback { action, .. } => !matches!(
            action,
            light_engine::PlaybackAction::Off
                | light_engine::PlaybackAction::Release
                | light_engine::PlaybackAction::SetFlash(_)
                | light_engine::PlaybackAction::Pause
                | light_engine::PlaybackAction::TogglePause
        ),
        _ => false,
    }
}
