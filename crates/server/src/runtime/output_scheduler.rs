//! Network-output scheduling and safe shutdown for the server runtime.

use super::{OutputControl, PersistedOutputRuntime, playback_service};
use light_application::{
    PlaybackOperation, PlaybackService, PlaybackShowScope, PlaybackUnitOfWork,
    automatic_playback_events,
};
use light_control::{SmpteTimecode, TimecodeRouter};
use light_core::Universe;
use light_engine::{Engine, EngineError, RenderOptions, RenderResult};
use light_output::{DmxFrame, NetworkOutput, OutputHealth, Protocol, run_scheduler_dynamic};
use light_show::ShowEntry;
use parking_lot::{Mutex, RwLock};
use std::{
    collections::HashMap,
    io,
    net::IpAddr,
    sync::{Arc, atomic::AtomicU16},
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

type OutputSequences = HashMap<(Protocol, Universe), u8>;
type SharedSequences = Arc<tokio::sync::Mutex<OutputSequences>>;

pub(super) struct Config {
    pub bind_ip: IpAddr,
    pub engine: Arc<Engine>,
    pub health: Arc<std::sync::Mutex<OutputHealth>>,
    pub rate: Arc<AtomicU16>,
    pub timecode: Arc<Mutex<TimecodeRouter>>,
    pub cancellation: CancellationToken,
    pub persisted_runtime: PersistedOutputRuntime,
    pub playback_service: PlaybackService,
    pub active_show: Arc<RwLock<Option<ShowEntry>>>,
    pub activation_lock: Arc<tokio::sync::Mutex<()>>,
    pub test_bench: bool,
}

pub(super) struct OutputScheduler {
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
    pub(super) task: JoinHandle<()>,
}

struct SharedResources {
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
}

#[derive(Clone)]
struct Runtime {
    pub(super) engine: Arc<Engine>,
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
    pub(super) timecode: Arc<Mutex<TimecodeRouter>>,
    pub(super) playback_service: PlaybackService,
    pub(super) active_show: Arc<RwLock<Option<ShowEntry>>>,
    pub(super) activation_lock: Arc<tokio::sync::Mutex<()>>,
    pub(super) cancellation: CancellationToken,
}

pub(super) async fn start(config: Config) -> anyhow::Result<OutputScheduler> {
    let resources = SharedResources::create(&config).await?;
    let runtime = resources.runtime(&config);
    let task = spawn(runtime, config.rate, config.health, config.test_bench);
    Ok(resources.scheduler(task))
}

async fn bind_output(bind_ip: IpAddr) -> anyhow::Result<Arc<NetworkOutput>> {
    let cid = *Uuid::new_v4().as_bytes();
    Ok(Arc::new(NetworkOutput::bind(bind_ip, cid, "Light").await?))
}

fn create_control(runtime: &PersistedOutputRuntime) -> Arc<Mutex<OutputControl>> {
    Arc::new(Mutex::new(OutputControl {
        options: RenderOptions {
            grand_master: runtime.grand_master,
            blackout: runtime.blackout,
            control_loss_progress: None,
        },
        ..OutputControl::default()
    }))
}

fn spawn(
    runtime: Runtime,
    rate: Arc<AtomicU16>,
    health: Arc<std::sync::Mutex<OutputHealth>>,
    test_bench: bool,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        run(&runtime, rate, health, test_bench).await;
        shut_down_safely(&runtime).await;
    })
}

async fn run(
    runtime: &Runtime,
    rate: Arc<AtomicU16>,
    health: Arc<std::sync::Mutex<OutputHealth>>,
    test_bench: bool,
) {
    if test_bench {
        runtime.cancellation.cancelled().await;
        return;
    }
    let cancellation = runtime.cancellation.clone();
    run_scheduler_dynamic(rate, cancellation, health, || render_tick(runtime.clone())).await;
}

async fn render_tick(runtime: Runtime) -> io::Result<u64> {
    update_timecode(&runtime);
    let options = runtime.control.lock().render_options();
    let rendered = {
        let _activation = runtime.activation_lock.clone().lock_owned().await;
        render_with_playback_events(
            &runtime.engine,
            &runtime.active_show,
            &runtime.playback_service,
            options,
        )
        .map_err(io::Error::other)?
    };
    let frames = output_frames(&mut runtime.control.lock(), rendered.universes);
    runtime
        .output
        .send_routes(
            &rendered.routes,
            &frames,
            &rendered.patched_slots,
            &mut *runtime.sequences.lock().await,
        )
        .await
}

pub(super) fn render_with_playback_events(
    engine: &Engine,
    active_show: &RwLock<Option<ShowEntry>>,
    service: &PlaybackService,
    options: RenderOptions,
) -> Result<RenderResult, EngineError> {
    service
        .run_unit_of_work(AutomaticRender {
            engine,
            active_show,
            options,
        })
        .output
}

struct AutomaticRender<'a> {
    engine: &'a Engine,
    active_show: &'a RwLock<Option<ShowEntry>>,
    options: RenderOptions,
}

