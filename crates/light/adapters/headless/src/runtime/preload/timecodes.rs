//! Ordered external transport steps in the same isolated Playback candidate.
use super::*;
use light_application::timeline::PreparedTimecodeBatch;
use light_playback::{
    PlaybackRuntimeEffect, PlaybackTarget, TimecodeId, TimecodeTransportAction,
    TimecodeTransportState,
};

pub(in crate::runtime) enum PreloadResources {
    Ready(Box<PreloadResourcePlan>),
    Refused(String),
}
pub(in crate::runtime) struct PreloadResourcePlan {
    pending: Vec<light_programmer::PreloadPlaybackAction>,
    revisions: (u64, u64, u64),
    snapshot: Arc<EngineSnapshot>,
    show_id: light_core::ShowId,
    show_revision: u64,
    fallback: u64,
    resources:
        std::collections::HashMap<TimecodeId, (light_playback::TimecodeDefinition, AudioMetadata)>,
    audio: Option<light_application::timeline::PreparedTimecodeAudioResources>,
}

fn revisions(state: &AppState) -> (u64, u64, u64) {
    (
        state.programming.capture_mode_revision(),
        state.programming.preload_values_revision(),
        state.programming.preload_playback_queue_revision(),
    )
}
fn target_ids(
    snapshot: &EngineSnapshot,
    commands: &[PlaybackBatchCommand],
) -> std::collections::BTreeSet<TimecodeId> {
    let mut ids = std::collections::BTreeSet::new();
    for command in commands {
        if let Some(PlaybackTarget::Timecode { timecode_id }) =
            definition(snapshot, command.address).map(|value| &value.target)
        {
            ids.insert(*timecode_id);
        }
        ids.extend(crate::runtime::playback_solo::timecode_peer_ids(
            snapshot,
            command.address,
        ));
    }
    ids
}
/// Caller has authorized the action, but holds no activation, Programmer, Playback or transport gate.
pub(in crate::runtime) fn preflight(
    state: &AppState,
    session: &Session,
) -> Result<Option<PreloadResources>, String> {
    let stamp = revisions(state);
    let pending = state
        .programming
        .preload_playback_actions(session.id)
        .ok_or("programmer does not exist")?;
    let at = state.programming.clock().now();
    let commands = CapturedPreloadPlaybackContext::new(session.desk.id).commands(&pending, at)?;
    let snapshot = state.output.snapshot();
    if target_ids(&snapshot, &commands).is_empty() {
        return Ok(None);
    }
    preparation::validate_playback_definitions(&pending, &snapshot)?;
    let fallback = state.installation.configuration().programmer_fade_millis;
    let show = state
        .active_show
        .current()
        .clone()
        .ok_or("no show is open")?;
    let store = ActiveShowRepository::open(&show.path).map_err(|error| error.to_string())?;
    let show_revision = store
        .portable_revision()
        .map_err(|error| error.to_string())?;
    let mut plan = OrderedTransportPlan {
        state,
        store,
        snapshot: snapshot.clone(),
        timing: state.output.engine().cue_timing_masters(),
        fallback,
        resources: std::collections::HashMap::new(),
        allow_new_resources: true,
    };
    let mut candidate = state
        .timecodes
        .prepare_detached_transport_batch(state.timecodes.capture_transport_micros());
    let discovered = plan.execute(&candidate, &commands, at)?;
    if !discovered.matches_snapshot(&snapshot) {
        return Err("compiled show changed during Timecode resource discovery".into());
    }
    candidate.preflight_audio()?;
    let audio = candidate.take_audio_resources();
    if revisions(state) != stamp
        || state
            .programming
            .preload_playback_actions(session.id)
            .as_ref()
            != Some(&pending)
    {
        return Err("Preload changed during Timecode resource preflight".into());
    }
    if plan
        .store
        .portable_revision()
        .map_err(|error| error.to_string())?
        != show_revision
    {
        return Err("active Show changed during Timecode resource preflight".into());
    }
    Ok(Some(PreloadResources::Ready(Box::new(
        PreloadResourcePlan {
            pending,
            revisions: stamp,
            snapshot,
            show_id: show.id,
            show_revision: show_revision.value(),
            fallback,
            resources: plan.resources,
            audio,
        },
    ))))
}

