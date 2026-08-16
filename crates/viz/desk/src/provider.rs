//! The lighting-desk scene provider.
//!
//! Two deliberately separate input planes meet here and nowhere else:
//!
//! - the desk API supplies scene and configuration over authenticated HTTP plus the existing
//!   event WebSocket; and
//! - the lighting network supplies every live output value as real Art-Net or sACN UDP.
//!
//! The API is never asked for live values, and the event connection is never used as a DMX
//! transport.

use crate::client::DeskClient;
use crate::routes;
use crate::scene_build::{self, DeskReadModels};
use crate::wire::StageLayoutBody;
use futures_util::StreamExt;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};
use viz_dmx::DmxReceiver;
use viz_project::Decoder;
use viz_scene::{
    ConnectionState, ProviderCapabilities, ProviderDiagnostics, ProviderError, ProviderEvent,
    ProviderKind, SceneProvider, SceneValues,
};

/// How the operator configured this connection.
#[derive(Clone, Debug)]
pub struct DeskConnection {
    pub host: String,
    pub port: u16,
    pub user: String,
    /// Interface the receivers bind to. `None` binds every interface.
    pub bind_interface: Option<Ipv4Addr>,
    /// Bounded reconnect backoff.
    pub retry: Duration,
    /// Operator statements about where a universe actually arrives, overriding the show's routes.
    pub input_overrides: Vec<viz_dmx::UniverseInput>,
    /// Which renderer this window is, for a desk driving more than one. The desk keeps a view per
    /// target, so two renderers side by side can show two different things.
    pub target: String,
    /// Read live values from the desk's own output rather than from the network.
    ///
    /// Off for every renderer on a network, where the two-plane rule holds: a desk's live values
    /// arrive as real Art-Net or sACN, and a visualizer that invented a second path would be
    /// showing something no lighting rig would.
    ///
    /// On for a renderer drawing the desk's Stage inside the desk's own window, where the rule has
    /// nothing to protect: the two processes are one product on one machine, the desk already
    /// knows the values, and a desk with no output routes configured still has a Stage its
    /// operator expects to see lit.
    pub values_from_desk_output: bool,
}

impl Default for DeskConnection {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 5000,
            user: "Operator".into(),
            bind_interface: None,
            retry: Duration::from_secs(2),
            input_overrides: Vec::new(),
            target: "main".into(),
            // The network rule is the default; the desk's own window opts out deliberately.
            values_from_desk_output: false,
        }
    }
}

/// What the connection thread sends back to the render thread.
enum Message {
    Connection(ConnectionState),
    /// A complete scene that replaces whatever is displayed, values and receivers included.
    Scene {
        plan: Box<viz_scene::Scene>,
        bindings: Vec<viz_project::EmitterBinding>,
        external_camera: Option<viz_project::ExternalCameraBinding>,
        mappings: Vec<viz_dmx::InputMapping>,
        diagnostics: Box<ProviderDiagnostics>,
    },
    /// The same show, re-read after a configuration change: applied in place, keeping the live
    /// values and the sockets that are already delivering them.
    Delta {
        plan: Box<viz_scene::Scene>,
        bindings: Vec<viz_project::EmitterBinding>,
        external_camera: Option<viz_project::ExternalCameraBinding>,
        mappings: Vec<viz_dmx::InputMapping>,
        diagnostics: Box<ProviderDiagnostics>,
    },
    Diagnostics(Box<ProviderDiagnostics>),
    /// What the planning window is driving the rig with, as read. Only a planning source ever
    /// sends this.
    Preview(Box<crate::wire::PreviewSnapshot>),
    /// Identity-based editor/CAD selection from a planning source.
    Selection(Box<crate::wire::SelectionSnapshot>),
    /// The desk's own output, for a renderer inside the desk's window.
    DeskOutput(Box<crate::wire::OutputDmxSnapshot>),
    /// The operator's preload, laid over the live picture while they are following it.
    Preload2(Box<crate::wire::PreloadProjection>),
    /// The desk's own view for this target, as read. Converting it needs the scene, which lives
    /// on the render thread, so the raw reading travels and the conversion happens there.
    View(Box<DeskView>),
    RendererSettings(Box<viz_scene::RendererSettingsUpdate>),
    Resync(String),
}

/// The desk's view for one target, in the renderer's own vocabulary but without a camera yet.
#[derive(Clone, Debug)]
pub struct DeskView {
    pub mode: viz_scene::ViewMode,
    pub quality: viz_scene::RenderQuality,
    /// Absent means the desk has no opinion about where the camera stands, and the renderer
    /// frames the named view from the rig itself.
    pub camera: Option<viz_scene::Camera>,
    pub exposure: f32,
    pub ambient: f32,
    pub revision: u64,
    pub physics_reset_generation: u64,
}

/// Commands the render thread sends to the connection thread.
///
/// Stopping is a shared flag rather than a message: a message can only be consumed once, and the
/// connection thread reads its inbox from more than one place, so a consumed stop would leave the
/// worker reconnecting after shutdown.
enum Command {
    Resync,
    RendererSettings(viz_scene::RendererSettingsIntent),
}

