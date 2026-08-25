use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use light_extensions_contract::{
    CanonicalControlIntent, ControlInput, FeedbackControlState, FeedbackDelta, FeedbackSnapshot,
    FeedbackValue, HighlightControlAction, LampState, ModifierKey, NavigationAction,
    PlaybackControl, ProgrammerKey, SpeedGroupControl,
};
use light_extensions_host::{
    BoundControlInput, ExtensionApplicationPorts, ExtensionManager, HostControlContext, PortError,
    TelemetryEnvelope, TimecodeEnvelope,
};
use light_wire::v2::extensions::{
    ExtensionDiagnostic, ExtensionInstanceDiagnosticSnapshot, ExtensionInstanceSnapshot,
    ExtensionPackageSnapshot, ExtensionRescanRequest, ExtensionRuntimeSnapshot,
};
use parking_lot::{Mutex, RwLock};

use super::{
    ApiError, AppState, ControlDesk, OSC_DESK_PATH, Session, application_millis, authenticate,
    command_http, copy_speed_group_runtime_to_configuration, emit, output_runtime_service,
    persist_server_configuration, playback_service, refresh_speed_group_engine,
};
use crate::tolerant_json::TolerantJson;

#[derive(Default)]
struct HeadlessExtensionPorts {
    feedback_revision: AtomicU64,
    telemetry: RwLock<BTreeMap<(String, String), light_extensions_contract::TelemetrySample>>,
    state: RwLock<Option<AppState>>,
}

impl ExtensionApplicationPorts for HeadlessExtensionPorts {
    fn feedback_snapshot(
        &self,
        context: &HostControlContext,
        bindings: &BTreeMap<String, CanonicalControlIntent>,
    ) -> FeedbackSnapshot {
        let state = self.state.read();
        let output = state
            .as_ref()
            .map(|state| state.output.control_projection());
        let desk = state
            .as_ref()
            .and_then(|state| extension_desk(state, context));
        let session = state.as_ref().and_then(|state| {
            desk.as_ref().and_then(|desk| {
                state
                    .sessions
                    .sessions()
                    .into_iter()
                    .find(|session| session.connected && session.desk.id == desk.id)
            })
        });
        let highlight = state.as_ref().and_then(|state| {
            session.as_ref().and_then(|session| {
                super::execute_highlight(
                    state,
                    session,
                    light_application::HighlightCommand::status(),
                    light_application::ActionSource::Extension,
                    false,
                )
                .ok()
            })
        });
        FeedbackSnapshot {
            context: feedback_context(state.as_ref(), context),
            revision: self.feedback_revision.load(Ordering::Acquire),
            controls: bindings
                .iter()
                .map(|(id, intent)| {
                    let mut feedback = FeedbackControlState {
                        available: state.is_some(),
                        enabled: state.is_some(),
                        text: Some(control_label(intent)),
                        ..FeedbackControlState::default()
                    };
                    match intent {
                        CanonicalControlIntent::GrandMaster => {
                            feedback.value = output.as_ref().map(|value| value.grand_master);
                        }
                        CanonicalControlIntent::Blackout => {
                            feedback.selected = output.as_ref().is_some_and(|value| value.blackout);
                            feedback.lamp = if feedback.selected {
                                LampState::On
                            } else {
                                LampState::Off
                            };
                        }
                        CanonicalControlIntent::Highlight { action } => {
                            apply_highlight_feedback(&mut feedback, highlight.as_ref(), *action);
                        }
                        CanonicalControlIntent::PlaybackCurrent { slot, .. } => {
                            apply_playback_feedback(
                                &mut feedback,
                                state.as_ref(),
                                desk.as_ref(),
                                None,
                                *slot,
                            );
                        }
                        CanonicalControlIntent::PlaybackExplicit { page, slot, .. } => {
                            apply_playback_feedback(
                                &mut feedback,
                                state.as_ref(),
                                desk.as_ref(),
                                Some(*page),
                                *slot,
                            );
                        }
                        CanonicalControlIntent::SpeedGroup { group, .. } => {
                            let snapshot = state.as_ref().and_then(|state| {
                                "ABCDE".find(group.to_ascii_uppercase()).map(|index| {
                                    state
                                        .output
                                        .speed_group_snapshot(index, application_millis(state))
                                })
                            });
                            feedback.available = snapshot.is_some();
                            feedback.enabled = snapshot.is_some();
                            if let Some(snapshot) = snapshot {
                                feedback.value = Some(snapshot.speed_master_scale as f32);
                                feedback.selected = snapshot.phase_advancing;
                                feedback.text = Some(format!(
                                    "Speed Group {} · {:.1} BPM",
                                    group.to_ascii_uppercase(),
                                    snapshot.effective_bpm
                                ));
                            }
                        }
                        CanonicalControlIntent::Encoder { .. } => {
                            feedback.ring_style =
                                Some(light_extensions_contract::EncoderRingStyle::Dot);
                        }
                        _ => {}
                    }
                    (id.clone(), FeedbackValue::Control(feedback))
                })
                .collect(),
        }
    }