impl PlaybackUnitOfWork for AutomaticRender<'_> {
    type Output = Result<RenderResult, EngineError>;

    fn execute(self) -> PlaybackOperation<Self::Output> {
        let mut rendered = match self.engine.render(self.options) {
            Ok(rendered) => rendered,
            Err(error) => return PlaybackOperation::new(Err(error)),
        };
        let transitions = std::mem::take(&mut rendered.automatic_playback_transitions);
        let events = self
            .active_show
            .read()
            .as_ref()
            .map(|show| {
                playback_service::automatic_projection_changes(
                    self.engine,
                    PlaybackShowScope {
                        show_id: show.id.0,
                        show_revision: rendered.revision,
                    },
                    transitions,
                )
            })
            .map(automatic_playback_events)
            .unwrap_or_default();
        PlaybackOperation::with_events(Ok(rendered), events)
    }
}

fn update_timecode(runtime: &Runtime) {
    let current = runtime.timecode.lock().poll_loss().cloned();
    runtime
        .engine
        .set_timecode_frame(current.as_ref().map(timecode_frame));
}

fn timecode_frame(timecode: &SmpteTimecode) -> u64 {
    let fps = u64::from(timecode.rate.nominal_frames());
    let seconds = u64::from(timecode.hours) * 3600
        + u64::from(timecode.minutes) * 60
        + u64::from(timecode.seconds);
    seconds * fps + u64::from(timecode.frames)
}

fn output_frames(
    control: &mut OutputControl,
    mut rendered: HashMap<Universe, DmxFrame>,
) -> HashMap<Universe, DmxFrame> {
    if control.hold {
        return control.last_frames.clone();
    }
    apply_raw_overrides(&mut rendered, &control.raw_overrides);
    control.last_frames.clone_from(&rendered);
    rendered
}

fn apply_raw_overrides(
    frames: &mut HashMap<Universe, DmxFrame>,
    overrides: &HashMap<(Universe, light_core::DmxAddress), u8>,
) {
    for (&(universe, address), &value) in overrides {
        if let Some(frame) = frames.get_mut(&universe) {
            frame[usize::from(address - 1)] = value;
        }
    }
}

async fn shut_down_safely(runtime: &Runtime) {
    let routes = send_safe_frame(runtime)
        .await
        .unwrap_or_else(|| runtime.engine.output_routes());
    let _ = runtime
        .output
        .terminate_routes(&routes, &mut *runtime.sequences.lock().await)
        .await;
}

async fn send_safe_frame(runtime: &Runtime) -> Option<Arc<[light_output::OutputRoute]>> {
    let options = safe_shutdown_options(&runtime.control);
    let safe = runtime.engine.render(options).ok()?;
    let _ = runtime
        .output
        .send_routes(
            &safe.routes,
            &safe.universes,
            &safe.patched_slots,
            &mut *runtime.sequences.lock().await,
        )
        .await;
    Some(safe.routes)
}

fn safe_shutdown_options(control: &Mutex<OutputControl>) -> RenderOptions {
    let mut options = control.lock().options;
    options.control_loss_progress = Some(1.0);
    options
}

impl OutputScheduler {
    pub(super) fn network_output(&self) -> Arc<NetworkOutput> {
        Arc::clone(&self.output)
    }

    pub(super) fn sequences(&self) -> SharedSequences {
        Arc::clone(&self.sequences)
    }

    pub(super) fn control(&self) -> Arc<Mutex<OutputControl>> {
        Arc::clone(&self.control)
    }

    pub(super) async fn wait(self) {
        let _ = self.task.await;
    }
}

impl SharedResources {
    async fn create(config: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            output: bind_output(config.bind_ip).await?,
            sequences: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            control: create_control(&config.persisted_runtime),
        })
    }

    fn runtime(&self, config: &Config) -> Runtime {
        Runtime {
            engine: Arc::clone(&config.engine),
            output: Arc::clone(&self.output),
            sequences: Arc::clone(&self.sequences),
            control: Arc::clone(&self.control),
            timecode: Arc::clone(&config.timecode),
            playback_service: config.playback_service.clone(),
            active_show: Arc::clone(&config.active_show),
            activation_lock: Arc::clone(&config.activation_lock),
            cancellation: config.cancellation.clone(),
        }
    }

    fn scheduler(self, task: JoinHandle<()>) -> OutputScheduler {
        OutputScheduler {
            output: self.output,
            sequences: self.sequences,
            control: self.control,
            task,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_control::FrameRate;

    #[test]
    fn frame_selection_applies_overrides_and_reuses_held_frames() {
        let mut control = OutputControl::default();
        control.raw_overrides.insert((1, 2), 77);
        let live = output_frames(&mut control, frames_with_value(1, 10));
        assert_eq!(live[&1][0], 10);
        assert_eq!(live[&1][1], 77);

        control.hold = true;
        let held = output_frames(&mut control, frames_with_value(1, 99));
        assert_eq!(held, live);
    }

    #[test]
    fn timecode_frame_uses_the_nominal_frame_rate() {
        let timecode = SmpteTimecode {
            hours: 1,
            minutes: 2,
            seconds: 3,
            frames: 4,
            rate: FrameRate::Fps25,
            source: "test".into(),
            received_at: chrono::Utc::now(),
        };
        assert_eq!(timecode_frame(&timecode), 93_079);
    }

    fn frames_with_value(universe: Universe, value: u8) -> HashMap<Universe, DmxFrame> {
        let mut frame = [0; light_output::DMX_SLOTS];
        frame[0] = value;
        HashMap::from([(universe, frame)])
    }
}