pub struct DeskProvider {
    connection: DeskConnection,
    inbox: Receiver<Message>,
    commands: tokio::sync::mpsc::UnboundedSender<Command>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    receivers: Option<DmxReceiver>,
    decoder: Option<Decoder>,
    scene: Option<viz_scene::Scene>,
    values: SceneValues,
    diagnostics: ProviderDiagnostics,
    epoch: Instant,
    /// What the running receivers were started with, so a delta only rebinds them when the show
    /// actually moved a universe somewhere else.
    mappings: Vec<viz_dmx::InputMapping>,
    /// Emitted once per scene so the host can display it before DMX arrives.
    pending_snapshot: bool,
    /// Emitted once per applied delta, after the scene has been replaced in place.
    pending_delta: bool,
    /// The desk's own view for this target, once it has said. `None` while the desk has no
    /// opinion, which is also what a desk too old to have one leaves behind.
    view: Option<DeskView>,
    /// Emitted once per view the desk sends, so an operator's local selection is only replaced
    /// when the desk actually says something.
    pending_view: bool,
    /// Newest accepted packet already reported to the host.
    reported_input_micros: u64,
    value_frame: u64,
    /// The planning window's preview values, and the revision of them already folded in.
    ///
    /// Empty for a lighting desk, which never serves them.
    preview: crate::wire::PreviewSnapshot,
    selection: crate::wire::SelectionSnapshot,
    /// The desk's own output, while a renderer in the desk's window is reading it.
    desk_output: Option<crate::wire::OutputDmxSnapshot>,
    /// The operator's preload, and whether they are following it.
    preload_projection: crate::wire::PreloadProjection,
    following_preload: bool,
    applied_preview_revision: Option<u64>,
    /// Universes a real source has delivered at least one packet on, ever.
    ///
    /// Once a universe has had real DMX it keeps it: losing the source holds the last received
    /// values rather than handing the universe back to the editor, because a rig that jumps back
    /// to a preview look the moment a console is unplugged is worse than one that freezes.
    real_universes: std::collections::BTreeSet<u16>,
}

impl DeskProvider {
    pub fn start(connection: DeskConnection, epoch: Instant) -> Self {
        let (outbox, inbox) = channel();
        let (commands, orders) = tokio::sync::mpsc::unbounded_channel();
        let worker_connection = connection.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker = std::thread::Builder::new()
            .name("viz-desk".into())
            .spawn(move || run(worker_connection, outbox, orders, worker_stop))
            .ok();
        Self {
            connection,
            inbox,
            commands,
            stop,
            worker,
            receivers: None,
            decoder: None,
            scene: None,
            values: SceneValues::default(),
            diagnostics: ProviderDiagnostics::default(),
            epoch,
            mappings: Vec::new(),
            pending_snapshot: false,
            pending_delta: false,
            view: None,
            pending_view: false,
            reported_input_micros: 0,
            value_frame: 0,
            preview: crate::wire::PreviewSnapshot::default(),
            selection: crate::wire::SelectionSnapshot::default(),
            desk_output: None,
            preload_projection: crate::wire::PreloadProjection::default(),
            following_preload: false,
            applied_preview_revision: None,
            real_universes: std::collections::BTreeSet::new(),
        }
    }

    fn adopt_scene(
        &mut self,
        scene: viz_scene::Scene,
        bindings: Vec<viz_project::EmitterBinding>,
        external_camera: Option<viz_project::ExternalCameraBinding>,
        mappings: Vec<viz_dmx::InputMapping>,
        diagnostics: ProviderDiagnostics,
    ) {
        // Replace receivers first so no frame from the previous scene reaches the new bindings.
        if let Some(mut receivers) = self.receivers.take() {
            receivers.shutdown();
        }
        let decoder = Decoder::with_external_camera(
            bindings,
            (!self.connection.values_from_desk_output)
                .then_some(external_camera)
                .flatten(),
        );
        let mappings = self.resolved_mappings(mappings, &decoder);
        self.receivers = self
            .listens_on_the_network()
            .then(|| DmxReceiver::start(mappings.clone(), self.epoch));
        self.mappings = mappings;
        self.values = SceneValues::default();
        self.values.resize(scene.emitters.len());
        apply_selection(&mut self.values, &self.selection);
        decoder.initialize_motion(&scene, &mut self.values);
        self.decoder = Some(decoder);
        self.reported_input_micros = 0;
        self.scene = Some(scene);
        self.diagnostics = diagnostics;
        self.pending_snapshot = true;
        self.pending_delta = false;
    }