    fn apply_control(
        &self,
        context: &HostControlContext,
        input: BoundControlInput,
    ) -> Result<Option<FeedbackDelta>, PortError> {
        let state = self
            .state
            .read()
            .clone()
            .ok_or_else(|| PortError::new("extension application authority is not ready"))?;
        apply_bound_control(&state, context, &input)?;
        let base_revision = self.feedback_revision.fetch_add(1, Ordering::AcqRel);
        let feedback = self.feedback_snapshot(
            context,
            &BTreeMap::from([(
                control_id(&input.input.control).to_owned(),
                input.intent.clone(),
            )]),
        );
        Ok(Some(FeedbackDelta {
            context: feedback.context,
            base_revision,
            revision: base_revision + 1,
            changes: feedback
                .controls
                .into_iter()
                .map(
                    |(control_id, value)| light_extensions_contract::FeedbackChange {
                        control_id,
                        value: Some(value),
                    },
                )
                .collect(),
        }))
    }

    fn publish_telemetry(&self, telemetry: TelemetryEnvelope) -> Result<(), PortError> {
        self.telemetry.write().insert(
            (
                telemetry.extension_instance_id,
                telemetry.sample.channel_id.clone(),
            ),
            telemetry.sample,
        );
        Ok(())
    }

    fn publish_timecode(&self, timecode: TimecodeEnvelope) -> Result<(), PortError> {
        let state = self
            .state
            .read()
            .clone()
            .ok_or_else(|| PortError::new("timecode application port is not attached"))?;
        ingest_extension_timecode(&state, &timecode)
    }
}

pub(super) fn ingest_extension_timecode(
    state: &AppState,
    timecode: &TimecodeEnvelope,
) -> Result<(), PortError> {
    let rate = match timecode.sample.rate {
        light_extensions_contract::TimecodeRate::Fps24 => light_control::FrameRate::Fps24,
        light_extensions_contract::TimecodeRate::Fps25 => light_control::FrameRate::Fps25,
        light_extensions_contract::TimecodeRate::Fps2997 => {
            if !timecode.sample.drop_frame {
                return Err(PortError::new(
                    "29.97 extension timecode must use drop-frame numbering",
                ));
            }
            light_control::FrameRate::Fps2997Drop
        }
        light_extensions_contract::TimecodeRate::Fps30 => light_control::FrameRate::Fps30,
    };
    let received_at = i64::try_from(timecode.received_at_micros)
        .ok()
        .and_then(chrono::DateTime::from_timestamp_micros)
        .unwrap_or_else(chrono::Utc::now);
    super::osc_playback::ingest_timecode(
        state,
        light_control::SmpteTimecode {
            hours: timecode.sample.hours,
            minutes: timecode.sample.minutes,
            seconds: timecode.sample.seconds,
            frames: timecode.sample.frames,
            rate,
            source: format!(
                "extension:{}:{}",
                timecode.extension_id, timecode.extension_instance_id
            ),
            received_at,
        },
    );
    Ok(())
}

pub(super) fn feedback_context(
    state: Option<&AppState>,
    host: &HostControlContext,
) -> light_extensions_contract::FeedbackContext {
    let desk = state.and_then(|state| extension_desk(state, host));
    let show = state.and_then(|state| state.active_show.current());
    light_extensions_contract::FeedbackContext {
        desk_id: desk
            .map(|desk| desk.id.to_string())
            .unwrap_or_else(|| host.desk_id.clone()),
        show_id: show.as_ref().map(|show| show.id.0.to_string()),
        show_generation: show.map_or(0, |show| show.revision),
    }
}

fn extension_desk(state: &AppState, _context: &HostControlContext) -> Option<ControlDesk> {
    state.installation.desk().ok()
}

fn apply_playback_feedback(
    feedback: &mut FeedbackControlState,
    state: Option<&AppState>,
    desk: Option<&ControlDesk>,
    explicit_page: Option<u16>,
    slot: u16,
) {
    let Some((state, desk)) = state.zip(desk) else {
        feedback.available = false;
        feedback.enabled = false;
        return;
    };
    let snapshot = state.output.snapshot();
    let page = explicit_page
        .and_then(|page| u8::try_from(page).ok())
        .or_else(|| {
            state
                .active_show
                .current()
                .and_then(|show| state.installation.desk_page(desk.id, show.id).ok())
        })
        .unwrap_or(1);
    let playback_number = u8::try_from(slot).ok().and_then(|slot| {
        snapshot
            .playback_pages
            .iter()
            .find(|candidate| candidate.number == page)
            .and_then(|page| page.slots.get(&slot).copied())
    });
    let active = playback_number.and_then(|number| {
        state
            .output
            .active_playbacks()
            .into_iter()
            .find(|active| active.playback_number == Some(number))
    });
    feedback.available = playback_number.is_some();
    feedback.enabled = playback_number.is_some();
    feedback.selected = active.is_some();
    feedback.lamp = if feedback.selected {
        LampState::On
    } else {
        LampState::Off
    };
    feedback.value = active.as_ref().map(|active| active.master);
}