pub(super) fn prepare<'a>(
    state: &'a AppState,
    session: &Session,
    commands: &[PlaybackBatchCommand],
    at: chrono::DateTime<chrono::Utc>,
    _transport_micros: u64,
    fallback: u64,
    resources: Option<PreloadResources>,
) -> Result<
    (
        light_engine::PreparedPlaybackBatch,
        Option<PreparedTimecodeBatch<'a>>,
        chrono::DateTime<chrono::Utc>,
        u64,
    ),
    String,
> {
    let snapshot = state.output.snapshot();
    if target_ids(&snapshot, commands).is_empty() && resources.is_none() {
        return state
            .output
            .prepare_playback_batch(commands, at, fallback)
            .map(|prepared| (prepared, None, at, fallback));
    }
    let resources = match resources {
        Some(PreloadResources::Ready(resources)) => *resources,
        Some(PreloadResources::Refused(error)) => return Err(error),
        None => {
            return Err(
                "Timecode Preload resources must be prepared before acquiring commit gates".into(),
            );
        }
    };
    if revisions(state) != resources.revisions
        || state
            .programming
            .preload_playback_actions(session.id)
            .as_ref()
            != Some(&resources.pending)
    {
        return Err("Preload changed during Timecode resource preflight".into());
    }
    if !Arc::ptr_eq(&snapshot, &resources.snapshot) {
        return Err("compiled show changed during Timecode resource preflight".into());
    }
    let show = state
        .active_show
        .current()
        .clone()
        .ok_or("no show is open")?;
    if show.id != resources.show_id {
        return Err("active Show changed during Timecode resource preflight".into());
    }
    let store = ActiveShowRepository::open(&show.path).map_err(|error| error.to_string())?;
    if store
        .portable_revision()
        .map_err(|error| error.to_string())?
        .value()
        != resources.show_revision
    {
        return Err("active Show changed during Timecode resource preflight".into());
    }
    // Build from current Live only after preflight, never install the discovery candidate.
    let committed_at = state.programming.clock().now();
    let mut candidate = state
        .timecodes
        .prepare_transport_batch(state.timecodes.capture_transport_micros());
    let mut plan = OrderedTransportPlan {
        state,
        store,
        snapshot,
        timing: state.output.engine().cue_timing_masters(),
        fallback: resources.fallback,
        resources: resources.resources,
        allow_new_resources: false,
    };
    let prepared = plan.execute(&candidate, commands, committed_at)?;
    if !prepared.matches_snapshot(&plan.snapshot) {
        return Err("compiled show changed during Timecode Preload replay".into());
    }
    candidate.use_audio_resources(resources.audio)?;
    Ok((prepared, Some(candidate), committed_at, resources.fallback))
}