    /// Apply a re-read of the same show without interrupting anything that is working.
    ///
    /// A fixture moved, renamed, repatched or added is a configuration change, not a new show.
    /// The sockets stay open unless the show actually moved a universe somewhere else, and every
    /// head that still exists keeps the level and colour it is being sent — a rig edited during a
    /// running show must not blink.
    fn apply_delta(
        &mut self,
        scene: viz_scene::Scene,
        bindings: Vec<viz_project::EmitterBinding>,
        external_camera: Option<viz_project::ExternalCameraBinding>,
        mappings: Vec<viz_dmx::InputMapping>,
        diagnostics: ProviderDiagnostics,
    ) {
        let Some(previous) = self.scene.take() else {
            // Nothing is displayed yet, so there is nothing to preserve.
            self.adopt_scene(scene, bindings, external_camera, mappings, diagnostics);
            return;
        };
        let decoder = Decoder::with_external_camera(
            bindings,
            (!self.connection.values_from_desk_output)
                .then_some(external_camera)
                .flatten(),
        );
        let mappings = self.resolved_mappings(mappings, &decoder);
        if mappings != self.mappings || self.receivers.is_none() {
            if let Some(mut receivers) = self.receivers.take() {
                receivers.shutdown();
            }
            self.receivers = self
                .listens_on_the_network()
                .then(|| DmxReceiver::start(mappings.clone(), self.epoch));
            self.mappings = mappings;
            self.reported_input_micros = 0;
        } else if let Some(receivers) = &self.receivers {
            // The bindings are new; every held universe has to be decoded through them again.
            receivers.refresh_all();
        }
        self.values.carry_over(&previous, &scene);
        decoder.reconcile_external_camera(&mut self.values);
        decoder.initialize_motion(&scene, &mut self.values);
        self.decoder = Some(decoder);
        self.scene = Some(scene);
        self.diagnostics = diagnostics;
        self.pending_delta = true;
    }

    /// The desk's view as the renderer applies it.
    ///
    /// A desk that names a mode but no camera is asking for that view of this rig, so the camera
    /// is framed from the scene rather than left wherever the last one stood.
    fn effective_view(&self) -> Option<viz_scene::ViewConfiguration> {
        let view = self.view.as_ref()?;
        let bounds = self
            .scene
            .as_ref()
            .map(viz_scene::Scene::framing_bounds)
            .unwrap_or_default();
        Some(viz_scene::ViewConfiguration {
            mode: view.mode,
            camera: view
                .camera
                .unwrap_or_else(|| viz_scene::Camera::framed(view.mode, bounds)),
            quality: view.quality,
            exposure: view.exposure,
            ambient: view.ambient,
            physics_reset_generation: view.physics_reset_generation,
            ..viz_scene::ViewConfiguration::default()
        })
    }

    /// The mappings the receivers are actually started with: the show's own, the defaults when it
    /// configures none, and the operator's overrides on top of either.
    /// The universes the planning window is currently driving.
    ///
    /// A universe the network has ever delivered is not one of them, and never becomes one again
    /// while this connection lasts. That is the whole precedence rule: editor values apply where
    /// no healthy input has ever delivered that universe, a real source takes it the moment one
    /// arrives, and losing that source afterwards holds the last received values rather than
    /// reverting. Nothing is merged per parameter — a universe has exactly one owner.
    fn editor_driven_universes(&self) -> std::collections::BTreeSet<u16> {
        editor_driven(&self.preview, &self.real_universes)
    }

    /// Follow the operator's preload, or show what is lit.
    pub fn follow_preload(&mut self, following: bool) {
        self.following_preload = following;
    }

    /// Whether this renderer binds sockets and waits for real packets.
    ///
    /// A renderer running on its own does: a desk's live values arrive as Art-Net or sACN, and
    /// that is the whole of the two-plane rule. One inside the desk's own window does not — it is
    /// handed the same numbers directly — and binding anyway would mean two processes on one
    /// machine competing for the same multicast groups to learn something one of them already
    /// knows.
    fn listens_on_the_network(&self) -> bool {
        !self.connection.values_from_desk_output
    }

    fn resolved_mappings(
        &self,
        mappings: Vec<viz_dmx::InputMapping>,
        decoder: &Decoder,
    ) -> Vec<viz_dmx::InputMapping> {
        let mappings = if mappings.is_empty() {
            routes::default_mappings(
                &decoder.required_universes(),
                self.connection.bind_interface,
            )
        } else {
            mappings
        };
        viz_dmx::apply_overrides(
            mappings,
            &self.connection.input_overrides,
            self.connection.bind_interface,
        )
    }

    fn drain_messages(&mut self, events: &mut Vec<ProviderEvent>) {
        loop {
            match self.inbox.try_recv() {
                Ok(Message::Connection(state)) => events.push(ProviderEvent::Connection(state)),
                Ok(Message::Scene {
                    plan,
                    bindings,
                    external_camera,
                    mappings,
                    diagnostics,
                }) => {
                    self.adopt_scene(*plan, bindings, external_camera, mappings, *diagnostics);
                }
                Ok(Message::Delta {
                    plan,
                    bindings,
                    external_camera,
                    mappings,
                    diagnostics,
                }) => {
                    self.apply_delta(*plan, bindings, external_camera, mappings, *diagnostics);
                }
                Ok(Message::Diagnostics(value)) => self.diagnostics = *value,
                Ok(Message::Preview(value)) => self.preview = *value,
                Ok(Message::Selection(value)) => {
                    if value.revision >= self.selection.revision {
                        self.selection = *value;
                        apply_selection(&mut self.values, &self.selection);
                        self.value_frame = self.value_frame.saturating_add(1);
                        self.values.frame = self.value_frame;
                        events.push(ProviderEvent::Values(Box::new(self.values.clone())));
                    }
                }
                Ok(Message::DeskOutput(value)) => self.desk_output = Some(*value),
                Ok(Message::Preload2(value)) => self.preload_projection = *value,
                Ok(Message::View(view)) => {
                    self.pending_view = self
                        .view
                        .as_ref()
                        .is_none_or(|current| current.revision != view.revision);
                    self.view = Some(*view);
                }
                Ok(Message::RendererSettings(settings)) => {
                    events.push(ProviderEvent::RendererSettings(*settings));
                }
                Ok(Message::Resync(reason)) => {
                    events.push(ProviderEvent::ResyncRequired { reason });
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    events.push(ProviderEvent::Connection(ConnectionState::Failed {
                        boundary: "desk connection".into(),
                        detail: "the connection worker stopped".into(),
                    }));
                    break;
                }
            }
        }
    }