pub(super) fn apply_highlight_feedback(
    feedback: &mut FeedbackControlState,
    highlight: Option<&light_programmer::HighlightState>,
    action: HighlightControlAction,
) {
    feedback.available = highlight.is_some();
    let active = highlight.is_some_and(|state| state.active);
    feedback.enabled = match action {
        HighlightControlAction::Toggle => highlight.is_some(),
        HighlightControlAction::Previous => {
            active && highlight.is_some_and(|state| state.can_previous)
        }
        HighlightControlAction::Next => active && highlight.is_some_and(|state| state.can_next),
        HighlightControlAction::All => active,
    };
    feedback.selected = match action {
        HighlightControlAction::Toggle => active,
        HighlightControlAction::All => {
            active
                && highlight
                    .is_some_and(|state| state.mode == light_programmer::HighlightMode::Selection)
        }
        HighlightControlAction::Previous | HighlightControlAction::Next => false,
    };
    let lamp_on = match action {
        HighlightControlAction::Previous | HighlightControlAction::Next => feedback.enabled,
        HighlightControlAction::Toggle | HighlightControlAction::All => feedback.selected,
    };
    feedback.lamp = if lamp_on {
        LampState::On
    } else {
        LampState::Off
    };
}

fn control_label(intent: &CanonicalControlIntent) -> String {
    match intent {
        CanonicalControlIntent::ProgrammerKey { key } => {
            programmer_key_name(*key).to_ascii_uppercase()
        }
        CanonicalControlIntent::Modifier { .. } => "SHIFT".into(),
        CanonicalControlIntent::Navigation { action } => format!("{action:?}"),
        CanonicalControlIntent::Highlight { action } => match action {
            HighlightControlAction::Toggle => "HIGH".into(),
            HighlightControlAction::Previous => "PREV".into(),
            HighlightControlAction::Next => "NEXT".into(),
            HighlightControlAction::All => "ALL".into(),
        },
        CanonicalControlIntent::Encoder { index } => format!("Encoder {index}"),
        CanonicalControlIntent::PlaybackCurrent { slot, control } => {
            format!("Playback {slot} {control:?}")
        }
        CanonicalControlIntent::PlaybackExplicit {
            page,
            slot,
            control,
        } => {
            format!("Page {page} Playback {slot} {control:?}")
        }
        CanonicalControlIntent::SpeedGroup { group, control } => {
            format!("Speed Group {} {control:?}", group.to_ascii_uppercase())
        }
        CanonicalControlIntent::GrandMaster => "Grand Master".into(),
        CanonicalControlIntent::Blackout => "Blackout".into(),
        CanonicalControlIntent::DeskCommand { command } => format!("{command:?}"),
    }
}

#[derive(Clone)]
pub(super) struct ExtensionResource {
    manager: Arc<Mutex<ExtensionManager<HeadlessExtensionPorts>>>,
    ports: Arc<HeadlessExtensionPorts>,
    replay: Arc<Mutex<BTreeMap<(uuid::Uuid, String), ExtensionRuntimeSnapshot>>>,
    shift_held: Arc<Mutex<BTreeMap<(String, String), bool>>>,
}