type AudioMetadata = Option<(
    light_application::AssetReference,
    light_application::WavMetadata,
)>;
struct OrderedTransportPlan<'a> {
    state: &'a AppState,
    store: ActiveShowRepository,
    snapshot: Arc<EngineSnapshot>,
    timing: (u64, u64),
    fallback: u64,
    resources:
        std::collections::HashMap<TimecodeId, (light_playback::TimecodeDefinition, AudioMetadata)>,
    allow_new_resources: bool,
}
impl OrderedTransportPlan<'_> {
    fn execute(
        &mut self,
        candidate: &PreparedTimecodeBatch<'_>,
        commands: &[PlaybackBatchCommand],
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<light_engine::PreparedPlaybackBatch, String> {
        let state = self.state;
        let snapshot = self.snapshot.clone();
        let timing = self.timing;
        let fallback = self.fallback;
        let mut validated = std::collections::HashSet::new();
        let commands = commands
            .iter()
            .cloned()
            .map(|mut command| {
                if let Some(origin) = command.activation_origin.as_mut() {
                    origin.at = at;
                }
                command
            })
            .collect::<Vec<_>>();
        let prepared = state.output.prepare_preload_batch_with_steps(
            &commands,
            at,
            fallback,
            |playback, command, regions| {
                let mut outcome =
                    match definition(&snapshot, command.address).map(|value| &value.target) {
                        Some(PlaybackTarget::Timecode { timecode_id }) => {
                            self.ensure_current(
                                candidate,
                                &mut validated,
                                *timecode_id,
                                command.action != PlaybackBatchAction::Off,
                            )?;
                            transport_step(candidate, playback, command, *timecode_id, regions)?
                        }
                        _ => light_engine::Engine::apply_prepared_playback_step(
                            playback, command, at, fallback, regions,
                        )?,
                    };
                if outcome.permanent_started {
                    for peer in
                        crate::runtime::playback_solo::timecode_peer_ids(&snapshot, command.address)
                    {
                        self.ensure_current(candidate, &mut validated, peer, false)?;
                        if candidate.snapshot(peer).is_ok_and(|runtime| {
                            runtime.transport != TimecodeTransportState::Stopped
                        }) {
                            candidate
                                .handle(peer, TimecodeTransportAction::Stop)
                                .map_err(|error| error.message)?;
                            outcome.released_playbacks.extend(aliases(&snapshot, peer));
                        }
                    }
                }
                candidate.reconcile(playback, &snapshot.cue_lists, timing)?;
                outcome.released_playbacks.sort_unstable();
                outcome.released_playbacks.dedup();
                Ok(outcome)
            },
        )?;
        Ok(prepared)
    }
}

fn definition(
    snapshot: &EngineSnapshot,
    address: light_playback::PlaybackAddress,
) -> Option<&light_playback::PlaybackDefinition> {
    snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == address.page())
        .and_then(|page| page.assignments.get(&address.position()))
}

fn aliases(snapshot: &EngineSnapshot, id: TimecodeId) -> Vec<light_playback::PlaybackAddress> {
    snapshot.playback_pages.iter().flat_map(|page|page.assignments.iter().filter_map(move |(position, definition)| {
        matches!(definition.target,PlaybackTarget::Timecode{timecode_id} if timecode_id==id).then(||light_playback::PlaybackAddress::new(page.number,*position).ok()).flatten()
    })).collect()
}

fn transport_step(
    candidate: &PreparedTimecodeBatch<'_>,
    playback: &mut light_playback::PlaybackEngine,
    command: &PlaybackBatchCommand,
    id: TimecodeId,
    regions: &[light_playback::PlaybackSoloRegion],
) -> Result<PlaybackBatchOutcome, String> {
    let before = match candidate.snapshot(id) {
        Ok(before) => before,
        Err(_) if command.action == PlaybackBatchAction::Off => {
            return Ok(PlaybackBatchOutcome {
                address: command.address,
                permanent_started: false,
                released_playbacks: Vec::new(),
                addressed_effect: PlaybackRuntimeEffect::None,
                effect: PlaybackRuntimeEffect::None,
            });
        }
        Err(error) => return Err(error.message),
    };
    let action = match command.action {
        PlaybackBatchAction::Go => TimecodeTransportAction::Go,
        PlaybackBatchAction::Off => TimecodeTransportAction::Stop,
        PlaybackBatchAction::TogglePause => TimecodeTransportAction::Pause,
        _ => return Err("action is incompatible with a Timecode playback".into()),
    };
    let result = candidate
        .handle(id, action)
        .map_err(|error| error.message)?;
    let permanent_started = before.transport == TimecodeTransportState::Stopped
        && result.snapshot.transport != TimecodeTransportState::Stopped;
    let mut released_playbacks = Vec::new();
    let effect = if result.changed {
        PlaybackRuntimeEffect::Durable
    } else {
        PlaybackRuntimeEffect::None
    };
    let mut aggregate = effect;
    if permanent_started {
        let before: Vec<_> = regions
            .iter()
            .filter(|region| region.members.contains(&command.address))
            .flat_map(|region| region.members.iter().copied())
            .filter(|address| playback.permanent_enabled(*address))
            .collect();
        aggregate = aggregate.combine(light_engine::Engine::enforce_prepared_target_solo(
            playback,
            command.address,
            regions,
        )?);
        released_playbacks.extend(
            before
                .into_iter()
                .filter(|address| !playback.permanent_enabled(*address)),
        );
    }
    Ok(PlaybackBatchOutcome {
        address: command.address,
        permanent_started,
        released_playbacks,
        addressed_effect: effect,
        effect: aggregate,
    })
}