    fn emit_pending_scene(&mut self, events: &mut Vec<ProviderEvent>) {
        if self.pending_snapshot
            && let Some(scene) = &self.scene
        {
            self.pending_snapshot = false;
            let view = self.effective_view();
            self.pending_view = false;
            events.push(ProviderEvent::Snapshot {
                scene: Box::new(scene.clone()),
                view,
            });
        }
        if self.pending_view {
            self.pending_view = false;
            if let Some(view) = self.effective_view() {
                events.push(ProviderEvent::View(view));
            }
        }
        if self.pending_delta
            && let Some(scene) = &self.scene
        {
            self.pending_delta = false;
            events.push(ProviderEvent::SceneDelta(Box::new(scene.clone())));
        }
    }

    fn update_real_universes(&mut self) {
        if let Some(receivers) = &self.receivers {
            self.real_universes.extend(
                receivers
                    .universes()
                    .into_iter()
                    .filter(|universe| universe.accepted > 0)
                    .map(|universe| universe.universe),
            );
        }
    }
}

impl SceneProvider for DeskProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            kind: ProviderKind::LightingDesk,
            available: true,
            unavailable_reason: None,
            default_host: "127.0.0.1".into(),
            default_port: 5000,
            uses_network_input: true,
        }
    }

    fn poll(&mut self) -> Vec<ProviderEvent> {
        let mut events = Vec::new();
        self.drain_messages(&mut events);
        self.emit_pending_scene(&mut events);

        // Live values come from the network, and — for a planning source only — from the preview
        // plane on universes the network has never claimed.
        // A universe that has ever accepted a real packet belongs to the network from then on.
        self.update_real_universes();
        // The editor's own frames, for the universes it still owns. Rebuilt every poll, and
        // re-applied whenever the operator changes something — including when a real source has
        // just taken a universe away, because the frame the decoder holds is a merge of everything
        // applied so far and the remaining preview universes have to be re-asserted.
        let preview_universes = self.editor_driven_universes();
        let preview_moved = self.applied_preview_revision != Some(self.preview.revision);
        let preview_now = self.epoch.elapsed().as_micros() as u64;
        let preview_frames: Vec<viz_dmx::UniverseFrame> = self
            .preview
            .universes
            .iter()
            .filter(|universe| preview_universes.contains(&universe.universe))
            .map(|universe| {
                let mut slots = [0_u8; viz_dmx::DMX_SLOTS];
                let length = universe.slots.len().min(viz_dmx::DMX_SLOTS);
                slots[..length].copy_from_slice(&universe.slots[..length]);
                viz_dmx::UniverseFrame {
                    logical_universe: universe.universe,
                    slots,
                    received_micros: preview_now,
                    stale: false,
                }
            })
            .collect();

        // The desk's own output, for a renderer drawing inside the desk's window. Decoded through
        // exactly the path a real packet takes, so nothing downstream can tell the difference —
        // the numbers are the same numbers, read from the desk instead of heard from the wire.
        // Applied on every read rather than when a revision moves. The desk's output revision
        // counts structural changes, not frames: it sits still while every level in the show is
        // moving, so gating on it showed the rig as it was at the moment the pane opened and never
        // again.
        if let (Some(output), Some(decoder), Some(scene)) =
            (self.desk_output.take(), &mut self.decoder, &self.scene)
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_micros() as u64)
                .unwrap_or_default();
            let frames: Vec<viz_dmx::UniverseFrame> = output
                .universes
                .iter()
                .map(|universe| {
                    let mut slots = [0_u8; 512];
                    let length = universe.slots.len().min(512);
                    slots[..length].copy_from_slice(&universe.slots[..length]);
                    viz_dmx::UniverseFrame {
                        logical_universe: universe.universe,
                        slots,
                        received_micros: now,
                        stale: false,
                    }
                })
                .collect();
            if !frames.is_empty() {
                decoder.apply(
                    scene,
                    &frames,
                    &mut self.values,
                    self.epoch.elapsed().as_secs_f32(),
                );
            }
            // The preload sits on top of the live picture rather than replacing it: a fixture
            // nobody preloaded goes on showing what it is doing. This also applies when the desk
            // has no patched universes; unpatched fixtures are still part of the show.
            if self.following_preload {
                let overlay: Vec<crate::preload_overlay::PreloadValue> = self
                    .preload_projection
                    .fixture_values
                    .iter()
                    .filter_map(|entry| match entry.value {
                        crate::wire::PreloadAttributeValue::Normalized(value) => {
                            Some(crate::preload_overlay::PreloadValue {
                                fixture_id: entry.fixture_id,
                                attribute: entry.attribute.clone(),
                                value,
                            })
                        }
                        crate::wire::PreloadAttributeValue::Other => None,
                    })
                    .collect();
                crate::preload_overlay::apply(scene, &overlay, &mut self.values);
            }
            // An empty output snapshot is still an authoritative source frame. A show may retain
            // a complete unpatched rig, and the Stage must keep presenting it instead of treating
            // the absence of network universes as the absence of the desk.
            stamp_desk_output_frame(&mut self.values, &mut self.value_frame, now);
            events.push(ProviderEvent::Values(Box::new(self.values.clone())));
        }

        if let (Some(receivers), Some(decoder), Some(scene)) =
            (&self.receivers, &mut self.decoder, &self.scene)
        {
            let frames = receivers.drain_changed();
            let elapsed = self.epoch.elapsed().as_secs_f32();
            if !frames.is_empty() {
                decoder.apply(scene, &frames, &mut self.values, elapsed);
            }

            if preview_moved && !preview_frames.is_empty() {
                let now = preview_now;
                decoder.apply(scene, &preview_frames, &mut self.values, elapsed);
                self.applied_preview_revision = Some(self.preview.revision);
                // A preview change is not a packet, so it gets its own frame stamp; without one
                // the host would never present it, because nothing arrived from the network.
                self.value_frame += 1;
                self.values.newest_input_micros = now;
                self.values.frame = self.value_frame;
                events.push(ProviderEvent::Values(Box::new(self.values.clone())));
            } else if preview_moved {
                // Nothing to apply, but the revision is accounted for so a later change is still
                // seen as one.
                self.applied_preview_revision = Some(self.preview.revision);
            }

            // A held look still arrives at full rate, so the newest accepted packet — not the
            // newest content change — is what the presented frame is measured against.
            let newest = receivers.newest_accepted_micros();
            if newest > self.reported_input_micros {
                self.reported_input_micros = newest;
                self.value_frame += 1;
                self.values.newest_input_micros = newest;
                self.values.frame = self.value_frame;
                events.push(ProviderEvent::Values(Box::new(self.values.clone())));
            }
            let mut diagnostics = self.diagnostics.clone();
            diagnostics.inputs = receivers.status();
            diagnostics.universes = receivers.universes();
            diagnostics.preview_universes = preview_universes.into_iter().collect();
            self.diagnostics = diagnostics.clone();
            events.push(ProviderEvent::Diagnostics(Box::new(diagnostics)));
        }
        events
    }

    fn request_resync(&mut self) {
        let _ = self.commands.send(Command::Resync);
    }

    fn update_renderer_settings(&mut self, intent: viz_scene::RendererSettingsIntent) {
        let _ = self.commands.send(Command::RendererSettings(intent));
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(mut receivers) = self.receivers.take() {
            receivers.shutdown();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// The connection thread: HTTP reads and the event subscription, never live values.
fn run(
    connection: DeskConnection,
    outbox: Sender<Message>,
    mut orders: tokio::sync::mpsc::UnboundedReceiver<Command>,
    stop: Arc<AtomicBool>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = outbox.send(Message::Connection(ConnectionState::Failed {
                boundary: "connection runtime".into(),
                detail: error.to_string(),
            }));
            return;
        }
    };
    runtime.block_on(async move {
        let mut backoff = connection.retry;
        loop {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            if orders.is_closed() {
                return;
            }
            match connect_once(&connection, &outbox, &mut orders, &stop).await {
                Ok(()) => {
                    backoff = connection.retry;
                }
                Err(error) => {
                    let _ = outbox.send(Message::Connection(ConnectionState::Failed {
                        boundary: error.boundary.into(),
                        detail: error.detail.clone(),
                    }));
                    if !error.retryable {
                        // Still retry, but slowly: the operator may fix the desk at any time.
                        backoff = (backoff * 2).min(Duration::from_secs(15));
                    }
                }
            }
            // Sleep in short slices so shutdown is prompt even during a long backoff.
            let deadline = std::time::Instant::now() + backoff;
            while std::time::Instant::now() < deadline {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            backoff = (backoff + connection.retry).min(Duration::from_secs(10));
        }
    });
}

async fn connect_once(
    connection: &DeskConnection,
    outbox: &Sender<Message>,
    orders: &mut tokio::sync::mpsc::UnboundedReceiver<Command>,
    stop: &AtomicBool,
) -> Result<(), ProviderError> {
    let endpoint = format!("http://{}:{}", connection.host, connection.port);
    let _ = outbox.send(Message::Connection(ConnectionState::Resolving {
        endpoint: endpoint.clone(),
    }));
    let mut client = DeskClient::new(&connection.host, connection.port)?;
    let readiness = client.readiness().await?;
    if readiness.status != "ready" {
        // A source with nothing loaded is not a broken source. An editor whose operator has not
        // opened a document yet answers exactly like this, and saying "readiness failed" about it
        // is both wrong and alarming — so it is reported as what it is and retried quietly.
        if readiness.active_show.is_none() && readiness.active_show_error.is_none() {
            let _ = outbox.send(Message::Connection(ConnectionState::WaitingForShow {
                endpoint: endpoint.clone(),
            }));
            return Ok(());
        }
        return Err(ProviderError::new(
            "server readiness",
            format!(
                "the desk reports {}{}",
                readiness.status,
                readiness
                    .active_show_error
                    .map(|error| format!(": {error}"))
                    .unwrap_or_default()
            ),
            true,
        ));
    }

    let _ = outbox.send(Message::Connection(ConnectionState::Authenticating {
        endpoint: endpoint.clone(),
    }));
    let read_only = client.open_session(&connection.user).await?;

    let _ = outbox.send(Message::Connection(ConnectionState::LoadingScene {
        endpoint: endpoint.clone(),
    }));
    let (plan, mappings, mut diagnostics) = read_scene(&client, &endpoint, connection).await?;
    if !read_only {
        diagnostics.warnings.push(
            "This desk does not support the read-only visualizer role; the renderer still never \
             writes."
                .into(),
        );
    }
    if let Some(view) = read_view(&client, connection).await {
        let _ = outbox.send(Message::View(Box::new(view)));
    }
    let revision = plan.scene.revision;
    let _ = outbox.send(Message::Scene {
        plan: Box::new(plan.scene),
        bindings: plan.bindings,
        external_camera: plan.external_camera,
        mappings,
        diagnostics: Box::new(diagnostics),
    });
    let _ = outbox.send(Message::Connection(ConnectionState::Connected {
        endpoint: endpoint.clone(),
        revision,
    }));
    // A planning window may already be driving the rig before the renderer ever connected, and
    // reconnecting must not lose the look. A desk answers 404 here and nothing is merged.
    if let Some(preview) = client.preview_values().await {
        let _ = outbox.send(Message::Preview(Box::new(preview)));
    }
    if let Some(selection) = client.selection().await {
        let _ = outbox.send(Message::Selection(Box::new(selection)));
    }
    watch(&client, &endpoint, connection, outbox, orders, stop).await;
    client.close_session().await;
    Ok(())
}

/// One coherent read of the show: the scene, where its universes arrive, and what to say about
/// both. The first connection and every later re-read go through here, so a delta cannot drift
/// from the snapshot it is amending.
async fn read_scene(
    client: &DeskClient,
    endpoint: &str,
    connection: &DeskConnection,
) -> Result<
    (
        viz_project::ScenePlan,
        Vec<viz_dmx::InputMapping>,
        ProviderDiagnostics,
    ),
    ProviderError,
> {
    let models = read_models(client, endpoint).await?;
    // Output routes are stored as show objects of kind `route`.
    let route_objects = client.objects("route").await?.objects;
    let input_objects = client
        .objects(viz_document::LIVE_DMX_INPUT_KIND)
        .await
        .map(|collection| collection.objects)
        .unwrap_or_default();
    let plan = scene_build::build(&models);
    let (mappings, input_warnings) = routes::apply_document_inputs(
        routes::mappings(&route_objects, connection.bind_interface),
        &input_objects,
        connection.bind_interface,
    );
    let mut diagnostics = ProviderDiagnostics {
        endpoint: endpoint.to_owned(),
        resolved_address: endpoint.to_owned(),
        authenticated: client.token().is_some(),
        show_identity: format!("{} ({})", models.show_name, models.patch.show_id),
        scene_revision: plan.scene.revision,
        interface: connection
            .bind_interface
            .map(|address| address.to_string())
            .unwrap_or_else(|| "all interfaces".into()),
        inputs: Vec::new(),
        universes: Vec::new(),
        preview_universes: Vec::new(),
        warnings: plan.warnings.clone(),
    };
    diagnostics.warnings.extend(input_warnings);
    if mappings.is_empty() {
        diagnostics.warnings.push(
            "The show configures no output routes; listening on the Art-Net and sACN defaults."
                .into(),
        );
    }
    Ok((plan, mappings, diagnostics))
}

async fn read_models(client: &DeskClient, endpoint: &str) -> Result<DeskReadModels, ProviderError> {
    let patch = client.patch().await?;
    let stage_layout = client
        .objects("stage_layout")
        .await?
        .objects
        .into_iter()
        .find(|object| object.id == "main")
        .and_then(|object| serde_json::from_value::<StageLayoutBody>(object.body).ok())
        .unwrap_or_default();
    let venue_objects = client
        .objects("venue")
        .await
        .map(|collection| collection.objects)
        .unwrap_or_default();
    let media_servers = optional_objects(client, "media_server").await;
    let media_fallback_assets = optional_objects(client, "media_fallback_asset").await;
    let media_sources = optional_objects(client, "media_source").await;
    let led_module_types = optional_objects(client, "led_module_type").await;
    let media_surfaces = optional_objects(client, "media_surface").await;
    let media_projectors = optional_objects(client, "media_projector").await;
    Ok(DeskReadModels {
        show_name: patch.show_id.to_string(),
        server_identity: endpoint.to_owned(),
        patch,
        stage_layout,
        venue_objects,
        media_servers,
        media_fallback_assets,
        media_sources,
        led_module_types,
        media_surfaces,
        media_projectors,
    })
}

async fn optional_objects(client: &DeskClient, kind: &str) -> Vec<crate::wire::ObjectRecord> {
    client
        .objects(kind)
        .await
        .map(|collection| collection.objects)
        .unwrap_or_default()
}

/// Subscribe to revisioned configuration changes and apply them to the running scene.
///
/// A configuration change is re-read over the connection that is already open and sent on as a
/// delta: the socket stays up, the session stays open, the receivers keep delivering, and the
/// values are carried across. Only a change of show — or a re-read that fails — goes back through
/// the full reconnect, because then the values genuinely belong to something else.
async fn watch(
    client: &DeskClient,
    endpoint: &str,
    connection: &DeskConnection,
    outbox: &Sender<Message>,
    orders: &mut tokio::sync::mpsc::UnboundedReceiver<Command>,
    stop: &AtomicBool,
) {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL;

    let Some(token) = client.token() else {
        return;
    };
    let url = endpoint
        .replacen("http://", "ws://", 1)
        .replacen("https://", "wss://", 1);
    let Ok(mut request) = format!("{url}/api/v2/events").into_client_request() else {
        return;
    };
    let protocols = format!("light.events.v2, light.v2, light.token.{token}");
    if let Ok(value) = protocols.parse() {
        request.headers_mut().insert(SEC_WEBSOCKET_PROTOCOL, value);
    }
    let Ok((mut socket, _)) = tokio_tungstenite::connect_async(request).await else {
        let _ = outbox.send(Message::Diagnostics(Box::new(ProviderDiagnostics {
            endpoint: endpoint.to_owned(),
            warnings: vec![
                "The configuration event stream is unavailable; press R to resynchronise.".into(),
            ],
            ..ProviderDiagnostics::default()
        })));
        return;
    };

    // A desk delivers events to a subscriber, not to whoever opens the socket: the first frame
    // has to say what this client wants, or the desk answers with an error and closes. Everything
    // the renderer follows is a projection of the show or of the desk's own configuration.
    let subscribe = serde_json::json!({
        "type": "subscribe",
        "filter": {"capabilities": ["show", "desk"], "classes": ["projection"]},
        "capacity": 128,
        "rate_limits": [],
    });
    if socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            subscribe.to_string().into(),
        ))
        .await
        .is_err()
    {
        let _ = outbox.send(Message::Connection(ConnectionState::Stale {
            endpoint: endpoint.to_owned(),
            reason: "the configuration event stream would not take a subscription".into(),
        }));
        return;
    }
    // Close the GET-to-WebSocket subscription gap: a settings write between the initial scene
    // read and this subscription is still observed as the newest authoritative snapshot.
    if let Some(settings) = client.renderer_settings().await {
        let _ = outbox.send(Message::RendererSettings(Box::new(settings)));
    }

    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if connection.values_from_desk_output {
            if let Some(output) = client.output_dmx().await {
                let _ = outbox.send(Message::DeskOutput(Box::new(output)));
            }
            if let Some(preload) = client.preload_projection().await {
                let _ = outbox.send(Message::Preload2(Box::new(preload)));
            }
        }
        let poll = if connection.values_from_desk_output {
            // Stage's source contract is 10 Hz. Rendering remains independent and can interpolate
            // physical motion between these authoritative desk-output snapshots.
            100
        } else {
            500
        };
        let next = tokio::select! {
            command = orders.recv() => {
                match command {
                    None => return,
                    Some(Command::Resync) => {
                        let _ = outbox.send(Message::Resync("operator requested".into()));
                        return;
                    }
                    Some(Command::RendererSettings(intent)) => {
                        if let Ok(update) = client.update_renderer_settings(&intent).await {
                            let _ = outbox.send(Message::RendererSettings(Box::new(update)));
                        }
                        continue;
                    }
                }
            }
            message = socket.next() => message,
            _ = tokio::time::sleep(Duration::from_millis(poll)) => continue,
        };
        let Some(Ok(message)) = next else {
            let _ = outbox.send(Message::Connection(ConnectionState::Stale {
                endpoint: endpoint.to_owned(),
                reason: "the configuration event stream closed".into(),
            }));
            return;
        };
        let Ok(text) = message.into_text() else {
            continue;
        };
        let Some(frame) = crate::wire::EventFrame::parse(&text) else {
            continue;
        };
        if frame.kind == "renderer_settings_changed" {
            if let Some(settings) = frame.renderer_settings {
                let _ = outbox.send(Message::RendererSettings(Box::new(settings)));
            } else if let Some(settings) = client.renderer_settings().await {
                let _ = outbox.send(Message::RendererSettings(Box::new(settings)));
            }
            continue;
        }
        // A different show is a different scene: its values, mappings and identity all change
        // together, so it is staged as a whole rather than merged into what is displayed.
        if replaces_the_show(&frame.kind) {
            let _ = outbox.send(Message::Resync(format!("{} changed the scene", frame.kind)));
            return;
        }
        // A preview change is one small re-read, not a scene resynchronisation: the rig has not
        // moved, only what it is being lit with.
        if frame.kind == "preview_values_changed" {
            if let Some(preview) = client.preview_values().await {
                let _ = outbox.send(Message::Preview(Box::new(preview)));
            }
            continue;
        }
        if frame.kind == "visualizer_selection_changed" {
            if let Some(selection) = client.selection().await {
                let _ = outbox.send(Message::Selection(Box::new(selection)));
            }
            continue;
        }
        // The desk moving a camera is not a change of rig: nothing is re-read but the view.
        if view_affecting(&frame.kind) {
            if let Some(view) = read_view(client, connection).await {
                let _ = outbox.send(Message::View(Box::new(view)));
            }
            continue;
        }
        if !scene_affecting(&frame.kind) {
            continue;
        }
        match read_scene(client, endpoint, connection).await {
            Ok((plan, mappings, diagnostics)) => {
                let _ = outbox.send(Message::Delta {
                    plan: Box::new(plan.scene),
                    bindings: plan.bindings,
                    external_camera: plan.external_camera,
                    mappings,
                    diagnostics: Box::new(diagnostics),
                });
            }
            Err(error) => {
                // The re-read is the only thing that failed; the displayed scene is still the
                // last good one, so this asks for the full path rather than pretending.
                let _ = outbox.send(Message::Resync(format!(
                    "{} changed the scene, and re-reading it failed: {}",
                    frame.kind, error.detail
                )));
                return;
            }
        }
    }
}