impl ExtensionResource {
    pub(super) fn start(extensions_directory: PathBuf, configuration_path: PathBuf) -> Self {
        let ports = Arc::new(HeadlessExtensionPorts::default());
        let mut manager =
            ExtensionManager::new(extensions_directory, configuration_path, Arc::clone(&ports));
        manager.rescan();
        Self {
            manager: Arc::new(Mutex::new(manager)),
            ports,
            replay: Arc::new(Mutex::new(BTreeMap::new())),
            shift_held: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub(super) fn attach_state(&self, mut state: AppState) {
        state.extensions = Self::detached();
        *self.ports.state.write() = Some(state.clone());

        let resource = self.clone();
        let mut events = state.events.subscribe(
            light_application::EventFilter::default(),
            light_application::SubscriptionOptions::default(),
        );
        let lifecycle = state.lifecycle.clone();
        let task_lifecycle = lifecycle.clone();
        lifecycle
            .schedule(async move {
                loop {
                    tokio::select! {
                        _ = task_lifecycle.cancelled() => break,
                        delivery = events.next() => {
                            if delivery.is_none() {
                                break;
                            }
                            resource.refresh_feedback_snapshots();
                        }
                    }
                }
                Ok(())
            })
            .expect("extension feedback task must fit the startup supervisor queue");
    }

    fn detached() -> Self {
        let ports = Arc::new(HeadlessExtensionPorts::default());
        Self {
            manager: Arc::new(Mutex::new(ExtensionManager::new(
                PathBuf::new(),
                PathBuf::new(),
                Arc::clone(&ports),
            ))),
            ports,
            replay: Arc::new(Mutex::new(BTreeMap::new())),
            shift_held: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    fn set_shift_held(&self, host: &HostControlContext, held: bool) {
        let key = (
            host.extension_id.clone(),
            host.extension_instance_id.clone(),
        );
        let mut controls = self.shift_held.lock();
        if held {
            controls.insert(key, true);
        } else {
            controls.remove(&key);
        }
    }

    fn shift_held(&self, host: &HostControlContext) -> bool {
        self.shift_held.lock().contains_key(&(
            host.extension_id.clone(),
            host.extension_instance_id.clone(),
        ))
    }

    pub(super) fn rescan(&self) -> ExtensionRuntimeSnapshot {
        let mut manager = self.manager.lock();
        manager.rescan();
        project(manager.snapshot())
    }
    pub(super) fn snapshot(&self) -> ExtensionRuntimeSnapshot {
        project(self.manager.lock().snapshot())
    }

    pub(super) fn refresh_feedback_snapshots(&self) {
        self.ports.feedback_revision.fetch_add(1, Ordering::AcqRel);
        self.manager.lock().refresh_feedback_snapshots();
    }

    fn replayed_rescan(
        &self,
        session: light_core::SessionId,
        request_id: &str,
    ) -> ExtensionRuntimeSnapshot {
        let key = (session.0, request_id.to_owned());
        if let Some(snapshot) = self.replay.lock().get(&key).cloned() {
            return snapshot;
        }
        let snapshot = self.rescan();
        let mut replay = self.replay.lock();
        if replay.len() >= 128
            && let Some(oldest) = replay.keys().next().cloned()
        {
            replay.remove(&oldest);
        }
        replay.insert(key, snapshot.clone());
        snapshot
    }
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/extensions", get(snapshot_http))
        .route("/api/v2/extensions/rescan", post(rescan_http))
}

async fn snapshot_http(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ExtensionRuntimeSnapshot>, ApiError> {
    let _ = authenticate(&state, &headers)?;
    Ok(Json(state.extensions.snapshot()))
}

async fn rescan_http(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<ExtensionRescanRequest>,
) -> Result<Json<ExtensionRuntimeSnapshot>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if request.request_id.trim().is_empty() || request.request_id.len() > 128 {
        return Err(ApiError::bad_request(
            "request_id must contain 1 to 128 characters",
        ));
    }
    Ok(Json(
        state
            .extensions
            .replayed_rescan(session.id, &request.request_id),
    ))
}

fn control_id(input: &ControlInput) -> &str {
    match input {
        ControlInput::Button { control_id, .. }
        | ControlInput::Absolute { control_id, .. }
        | ControlInput::Relative { control_id, .. } => control_id,
    }
}

pub(super) fn apply_bound_control(
    state: &AppState,
    host: &HostControlContext,
    bound: &BoundControlInput,
) -> Result<(), PortError> {
    let desk = state
        .installation
        .desk()
        .map_err(|error| PortError::new(format!("the desk is unavailable: {error}")))?;
    let session = state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.connected && session.desk.id == desk.id);
    let context = light_application::ActionContext::system(
        desk.id,
        light_application::ActionSource::Extension,
    );
    match (&bound.intent, &bound.input.control) {
        (CanonicalControlIntent::PlaybackCurrent { slot, control }, input) => execute_playback(
            state,
            session.as_ref(),
            &desk,
            context,
            light_application::PlaybackAddress::CurrentPage {
                slot: u8::try_from(*slot)
                    .map_err(|_| PortError::new("playback slot exceeds 255"))?,
            },
            *control,
            input,
        ),
        (
            CanonicalControlIntent::PlaybackExplicit {
                page,
                slot,
                control,
            },
            input,
        ) => execute_playback(
            state,
            session.as_ref(),
            &desk,
            context,
            light_application::PlaybackAddress::ExplicitPage {
                page: u8::try_from(*page)
                    .map_err(|_| PortError::new("playback page exceeds 255"))?,
                slot: u8::try_from(*slot)
                    .map_err(|_| PortError::new("playback slot exceeds 255"))?,
            },
            *control,
            input,
        ),
        (CanonicalControlIntent::GrandMaster, ControlInput::Absolute { value, .. }) => {
            output_runtime_service::execute(
                state,
                session.as_ref(),
                context,
                output_runtime_service::command(Some(*value), None)
                    .map_err(|error| PortError::new(error.message))?,
            )
            .map_err(|error| PortError::new(error.message))?;
            Ok(())
        }
        (CanonicalControlIntent::Blackout, ControlInput::Button { pressed, .. }) => {
            output_runtime_service::execute(
                state,
                session.as_ref(),
                context,
                output_runtime_service::command(None, Some(*pressed))
                    .map_err(|error| PortError::new(error.message))?,
            )
            .map_err(|error| PortError::new(error.message))?;
            Ok(())
        }
        (CanonicalControlIntent::ProgrammerKey { key }, ControlInput::Button { pressed, .. }) => {
            apply_programmer_key(&state, session.as_ref(), &desk, host, *key, *pressed)
        }
        (
            CanonicalControlIntent::Modifier {
                modifier: ModifierKey::Shift,
            },
            ControlInput::Button { pressed, .. },
        ) => apply_programmer_modifier(&state, session.as_ref(), host, *pressed),
        (CanonicalControlIntent::Highlight { action }, ControlInput::Button { pressed, .. }) => {
            if !pressed {
                return Ok(());
            }
            let session = require_session(session.as_ref(), "Highlight")?;
            super::execute_highlight(
                &state,
                session,
                light_application::HighlightCommand::action(match action {
                    HighlightControlAction::Toggle => light_programmer::HighlightAction::Toggle,
                    HighlightControlAction::Previous => light_programmer::HighlightAction::Previous,
                    HighlightControlAction::Next => light_programmer::HighlightAction::Next,
                    HighlightControlAction::All => light_programmer::HighlightAction::All,
                }),
                light_application::ActionSource::Extension,
                false,
            )
            .map_err(|error| PortError::new(error.message))?;
            Ok(())
        }
        (CanonicalControlIntent::SpeedGroup { group, control }, input) => {
            apply_speed_group(&state, *group, *control, input)
        }
        (intent, input) => {
            let Some(notification) = desk_action_notification(intent, input) else {
                return Ok(());
            };
            emit(
                state,
                "desk_action",
                serde_json::json!({
                    "path": OSC_DESK_PATH,
                    "desk_id": desk.id,
                    "session_id": session.as_ref().map(|session| session.id),
                    "action": notification.action,
                    "control": notification.control,
                    "value": notification.value,
                    "source": "extension",
                    "extension_id": host.extension_id,
                    "extension_instance_id": host.extension_instance_id,
                }),
            );
            Ok(())
        }
    }
}

fn require_session<'a>(
    session: Option<&'a Session>,
    control: &str,
) -> Result<&'a Session, PortError> {
    session.ok_or_else(|| PortError::new(format!("{control} requires a connected desk session")))
}

fn apply_programmer_key(
    state: &AppState,
    session: Option<&Session>,
    desk: &ControlDesk,
    host: &HostControlContext,
    key: ProgrammerKey,
    pressed: bool,
) -> Result<(), PortError> {
    let session = require_session(session, "Programmer control")?;
    let shifted = state.extensions.shift_held(host);
    if pressed
        && super::file_manager::route_control_input(
            state,
            session,
            programmer_key_name(key),
            "extension",
        )
    {
        return Ok(());
    }
    if key == ProgrammerKey::Set
        && pressed
        && state.programming.get(session.id).is_some_and(|programmer| {
            matches!(programmer.command_line.trim(), "" | "FIXTURE" | "GROUP")
        })
    {
        emit(
            state,
            "desk_action",
            serde_json::json!({
                "path": OSC_DESK_PATH,
                "desk_id": desk.id,
                "session_id": session.id,
                "action": "set",
                "source": "extension",
                "extension_id": host.extension_id,
                "extension_instance_id": host.extension_instance_id,
            }),
        );
        return Ok(());
    }
    let key = application_programmer_key(key);
    let gesture = pressed.then(|| {
        let current = state
            .programming
            .get(session.id)
            .map(|programmer| programmer.command_line)
            .unwrap_or_default();
        let repeated = if shifted {
            match key {
                light_programmer::command_line::CommandKey::Group => {
                    current.trim_end().ends_with("FIXTURE")
                }
                light_programmer::command_line::CommandKey::Divide => {
                    current.trim_end().ends_with("GO TO")
                }
                light_programmer::command_line::CommandKey::Clear => {
                    current.trim_end().ends_with("FREEZE")
                }
                light_programmer::command_line::CommandKey::Digit(digit) => [
                    "ALL",
                    "INTENSITY",
                    "COLOR",
                    "POSITION",
                    "BEAM",
                    "DYNAMICS",
                    "SHAPERS",
                    "FOCUS",
                    "CONTROL",
                    "MEDIA",
                ]
                .get(usize::from(digit))
                .is_some_and(|family| current.trim_end().ends_with(family)),
                _ => false,
            }
        } else {
            key == light_programmer::command_line::CommandKey::Off
                && current.trim_end().ends_with("OFF")
        };
        light_programmer::command_line::CommandGesture {
            kind: if repeated {
                light_programmer::command_line::CommandGestureKind::Double
            } else {
                light_programmer::command_line::CommandGestureKind::Regular
            },
            shifted,
        }
    });
    if let Some(gesture) = gesture {
        let current = state
            .programming
            .command_line_state(session.id)
            .ok_or_else(|| PortError::new("Programmer command line is unavailable"))?;
        if let light_programmer::command_line::CommandGestureIntent::Immediate(action) =
            light_programmer::command_line::command_gesture_intent(&current, key, gesture)
        {
            let action = match action {
                light_programmer::command_line::CommandImmediateAction::RunningOutput => {
                    "running-output"
                }
                light_programmer::command_line::CommandImmediateAction::Lock => "shift-enter",
                light_programmer::command_line::CommandImmediateAction::Undo => "shift-escape",
                light_programmer::command_line::CommandImmediateAction::ClearPreload => {
                    "shift-preload"
                }
                light_programmer::command_line::CommandImmediateAction::AlignOff => "shift-align",
                _ => return Ok(()),
            };
            emit(
                state,
                "desk_action",
                serde_json::json!({
                    "path": OSC_DESK_PATH,
                    "desk_id": desk.id,
                    "session_id": session.id,
                    "action": action,
                    "source": "extension",
                    "extension_id": host.extension_id,
                    "extension_instance_id": host.extension_instance_id,
                }),
            );
            return Ok(());
        }
    }
    apply_application_programmer_key(state, session, key, pressed, gesture)
}

fn apply_application_programmer_key(
    state: &AppState,
    session: &Session,
    key: light_programmer::command_line::CommandKey,
    pressed: bool,
    gesture: Option<light_programmer::command_line::CommandGesture>,
) -> Result<(), PortError> {
    let _activation = super::programming_interaction::try_programming_activation(state)
        .map_err(PortError::new)?;
    let result = command_http::run_service_with_source(
        state,
        session,
        light_application::ActionContext::operator(
            session.desk.id,
            session.id.0,
            light_application::ActionSource::Extension,
        ),
        light_application::ProgrammingCommand::ApplyKey {
            key,
            phase: if pressed {
                light_programmer::command_line::CommandKeyPhase::Press
            } else {
                light_programmer::command_line::CommandKeyPhase::Release
            },
            gesture,
            execute_policy: light_application::ExecutionPolicy::Compatibility,
        },
        "extension",
    )
    .map_err(|error| PortError::new(error.message))?;
    if let Some(warning) =
        command_http::publish_service_result(state, session, &result, "extension", None, None)
    {
        return Err(PortError::new(warning));
    }
    Ok(())
}

fn apply_programmer_modifier(
    state: &AppState,
    session: Option<&Session>,
    host: &HostControlContext,
    pressed: bool,
) -> Result<(), PortError> {
    let session = require_session(session, "Programmer modifier")?;
    apply_application_programmer_key(
        state,
        session,
        light_programmer::command_line::CommandKey::Shift,
        pressed,
        None,
    )?;
    state.extensions.set_shift_held(host, pressed);
    Ok(())
}

fn apply_speed_group(
    state: &AppState,
    group: char,
    control: SpeedGroupControl,
    input: &ControlInput,
) -> Result<(), PortError> {
    let index = "ABCDE"
        .find(group.to_ascii_uppercase())
        .ok_or_else(|| PortError::new("Speed Group must be A-E"))?;
    let now = application_millis(state);
    let affected = match (control, input) {
        (SpeedGroupControl::Tap, ControlInput::Button { pressed: true, .. }) => {
            state.output.apply_speed_group_action(index, now, "learn")
        }
        (SpeedGroupControl::Double, ControlInput::Button { pressed: true, .. }) => {
            state.output.apply_speed_group_action(index, now, "double")
        }
        (SpeedGroupControl::Half, ControlInput::Button { pressed: true, .. }) => {
            state.output.apply_speed_group_action(index, now, "half")
        }
        (SpeedGroupControl::Level, ControlInput::Absolute { value, .. }) => {
            state.output.set_speed_group_level(index, now, *value)
        }
        (
            SpeedGroupControl::Tap | SpeedGroupControl::Double | SpeedGroupControl::Half,
            ControlInput::Button { pressed: false, .. },
        ) => return Ok(()),
        _ => {
            return Err(PortError::new(
                "control value does not match Speed Group binding",
            ));
        }
    }
    .map_err(|error| PortError::new(error.message))?;
    copy_speed_group_runtime_to_configuration(state, &affected);
    persist_server_configuration(state).map_err(|error| PortError::new(error.message))?;
    let snapshots = refresh_speed_group_engine(state);
    emit(
        state,
        "speed_group_action",
        serde_json::json!({
            "group": group.to_ascii_uppercase().to_string(),
            "path": OSC_DESK_PATH,
            "source": "extension",
            "action": match control {
                SpeedGroupControl::Tap => "learn",
                SpeedGroupControl::Double => "double",
                SpeedGroupControl::Half => "half",
                SpeedGroupControl::Level => "level",
            },
            "snapshot": snapshots[index],
        }),
    );
    Ok(())
}

fn application_programmer_key(key: ProgrammerKey) -> light_programmer::command_line::CommandKey {
    use ProgrammerKey::*;
    use light_programmer::command_line::CommandKey as App;
    match key {
        Zero => App::Digit(0),
        One => App::Digit(1),
        Two => App::Digit(2),
        Three => App::Digit(3),
        Four => App::Digit(4),
        Five => App::Digit(5),
        Six => App::Digit(6),
        Seven => App::Digit(7),
        Eight => App::Digit(8),
        Nine => App::Digit(9),
        Plus => App::Plus,
        Minus => App::Minus,
        Point => App::Dot,
        At => App::At,
        Enter => App::Enter,
        Clear => App::Clear,
        Undo => App::Undo,
        Group => App::Group,
        Cue => App::Cue,
        Playback => App::Playback,
        Off => App::Off,
        Record => App::Record,
        Preload => App::Preload,
        Delete => App::Delete,
        Copy => App::Copy,
        Move => App::Move,
        Set => App::Set,
        Time => App::Time,
        Thru => App::Thru,
        Divide => App::Divide,
        Backspace => App::Backspace,
        Escape => App::Escape,
        Highlight => App::Highlight,
        Previous => App::Previous,
        Next => App::Next,
        All => App::All,
        EncoderPlayback => App::EncoderPlayback,
        PageUp => App::PageUp,
        PageDown => App::PageDown,
        Align => App::Align,
        Fade => App::Fade,
    }
}

fn execute_playback(
    state: &AppState,
    session: Option<&Session>,
    desk: &ControlDesk,
    context: light_application::ActionContext,
    address: light_application::PlaybackAddress,
    control: PlaybackControl,
    input: &ControlInput,
) -> Result<(), PortError> {
    let action = match (control, input) {
        (PlaybackControl::Master, ControlInput::Absolute { value, .. }) => {
            light_application::PlaybackAction::Master(light_application::PlaybackLevel::new(*value))
        }
        (PlaybackControl::ButtonOne, ControlInput::Button { pressed, .. }) => {
            light_application::PlaybackAction::ConfiguredButton {
                number: 1,
                pressed: *pressed,
            }
        }
        (PlaybackControl::ButtonTwo, ControlInput::Button { pressed, .. }) => {
            light_application::PlaybackAction::ConfiguredButton {
                number: 2,
                pressed: *pressed,
            }
        }
        (PlaybackControl::ButtonThree, ControlInput::Button { pressed, .. }) => {
            light_application::PlaybackAction::ConfiguredButton {
                number: 3,
                pressed: *pressed,
            }
        }
        _ => {
            return Err(PortError::new(
                "control value does not match playback binding",
            ));
        }
    };
    playback_service::execute(
        state,
        session,
        Some(desk),
        context,
        light_application::PlaybackCommand {
            address,
            action,
            surface: light_application::PlaybackSurface::Physical,
        },
    )
    .map_err(|error| PortError::new(error.message))?;
    Ok(())
}

struct DeskActionProjection {
    action: Option<String>,
    control: Option<String>,
    value: Option<String>,
}

fn desk_action_notification(
    intent: &CanonicalControlIntent,
    input: &ControlInput,
) -> Option<DeskActionProjection> {
    let action = match intent {
        CanonicalControlIntent::Modifier { .. } => match input {
            ControlInput::Button { pressed, .. } => Some(if *pressed {
                "shift-down".into()
            } else {
                "shift-up".into()
            }),
            _ => return None,
        },
        CanonicalControlIntent::Navigation {
            action: NavigationAction::Menu,
        } => pressed(input).then(|| "menu".into()),
        CanonicalControlIntent::Navigation {
            action: NavigationAction::Escape,
        } => pressed(input).then(|| "escape".into()),
        CanonicalControlIntent::ProgrammerKey { key } => {
            pressed(input).then(|| programmer_key_name(*key).into())
        }
        CanonicalControlIntent::Highlight { action } => pressed(input).then(|| {
            match action {
                HighlightControlAction::Toggle => "highlight-toggle",
                HighlightControlAction::Previous => "highlight-previous",
                HighlightControlAction::Next => "highlight-next",
                HighlightControlAction::All => "highlight-all",
            }
            .into()
        }),
        CanonicalControlIntent::SpeedGroup { group, control } => pressed(input).then(|| {
            format!("speed-group-{}-{control:?}", group.to_ascii_lowercase()).to_ascii_lowercase()
        }),
        CanonicalControlIntent::DeskCommand { command } => {
            pressed(input).then(|| format!("desk-{command:?}").to_ascii_lowercase())
        }
        CanonicalControlIntent::Navigation { .. } | CanonicalControlIntent::Encoder { .. } => None,
        CanonicalControlIntent::PlaybackCurrent { .. }
        | CanonicalControlIntent::PlaybackExplicit { .. }
        | CanonicalControlIntent::GrandMaster
        | CanonicalControlIntent::Blackout => return None,
    };
    let (control, value) = match intent {
        CanonicalControlIntent::Navigation { action } if action_value(*action).is_some() => {
            (Some("nav".into()), action_value(*action).map(str::to_owned))
        }
        CanonicalControlIntent::Encoder { index } => {
            let value = match input {
                ControlInput::Button { pressed: true, .. } => Some("press"),
                ControlInput::Button { pressed: false, .. } => None,
                ControlInput::Relative { delta, .. } if *delta > 0 => Some("up"),
                ControlInput::Relative { delta, .. } if *delta < 0 => Some("down"),
                _ => None,
            }?;
            (Some(format!("encode/{index}")), Some(value.into()))
        }
        _ => (None, None),
    };
    (action.is_some() || control.is_some()).then_some(DeskActionProjection {
        action,
        control,
        value,
    })
}

fn action_value(action: NavigationAction) -> Option<&'static str> {
    match action {
        NavigationAction::Up => Some("up"),
        NavigationAction::Down => Some("down"),
        NavigationAction::Left => Some("left"),
        NavigationAction::Right => Some("right"),
        NavigationAction::PageUp => Some("page-up"),
        NavigationAction::PageDown => Some("page-down"),
        NavigationAction::Menu | NavigationAction::Escape => None,
    }
}