#[derive(Default)]
struct AudioBytes(Vec<u8>);
impl light_application::AssetChunkSink for AudioBytes {
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), light_application::AssetError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}
fn audio_metadata(
    state: &AppState,
    definition: &light_playback::TimecodeDefinition,
) -> Result<
    Option<(
        light_application::AssetReference,
        light_application::WavMetadata,
    )>,
    String,
> {
    let Some(audio) = &definition.audio else {
        return Ok(None);
    };
    let asset = light_application::AssetReference {
        id: light_application::AssetId(audio.asset_id),
        revision: light_application::AssetRevision(audio.asset_revision),
    };
    let mut bytes = AudioBytes::default();
    state
        .managed_assets
        .stream(asset, &mut bytes)
        .map_err(|error| error.message)?;
    let metadata =
        light_application::parse_wav_metadata(&bytes.0).map_err(|error| error.message)?;
    Ok(Some((asset, metadata)))
}

impl OrderedTransportPlan<'_> {
    fn ensure_current(
        &mut self,
        candidate: &PreparedTimecodeBatch<'_>,
        validated: &mut std::collections::HashSet<TimecodeId>,
        id: TimecodeId,
        stage_if_missing: bool,
    ) -> Result<(), String> {
        if validated.contains(&id) && candidate.snapshot(id).is_ok() {
            return Ok(());
        }
        if !self.resources.contains_key(&id) {
            if !self.allow_new_resources {
                return Err("Timecode resource dependency changed after preflight".into());
            }
            let object = self
                .store
                .object_with_portable_revision("timecode", &id.0.to_string())
                .map_err(|error| error.to_string())?
                .1
                .ok_or("Timecode does not exist in the current show")?;
            let definition = serde_json::from_value(object.body)
                .map_err(|error| format!("stored Timecode is invalid: {error}"))?;
            let audio = if stage_if_missing && candidate.snapshot(id).is_err() {
                audio_metadata(self.state, &definition)?
            } else {
                None
            };
            self.resources.insert(id, (definition, audio));
        }
        let (definition, audio) = self.resources.get_mut(&id).expect("preflight resource");
        if candidate.snapshot(id).is_err() && !stage_if_missing {
            validated.insert(id);
            return Ok(());
        }
        if candidate.snapshot(id).is_err() && definition.audio.is_some() && audio.is_none() {
            if !self.allow_new_resources {
                return Err("Timecode audio resource dependency changed after preflight".into());
            }
            *audio = audio_metadata(self.state, definition)?;
        }
        if candidate.snapshot(id).is_err() {
            candidate
                .stage_definition(definition.clone(), *audio)
                .map_err(|error| error.message)?;
        } else if !candidate.installed_definition_matches(definition) {
            return Err("Timecode definition changed while Preload was captured".into());
        }
        validated.insert(id);
        Ok(())
    }
}