fn stamp_desk_output_frame(values: &mut SceneValues, value_frame: &mut u64, now: u64) {
    *value_frame = value_frame.saturating_add(1);
    values.newest_input_micros = now;
    values.frame = *value_frame;
}

fn apply_selection(values: &mut SceneValues, selection: &crate::wire::SelectionSnapshot) {
    values.selected_fixtures = selection.selected_fixture_ids.iter().copied().collect();
}
/// Which universes the planning window drives, given what the network has ever delivered.
///
/// Stated as a function of two values so the rule can be checked on its own: it is the whole of
/// the precedence contract and the one thing here that is easy to get subtly wrong.
fn editor_driven(
    preview: &crate::wire::PreviewSnapshot,
    real: &std::collections::BTreeSet<u16>,
) -> std::collections::BTreeSet<u16> {
    preview
        .universes
        .iter()
        .map(|universe| universe.universe)
        .filter(|universe| !real.contains(universe))
        .collect()
}

/// Event kinds that may carry a new view for this renderer.
///
/// The desk publishes its view under its own configuration capability, so a renderer hears it on
/// the stream it is already following. Re-reading one small object on any desk-configuration
/// change is cheaper than a second subscription, and the view is only applied when its revision
/// actually moved.
fn view_affecting(kind: &str) -> bool {
    matches!(
        kind,
        "server_configuration_changed" | "visualizer_view_changed"
    )
}