fn pressed(input: &ControlInput) -> bool {
    matches!(input, ControlInput::Button { pressed: true, .. })
}

fn programmer_key_name(key: ProgrammerKey) -> &'static str {
    use ProgrammerKey::*;
    match key {
        Zero => "0",
        One => "1",
        Two => "2",
        Three => "3",
        Four => "4",
        Five => "5",
        Six => "6",
        Seven => "7",
        Eight => "8",
        Nine => "9",
        Plus => "plus",
        Minus => "minus",
        Point => "point",
        At => "at",
        Enter => "enter",
        Clear => "clear",
        Undo => "undo",
        Group => "group",
        Cue => "cue",
        Playback => "playback",
        Off => "off",
        Record => "record",
        Preload => "preload",
        Delete => "delete",
        Copy => "copy",
        Move => "move",
        Set => "set",
        Time => "time",
        Thru => "thru",
        Divide => "divide",
        Backspace => "backspace",
        Escape => "escape",
        Highlight => "highlight",
        Previous => "previous",
        Next => "next",
        All => "all",
        EncoderPlayback => "encoder_playback",
        PageUp => "page_up",
        PageDown => "page_down",
        Align => "align",
        Fade => "fade",
    }
}

fn project(snapshot: &light_extensions_host::ExtensionManagerSnapshot) -> ExtensionRuntimeSnapshot {
    ExtensionRuntimeSnapshot {
        extensions_directory: snapshot.extensions_directory.display().to_string(),
        configuration_path: snapshot.configuration_path.display().to_string(),
        configuration_diagnostic: snapshot.configuration_diagnostic.clone(),
        packages: snapshot
            .packages
            .iter()
            .map(|package| ExtensionPackageSnapshot {
                id: package
                    .manifest
                    .as_ref()
                    .map(|manifest| manifest.id.clone()),
                name: package
                    .manifest
                    .as_ref()
                    .map(|manifest| manifest.name.clone()),
                version: package
                    .manifest
                    .as_ref()
                    .map(|manifest| manifest.version.clone()),
                directory: package.directory.display().to_string(),
                package_digest: package.package_digest.clone(),
                readiness: format!("{:?}", package.readiness).to_ascii_lowercase(),
                locally_approved_unsigned: package.locally_approved_unsigned,
                diagnostics: package
                    .diagnostics
                    .iter()
                    .map(|diagnostic| ExtensionDiagnostic {
                        code: format!("{:?}", diagnostic.code).to_ascii_lowercase(),
                        detail: diagnostic.detail.clone(),
                    })
                    .collect(),
            })
            .collect(),
        instances: snapshot
            .instances
            .iter()
            .map(|instance| {
                let health = instance.health.clone();
                ExtensionInstanceSnapshot {
                    id: instance.instance_id.clone(),
                    extension_id: instance.extension_id.clone(),
                    package_digest: instance.package_digest.clone(),
                    executable: instance.executable.display().to_string(),
                    state: health.as_ref().map_or_else(
                        || "starting".into(),
                        |health| format!("{:?}", health.state).to_ascii_lowercase(),
                    ),
                    last_error: health.as_ref().and_then(|health| health.last_error.clone()),
                    launches: health.as_ref().map_or(0, |health| health.launches),
                    crashes: health.as_ref().map_or(0, |health| health.crashes),
                    protocol_errors: health.as_ref().map_or(0, |health| health.protocol_errors),
                    inbound_drops: health.as_ref().map_or(0, |health| health.inbound_drops),
                    outbound_drops: health.as_ref().map_or(0, |health| health.outbound_drops),
                }
            })
            .collect(),
        instance_diagnostics: snapshot
            .instance_diagnostics
            .iter()
            .map(|diagnostic| ExtensionInstanceDiagnosticSnapshot {
                instance_id: diagnostic.instance_id.clone(),
                code: format!("{:?}", diagnostic.code).to_ascii_lowercase(),
                detail: diagnostic.detail.clone(),
            })
            .collect(),
    }
}