/// The desk's view for this renderer's target.
///
/// A desk with nothing to say — including one too old to have a view at all — leaves the
/// operator's own selection alone, so this answers `None` rather than a default that would take
/// the camera away from them.
async fn read_view(client: &DeskClient, connection: &DeskConnection) -> Option<DeskView> {
    let snapshot = client.visualizer_views().await?;
    let view = snapshot
        .views
        .into_iter()
        .find(|view| view.target.eq_ignore_ascii_case(&connection.target))?;
    Some(DeskView {
        mode: viz_scene::ViewMode::from_wire(&view.mode)?,
        quality: viz_scene::RenderQuality::from_wire(&view.quality)
            .unwrap_or(viz_scene::RenderQuality::High),
        camera: view.camera.map(|camera| viz_scene::Camera {
            position: camera.position.into(),
            target: camera.target.into(),
            up: camera.up.into(),
            fov_degrees: camera.fov_degrees,
            orthographic_size: camera.orthographic_size,
        }),
        exposure: view.exposure,
        ambient: view.ambient,
        revision: view.revision,
        physics_reset_generation: view.physics_reset_generation,
    })
}

/// Event kinds that change what the renderer must display.
/// A desk names the change in the payload of its typed envelope; the planning window names it on
/// its own and uses the desk's older spellings. Both vocabularies are listed, because the renderer
/// connects to both and neither is wrong.
fn scene_affecting(kind: &str) -> bool {
    matches!(
        kind,
        "show_patch_changed"
            | "show_objects_changed"
            | "show_object_changed"
            | "output_route_changed"
            | "fixture_library_changed"
    ) || replaces_the_show(kind)
}

/// Event kinds that put a different show in front of the renderer.
/// These are the ones the values cannot survive: the fixtures they belonged to are gone, and the
/// universes they arrived on may now mean something else. A library change that is not an open —
/// a rename, a rollback — is rare enough that staging the show whole costs nothing worth saving.
fn replaces_the_show(kind: &str) -> bool {
    matches!(
        kind,
        "show_library_changed" | "active_show_changed" | "show_loaded"
    )
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod provider_tests;
