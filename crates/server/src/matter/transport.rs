//! Commissionable Matter-over-IP transport for the playback bridge.
//!
//! `MatterTransport` owns a dedicated thread because `rs-matter` deliberately supports
//! single-threaded executors and its data-model futures do not need to be `Send`. The server
//! communicates with that thread through bounded, non-blocking commands: reconciled playback
//! values flow into Matter subscriptions and controller writes flow back to the server's normal
//! playback dispatcher.

use super::{MAX_MATTER_LEVEL, MatterPlaybackLight, MatterPlaybackWrite};
use embassy_futures::select::{Either, select, select3};
use futures_lite::future::block_on;
use parking_lot::{Mutex, RwLock};
use rand::RngCore;
use rs_matter::crypto::{Crypto, default_crypto};
use rs_matter::dm::clusters::decl::bridged_device_basic_information as bridged_info;
use rs_matter::dm::clusters::decl::level_control;
use rs_matter::dm::clusters::decl::on_off;
use rs_matter::dm::clusters::desc::{self, ClusterHandler as _};
use rs_matter::dm::clusters::groups::{self, ClusterHandler as _};
use rs_matter::dm::devices::test::{DAC_PRIVKEY, TEST_DEV_ATT, TEST_PID, TEST_VID};
use rs_matter::dm::devices::{DEV_TYPE_AGGREGATOR, DEV_TYPE_BRIDGED_NODE, DEV_TYPE_DIMMABLE_LIGHT};
use rs_matter::dm::endpoints;
use rs_matter::dm::networks::SysNetifs;
use rs_matter::dm::networks::eth::EthNetwork;
use rs_matter::dm::{
    Async, AttrChangeNotifier, Cluster, Dataver, Endpoint, EpClMatcher, InvokeContext, Node,
    ReadContext, WriteContext,
};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::im::{EthInteractionModelState, InteractionModel};
use rs_matter::pairing::qr::{CommFlowType, QrPayload, no_optional_data};
use rs_matter::pairing::{DiscoveryCapabilities, qr::NoOptionalData};
use rs_matter::persist::DirKvBlobStore;
use rs_matter::respond::DefaultResponder;
use rs_matter::sc::pase::{
    MAX_COMM_WINDOW_TIMEOUT_SECS, Spake2pVerifierPassword, Spake2pVerifierPasswordRef,
};
use rs_matter::tlv::{Nullable, TLVBuilderParent, Utf8StrBuilder};
use rs_matter::transport::MATTER_SOCKET_BIND_ADDR;
use rs_matter::transport::exchange::MatterBuffers;
use rs_matter::transport::network::MatterLocalService;
use rs_matter::transport::network::mdns::builtin::{BuiltinMdns, Host};
use rs_matter::transport::network::mdns::{
    MDNS_IPV4_BROADCAST_ADDR, MDNS_IPV6_BROADCAST_ADDR, MDNS_PORT, MDNS_SOCKET_DEFAULT_BIND_ADDR,
};
use rs_matter::transport::network::{Address, Ipv4Addr, Ipv6Addr, NetworkReceive, NetworkSend};
use rs_matter::utils::select::Coalesce;
use rs_matter::{BasicCommData, MATTER_PORT, Matter, clusters, devices, root_endpoint, with};
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const AGGREGATOR_ENDPOINT_ID: u16 = 0xfffe;
const IDENTITY_FILE: &str = "identity.json";
const COMPOSITION_FILE: &str = "composition.json";
const KV_DIRECTORY: &str = "kv";
const START_TIMEOUT: Duration = Duration::from_secs(5);
const CONTROL_POLL_INTERVAL: Duration = Duration::from_millis(25);
const ROOT_ENDPOINT: Endpoint<'static> = root_endpoint!(eth);

/// User-facing pairing material. The passcode is represented only through the standard manual
/// code so callers do not need to understand Matter's passcode encoding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatterPairingData {
    pub qr_code: String,
    pub manual_code: String,
    pub discriminator: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatterTransportLifecycle {
    #[default]
    Disabled,
    Starting,
    Running,
    Failed,
}

/// Truthful network status for the production transport. `commissionable` is true only while a
/// commissioning window is advertised by a successfully started IP transport.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatterTransportSnapshot {
    pub lifecycle: MatterTransportLifecycle,
    pub network_running: bool,
    pub commissioned: bool,
    pub commissioning_window_open: bool,
    pub commissionable: bool,
    pub endpoint_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairing: Option<MatterPairingData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl Default for MatterTransportSnapshot {
    fn default() -> Self {
        Self {
            lifecycle: MatterTransportLifecycle::Disabled,
            network_running: false,
            commissioned: false,
            commissioning_window_open: false,
            commissionable: false,
            endpoint_count: 0,
            pairing: None,
            last_error: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatterRemoteWrite {
    pub endpoint_id: u16,
    pub write: MatterPlaybackWrite,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct MatterIdentity {
    passcode: u32,
    discriminator: u16,
    serial: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct EndpointShape {
    endpoint_id: u16,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TransportLight {
    endpoint_id: u16,
    name: String,
    on: bool,
    level: u8,
}

impl From<&MatterPlaybackLight> for TransportLight {
    fn from(light: &MatterPlaybackLight) -> Self {
        Self {
            endpoint_id: light.endpoint_id,
            name: matter_string(&light.name, 32),
            on: light.on,
            level: light.level.min(MAX_MATTER_LEVEL),
        }
    }
}

enum ControlCommand {
    Reconcile(Vec<TransportLight>),
    Shutdown,
}

struct RuntimeHandle {
    shape: Vec<EndpointShape>,
    control: Sender<ControlCommand>,
    join: JoinHandle<()>,
}

struct TransportState {
    runtime: Option<RuntimeHandle>,
}

/// Lifecycle manager used by the HTTP/server layer.
///
/// Call [`Self::reconcile`] whenever the adapter snapshot changes. Changes limited to OnOff and
/// CurrentLevel are published in place. Adding, removing, or renaming an endpoint restarts only
/// this Matter service, preserving fabrics in `<data-dir>/matter/kv`.
pub struct MatterTransport {
    storage_dir: PathBuf,
    state: Mutex<TransportState>,
    snapshot: Arc<RwLock<MatterTransportSnapshot>>,
    remote_writes_rx: Mutex<Receiver<MatterRemoteWrite>>,
    remote_writes_tx: Sender<MatterRemoteWrite>,
}

impl std::fmt::Debug for MatterTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MatterTransport")
            .field("storage_dir", &self.storage_dir)
            .field("snapshot", &self.snapshot())
            .finish_non_exhaustive()
    }
}

impl MatterTransport {
    /// Create a transport rooted below the server's desk data directory.
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        let (remote_writes_tx, remote_writes_rx) = mpsc::channel();
        Self {
            storage_dir: data_dir.as_ref().join("matter"),
            state: Mutex::new(TransportState { runtime: None }),
            snapshot: Arc::new(RwLock::new(MatterTransportSnapshot::default())),
            remote_writes_rx: Mutex::new(remote_writes_rx),
            remote_writes_tx,
        }
    }

    /// Reconcile enabled state, endpoint composition, and mirrored values.
    pub fn reconcile(
        &self,
        enabled: bool,
        lights: &[MatterPlaybackLight],
    ) -> MatterTransportSnapshot {
        let transport_lights = match validate_lights(lights) {
            Ok(lights) => lights,
            Err(error) => {
                self.stop();
                self.set_failed(error);
                return self.snapshot();
            }
        };
        if !enabled {
            self.stop();
            return self.snapshot();
        }

        let shape = transport_lights
            .iter()
            .map(|light| EndpointShape {
                endpoint_id: light.endpoint_id,
                name: light.name.clone(),
            })
            .collect::<Vec<_>>();
        let mut state = self.state.lock();
        let restart = state
            .runtime
            .as_ref()
            .is_none_or(|runtime| runtime.shape != shape || runtime.join.is_finished());
        if restart {
            stop_runtime(&mut state.runtime);
            self.start_locked(&mut state, shape, transport_lights);
        } else if let Some(runtime) = &state.runtime
            && runtime
                .control
                .send(ControlCommand::Reconcile(transport_lights))
                .is_err()
        {
            stop_runtime(&mut state.runtime);
            self.set_failed("Matter transport control channel closed".into());
        }
        drop(state);
        self.snapshot()
    }

    pub fn snapshot(&self) -> MatterTransportSnapshot {
        self.snapshot.read().clone()
    }

    /// Drain controller writes without blocking the server's output loop.
    pub fn drain_remote_writes(&self) -> Vec<MatterRemoteWrite> {
        let receiver = self.remote_writes_rx.lock();
        let mut writes = Vec::new();
        while let Ok(write) = receiver.try_recv() {
            writes.push(write);
        }
        writes
    }

    pub fn stop(&self) {
        let mut state = self.state.lock();
        stop_runtime(&mut state.runtime);
        *self.snapshot.write() = MatterTransportSnapshot::default();
    }

    fn start_locked(
        &self,
        state: &mut TransportState,
        shape: Vec<EndpointShape>,
        lights: Vec<TransportLight>,
    ) {
        let identity = match load_or_create_identity(&self.storage_dir) {
            Ok(identity) => identity,
            Err(error) => {
                self.set_failed(format!("Matter identity persistence failed: {error}"));
                return;
            }
        };
        let pairing = match pairing_data(&identity) {
            Ok(pairing) => pairing,
            Err(error) => {
                self.set_failed(format!("Matter pairing payload failed: {error}"));
                return;
            }
        };
        *self.snapshot.write() = MatterTransportSnapshot {
            lifecycle: MatterTransportLifecycle::Starting,
            endpoint_count: shape.len(),
            pairing: Some(pairing),
            ..MatterTransportSnapshot::default()
        };

        let (control_tx, control_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let storage_dir = self.storage_dir.clone();
        let shared_snapshot = self.snapshot.clone();
        let remote_writes = self.remote_writes_tx.clone();
        let thread_shape = shape.clone();
        let startup_error_tx = ready_tx.clone();
        let join = match thread::Builder::new()
            .name("tosklight-matter".into())
            .spawn(move || {
                let result = run_transport(
                    &storage_dir,
                    &identity,
                    &thread_shape,
                    lights,
                    remote_writes,
                    control_rx,
                    ready_tx,
                    &shared_snapshot,
                );
                if let Err(error) = result {
                    let message = error.to_string();
                    let _ = startup_error_tx.try_send(Err(message.clone()));
                    let mut snapshot = shared_snapshot.write();
                    snapshot.lifecycle = MatterTransportLifecycle::Failed;
                    snapshot.network_running = false;
                    snapshot.commissionable = false;
                    snapshot.commissioning_window_open = false;
                    snapshot.last_error = Some(message);
                }
            }) {
            Ok(join) => join,
            Err(error) => {
                self.set_failed(format!("could not spawn Matter transport thread: {error}"));
                return;
            }
        };

        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(ready)) => {
                {
                    let mut snapshot = self.snapshot.write();
                    snapshot.lifecycle = MatterTransportLifecycle::Running;
                    snapshot.network_running = true;
                    snapshot.commissioned = ready.commissioned;
                    snapshot.commissioning_window_open = ready.commissioning_window_open;
                    snapshot.commissionable = ready.commissioning_window_open;
                    snapshot.last_error = None;
                }
                state.runtime = Some(RuntimeHandle {
                    shape,
                    control: control_tx,
                    join,
                });
            }
            Ok(Err(error)) => {
                let _ = join.join();
                self.set_failed(error);
            }
            Err(_) => {
                let _ = control_tx.send(ControlCommand::Shutdown);
                let _ = join.join();
                self.set_failed("Matter transport startup timed out".into());
            }
        }
    }

    fn set_failed(&self, error: String) {
        let mut snapshot = self.snapshot.write();
        snapshot.lifecycle = MatterTransportLifecycle::Failed;
        snapshot.network_running = false;
        snapshot.commissionable = false;
        snapshot.commissioning_window_open = false;
        snapshot.last_error = Some(error);
    }
}

impl Drop for MatterTransport {
    fn drop(&mut self) {
        stop_runtime(&mut self.state.get_mut().runtime);
    }
}

fn stop_runtime(runtime: &mut Option<RuntimeHandle>) {
    if let Some(runtime) = runtime.take() {
        let _ = runtime.control.send(ControlCommand::Shutdown);
        let _ = runtime.join.join();
    }
}

fn validate_lights(lights: &[MatterPlaybackLight]) -> Result<Vec<TransportLight>, String> {
    let mut endpoints = BTreeSet::new();
    let mut result = Vec::with_capacity(lights.len());
    for light in lights {
        if light.endpoint_id == 0 || light.endpoint_id == AGGREGATOR_ENDPOINT_ID {
            return Err(format!(
                "Matter playback endpoint {} is reserved",
                light.endpoint_id
            ));
        }
        if !endpoints.insert(light.endpoint_id) {
            return Err(format!(
                "Matter playback endpoint {} is duplicated",
                light.endpoint_id
            ));
        }
        result.push(TransportLight::from(light));
    }
    result.sort_by_key(|light| light.endpoint_id);
    Ok(result)
}

#[derive(Clone, Copy)]
struct StartupReady {
    commissioned: bool,
    commissioning_window_open: bool,
}

#[allow(clippy::too_many_arguments)]
fn run_transport(
    storage_dir: &Path,
    identity: &MatterIdentity,
    shape: &[EndpointShape],
    initial_lights: Vec<TransportLight>,
    remote_writes: Sender<MatterRemoteWrite>,
    control_rx: Receiver<ControlCommand>,
    ready_tx: SyncSender<Result<StartupReady, String>>,
    shared_snapshot: &RwLock<MatterTransportSnapshot>,
) -> Result<(), Error> {
    let basic_info = basic_info(identity);
    let commissioning = commissioning_data(identity);
    let matter = Matter::new(&basic_info, commissioning, &TEST_DEV_ATT, MATTER_PORT);
    let store = DirKvBlobStore::new(storage_dir.join(KV_DIRECTORY));
    let kv = matter.kv(store);
    let buffers: MatterBuffers = MatterBuffers::new();
    let mut im_state: EthInteractionModelState =
        EthInteractionModelState::new(EthNetwork::new_default());
    block_on(matter.load_persist(&kv))?;
    block_on(im_state.load_persist(&kv))?;

    let crypto = default_crypto(rand::thread_rng(), DAC_PRIVKEY);
    let mut random = crypto.rand()?;
    let bridge_lights = BridgeLights::new(
        initial_lights,
        remote_writes,
        Dataver::new_rand(&mut random),
        Dataver::new_rand(&mut random),
        Dataver::new_rand(&mut random),
    );
    let endpoints_meta = build_endpoints(shape);
    let node = Node::new(&endpoints_meta);

    let handler = endpoints::EthSysHandlerBuilder::new()
        .netif_diag(&SysNetifs)
        .build(random)
        .chain(
            EpClMatcher::new(None, Some(desc::DescHandler::CLUSTER.id)),
            Async(desc::DescHandler::new(Dataver::new_rand(&mut random)).adapt()),
        )
        .chain(
            EpClMatcher::new(
                Some(AGGREGATOR_ENDPOINT_ID),
                Some(desc::DescHandler::CLUSTER.id),
            ),
            Async(desc::DescHandler::new_aggregator(Dataver::new_rand(&mut random)).adapt()),
        )
        .chain(
            EpClMatcher::new(None, Some(groups::GroupsHandler::CLUSTER.id)),
            Async(groups::GroupsHandler::new(Dataver::new_rand(&mut random)).adapt()),
        )
        .chain(
            EpClMatcher::new(
                None,
                Some(<BridgeLights as bridged_info::ClusterHandler>::CLUSTER.id),
            ),
            Async(bridged_info::HandlerAdaptor(&bridge_lights)),
        )
        .chain(
            EpClMatcher::new(
                None,
                Some(<BridgeLights as on_off::ClusterHandler>::CLUSTER.id),
            ),
            Async(on_off::HandlerAdaptor(&bridge_lights)),
        )
        .chain(
            EpClMatcher::new(
                None,
                Some(<BridgeLights as level_control::ClusterHandler>::CLUSTER.id),
            ),
            Async(level_control::HandlerAdaptor(&bridge_lights)),
        );
    let data_model = (node, handler);
    let im = InteractionModel::new(&matter, &crypto, &buffers, data_model, &kv, &im_state);

    let composition_path = storage_dir.join(COMPOSITION_FILE);
    let serialized_shape = serde_json::to_vec_pretty(shape).map_err(|_| ErrorCode::StdIoError)?;
    let composition_changed =
        fs::read(&composition_path).map_or(true, |current| current != serialized_shape);
    if composition_changed {
        im.bump_configuration_version()?;
        fs::create_dir_all(storage_dir).map_err(|_| ErrorCode::StdIoError)?;
        fs::write(composition_path, serialized_shape).map_err(|_| ErrorCode::StdIoError)?;
    }
    if !matter.is_commissioned() {
        im.open_basic_comm_window(MAX_COMM_WINDOW_TIMEOUT_SECS)?;
    }

    let matter_socket = match async_io::Async::<UdpSocket>::bind(MATTER_SOCKET_BIND_ADDR) {
        Ok(socket) => socket,
        Err(error) => {
            let _ = ready_tx.send(Err(format!("Matter UDP bind failed: {error}")));
            return Ok(());
        }
    };
    let mdns = match BuiltinMdnsRuntime::bind() {
        Ok(mdns) => mdns,
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return Ok(());
        }
    };
    let startup = StartupReady {
        commissioned: matter.is_commissioned(),
        commissioning_window_open: commissioning_window_open(&matter)?,
    };
    if ready_tx.send(Ok(startup)).is_err() {
        return Ok(());
    }

    let responder = DefaultResponder::new(&im);
    let mut respond = pin!(responder.run::<4, 4>());
    let mut im_job = pin!(im.run());
    let mut transport = pin!(matter.run(&crypto, &matter_socket, &matter_socket, &matter_socket,));
    let mut mdns_job = pin!(mdns.run(&matter, &crypto, &identity.serial));
    let mut control = pin!(run_control_loop(
        &matter,
        &im,
        &bridge_lights,
        control_rx,
        shared_snapshot,
    ));
    block_on(
        select3(
            &mut transport,
            &mut mdns_job,
            select3(&mut respond, &mut im_job, &mut control).coalesce(),
        )
        .coalesce(),
    )
}

async fn run_control_loop(
    matter: &Matter<'_>,
    notifier: &dyn AttrChangeNotifier,
    lights: &BridgeLights,
    control_rx: Receiver<ControlCommand>,
    shared_snapshot: &RwLock<MatterTransportSnapshot>,
) -> Result<(), Error> {
    loop {
        while let Ok(command) = control_rx.try_recv() {
            match command {
                ControlCommand::Reconcile(values) => {
                    for change in lights.reconcile(values) {
                        if change.on {
                            notifier.notify_attr_changed(
                                change.endpoint_id,
                                <BridgeLights as on_off::ClusterHandler>::CLUSTER.id,
                                on_off::AttributeId::OnOff as _,
                            );
                        }
                        if change.level {
                            notifier.notify_attr_changed(
                                change.endpoint_id,
                                <BridgeLights as level_control::ClusterHandler>::CLUSTER.id,
                                level_control::AttributeId::CurrentLevel as _,
                            );
                        }
                    }
                }
                ControlCommand::Shutdown => return Ok(()),
            }
        }
        {
            let commissioned = matter.is_commissioned();
            let window = commissioning_window_open(matter)?;
            let mut snapshot = shared_snapshot.write();
            snapshot.commissioned = commissioned;
            snapshot.commissioning_window_open = window;
            snapshot.commissionable = snapshot.network_running && window;
        }
        async_io::Timer::after(CONTROL_POLL_INTERVAL).await;
    }
}

fn commissioning_window_open(matter: &Matter<'_>) -> Result<bool, Error> {
    let mut open = false;
    matter.mdns_services(|service| {
        if matches!(service, MatterLocalService::Commissionable { .. }) {
            open = true;
        }
        Ok(())
    })?;
    Ok(open)
}

fn build_endpoints(shape: &[EndpointShape]) -> Vec<Endpoint<'static>> {
    let mut endpoints_meta = Vec::with_capacity(shape.len() + 2);
    endpoints_meta.push(ROOT_ENDPOINT);
    for endpoint in shape {
        endpoints_meta.push(Endpoint::new(
            endpoint.endpoint_id,
            devices!(DEV_TYPE_DIMMABLE_LIGHT, DEV_TYPE_BRIDGED_NODE),
            clusters!(
                desc::DescHandler::CLUSTER,
                groups::GroupsHandler::CLUSTER,
                <BridgeLights as bridged_info::ClusterHandler>::CLUSTER,
                <BridgeLights as on_off::ClusterHandler>::CLUSTER,
                <BridgeLights as level_control::ClusterHandler>::CLUSTER,
            ),
        ));
    }
    endpoints_meta.push(Endpoint::new(
        AGGREGATOR_ENDPOINT_ID,
        devices!(DEV_TYPE_AGGREGATOR),
        clusters!(desc::DescHandler::CLUSTER),
    ));
    endpoints_meta
}

fn basic_info(
    identity: &MatterIdentity,
) -> rs_matter::dm::clusters::basic_info::BasicInfoConfig<'_> {
    rs_matter::dm::clusters::basic_info::BasicInfoConfig {
        vendor_name: "ToskLight",
        vid: TEST_VID,
        product_name: "ToskLight Matter Bridge",
        pid: TEST_PID,
        hw_ver: 1,
        hw_ver_str: "1",
        sw_ver: 1,
        sw_ver_str: env!("CARGO_PKG_VERSION"),
        serial_no: &identity.serial,
        unique_id: &identity.serial,
        device_name: "ToskLight",
        ..rs_matter::dm::clusters::basic_info::BasicInfoConfig::new()
    }
}

fn commissioning_data(identity: &MatterIdentity) -> BasicCommData {
    let passcode = identity.passcode.to_le_bytes();
    BasicCommData {
        password: Spake2pVerifierPassword::new_from_ref(Spake2pVerifierPasswordRef::new(&passcode)),
        discriminator: identity.discriminator,
    }
}

fn pairing_data(identity: &MatterIdentity) -> Result<MatterPairingData, Error> {
    let commissioning = commissioning_data(identity);
    let qr = QrPayload::new(
        DiscoveryCapabilities::IP,
        CommFlowType::Standard,
        commissioning.clone(),
        TEST_VID,
        TEST_PID,
        &identity.serial,
        no_optional_data as NoOptionalData,
    );
    let mut buffer = [0_u8; 512];
    let (qr_code, _) = qr.as_str(&mut buffer)?;
    Ok(MatterPairingData {
        qr_code: qr_code.to_owned(),
        manual_code: commissioning.compute_pretty_pairing_code().to_string(),
        discriminator: identity.discriminator,
    })
}

fn load_or_create_identity(storage_dir: &Path) -> io::Result<MatterIdentity> {
    fs::create_dir_all(storage_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(storage_dir, fs::Permissions::from_mode(0o700))?;
    }
    let path = storage_dir.join(IDENTITY_FILE);
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let identity = random_identity();
            write_private_json(&path, &identity)?;
            Ok(identity)
        }
        Err(error) => Err(error),
    }
}

fn random_identity() -> MatterIdentity {
    let mut random = rand::thread_rng();
    let passcode = loop {
        let candidate = random.next_u32() % 99_999_998 + 1;
        if !matches!(
            candidate,
            11_111_111
                | 22_222_222
                | 33_333_333
                | 44_444_444
                | 55_555_555
                | 66_666_666
                | 77_777_777
                | 88_888_888
                | 12_345_678
                | 87_654_321
        ) {
            break candidate;
        }
    };
    MatterIdentity {
        passcode,
        discriminator: (random.next_u32() & 0x0fff) as u16,
        serial: format!("{:016x}", random.next_u64()),
    }
}

fn write_private_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(temporary, path)
}

#[derive(Clone, Debug)]
struct EndpointState {
    name: String,
    on: bool,
    level: u8,
    options: u8,
    on_level: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AttributeChanges {
    endpoint_id: u16,
    on: bool,
    level: bool,
}

struct BridgeLights {
    endpoints: RwLock<BTreeMap<u16, EndpointState>>,
    remote_writes: Sender<MatterRemoteWrite>,
    on_off_dataver: Dataver,
    level_dataver: Dataver,
    bridged_info_dataver: Dataver,
}

impl std::fmt::Debug for BridgeLights {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BridgeLights")
            .field("endpoint_count", &self.endpoints.read().len())
            .finish_non_exhaustive()
    }
}

impl BridgeLights {
    fn new(
        lights: Vec<TransportLight>,
        remote_writes: Sender<MatterRemoteWrite>,
        on_off_dataver: Dataver,
        level_dataver: Dataver,
        bridged_info_dataver: Dataver,
    ) -> Self {
        let endpoints = lights
            .into_iter()
            .map(|light| {
                (
                    light.endpoint_id,
                    EndpointState {
                        name: light.name,
                        on: light.on,
                        level: if light.level == 0 { 1 } else { light.level },
                        options: level_control::OptionsBitmap::EXECUTE_IF_OFF.bits(),
                        on_level: None,
                    },
                )
            })
            .collect();
        Self {
            endpoints: RwLock::new(endpoints),
            remote_writes,
            on_off_dataver,
            level_dataver,
            bridged_info_dataver,
        }
    }

    fn reconcile(&self, lights: Vec<TransportLight>) -> Vec<AttributeChanges> {
        let mut endpoints = self.endpoints.write();
        let mut changes = Vec::new();
        for light in lights {
            if let Some(endpoint) = endpoints.get_mut(&light.endpoint_id) {
                let old_on = endpoint.on;
                let old_level = endpoint.level;
                endpoint.on = light.on;
                if light.level > 0 {
                    endpoint.level = light.level;
                }
                if old_on != endpoint.on || old_level != endpoint.level {
                    changes.push(AttributeChanges {
                        endpoint_id: light.endpoint_id,
                        on: old_on != endpoint.on,
                        level: old_level != endpoint.level,
                    });
                }
            }
        }
        changes
    }

    fn endpoint(&self, endpoint_id: u16) -> Result<EndpointState, Error> {
        self.endpoints
            .read()
            .get(&endpoint_id)
            .cloned()
            .ok_or_else(|| ErrorCode::EndpointNotFound.into())
    }

    fn send_write(&self, endpoint_id: u16, write: MatterPlaybackWrite) -> Result<(), Error> {
        self.remote_writes
            .send(MatterRemoteWrite { endpoint_id, write })
            .map_err(|_| ErrorCode::Failure.into())
    }

    fn set_on(&self, endpoint_id: u16, on: bool) -> Result<u8, Error> {
        let level = {
            let mut endpoints = self.endpoints.write();
            let endpoint = endpoints
                .get_mut(&endpoint_id)
                .ok_or(ErrorCode::EndpointNotFound)?;
            endpoint.on = on;
            endpoint.level
        };
        self.send_write(
            endpoint_id,
            MatterPlaybackWrite {
                on: Some(on),
                level: on.then_some(level),
            },
        )?;
        Ok(level)
    }

    fn set_level(&self, endpoint_id: u16, level: u8) -> Result<bool, Error> {
        if level == u8::MAX {
            return Err(ErrorCode::ConstraintError.into());
        }
        let on = level > 0;
        {
            let mut endpoints = self.endpoints.write();
            let endpoint = endpoints
                .get_mut(&endpoint_id)
                .ok_or(ErrorCode::EndpointNotFound)?;
            endpoint.on = on;
            if level > 0 {
                endpoint.level = level.min(MAX_MATTER_LEVEL);
            }
        }
        self.send_write(
            endpoint_id,
            MatterPlaybackWrite {
                on: Some(on),
                level: on.then_some(level.min(MAX_MATTER_LEVEL)),
            },
        )?;
        Ok(on)
    }

    fn step_level(&self, endpoint_id: u16, up: bool, step: u8) -> Result<u8, Error> {
        let current = self.endpoint(endpoint_id)?.level;
        let target = if up {
            current.saturating_add(step).min(MAX_MATTER_LEVEL)
        } else {
            current.saturating_sub(step)
        };
        self.set_level(endpoint_id, target)?;
        Ok(target)
    }
}

impl on_off::ClusterHandler for BridgeLights {
    const CLUSTER: Cluster<'static> = on_off::FULL_CLUSTER
        .with_features(on_off::Feature::LIGHTING.bits())
        .with_attrs(with!(required; on_off::AttributeId::OnOff))
        .with_cmds(with!(
            on_off::CommandId::Off | on_off::CommandId::On | on_off::CommandId::Toggle
        ));

    fn dataver(&self) -> u32 {
        self.on_off_dataver.get()
    }

    fn dataver_changed(&self) {
        self.on_off_dataver.changed();
    }

    fn on_off(&self, ctx: impl ReadContext) -> Result<bool, Error> {
        Ok(self.endpoint(ctx.attr().endpoint_id)?.on)
    }

    fn handle_off(&self, ctx: impl InvokeContext) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        self.set_on(endpoint, false)?;
        ctx.notify_own_attr_changed(on_off::AttributeId::OnOff as _);
        Ok(())
    }

    fn handle_on(&self, ctx: impl InvokeContext) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        let _ = self.set_on(endpoint, true)?;
        ctx.notify_own_attr_changed(on_off::AttributeId::OnOff as _);
        Ok(())
    }

    fn handle_toggle(&self, ctx: impl InvokeContext) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        let on = !self.endpoint(endpoint)?.on;
        let _ = self.set_on(endpoint, on)?;
        ctx.notify_own_attr_changed(on_off::AttributeId::OnOff as _);
        Ok(())
    }

    fn handle_off_with_effect(
        &self,
        _ctx: impl InvokeContext,
        _request: on_off::OffWithEffectRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }

    fn handle_on_with_recall_global_scene(&self, _ctx: impl InvokeContext) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }

    fn handle_on_with_timed_off(
        &self,
        _ctx: impl InvokeContext,
        _request: on_off::OnWithTimedOffRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }
}

impl level_control::ClusterHandler for BridgeLights {
    const CLUSTER: Cluster<'static> = level_control::FULL_CLUSTER
        .with_features(
            level_control::Feature::LIGHTING.bits() | level_control::Feature::ON_OFF.bits(),
        )
        .with_attrs(with!(
            required;
            level_control::AttributeId::CurrentLevel
                | level_control::AttributeId::MinLevel
                | level_control::AttributeId::MaxLevel
                | level_control::AttributeId::Options
                | level_control::AttributeId::OnLevel
        ))
        .with_cmds(with!(
            level_control::CommandId::MoveToLevel
                | level_control::CommandId::Move
                | level_control::CommandId::Step
                | level_control::CommandId::Stop
                | level_control::CommandId::MoveToLevelWithOnOff
                | level_control::CommandId::MoveWithOnOff
                | level_control::CommandId::StepWithOnOff
                | level_control::CommandId::StopWithOnOff
        ));

    fn dataver(&self) -> u32 {
        self.level_dataver.get()
    }

    fn dataver_changed(&self) {
        self.level_dataver.changed();
    }

    fn current_level(&self, ctx: impl ReadContext) -> Result<Nullable<u8>, Error> {
        Ok(Nullable::some(self.endpoint(ctx.attr().endpoint_id)?.level))
    }

    fn min_level(&self, _ctx: impl ReadContext) -> Result<u8, Error> {
        Ok(1)
    }

    fn max_level(&self, _ctx: impl ReadContext) -> Result<u8, Error> {
        Ok(MAX_MATTER_LEVEL)
    }

    fn options(&self, ctx: impl ReadContext) -> Result<level_control::OptionsBitmap, Error> {
        level_control::OptionsBitmap::from_bits(self.endpoint(ctx.attr().endpoint_id)?.options)
            .ok_or_else(|| ErrorCode::Invalid.into())
    }

    fn on_level(&self, ctx: impl ReadContext) -> Result<Nullable<u8>, Error> {
        Ok(Nullable::new(
            self.endpoint(ctx.attr().endpoint_id)?.on_level,
        ))
    }

    fn set_options(
        &self,
        ctx: impl WriteContext,
        value: level_control::OptionsBitmap,
    ) -> Result<(), Error> {
        let mut endpoints = self.endpoints.write();
        let endpoint = endpoints
            .get_mut(&ctx.attr().endpoint_id)
            .ok_or(ErrorCode::EndpointNotFound)?;
        endpoint.options = value.bits();
        Ok(())
    }

    fn set_on_level(&self, ctx: impl WriteContext, value: Nullable<u8>) -> Result<(), Error> {
        let mut endpoints = self.endpoints.write();
        let endpoint = endpoints
            .get_mut(&ctx.attr().endpoint_id)
            .ok_or(ErrorCode::EndpointNotFound)?;
        endpoint.on_level = value.into_option();
        Ok(())
    }

    fn handle_move_to_level(
        &self,
        ctx: impl InvokeContext,
        request: level_control::MoveToLevelRequest<'_>,
    ) -> Result<(), Error> {
        self.apply_level_command(ctx, request.level()?)
    }

    fn handle_move(
        &self,
        ctx: impl InvokeContext,
        request: level_control::MoveRequest<'_>,
    ) -> Result<(), Error> {
        let target = match request.move_mode()? {
            level_control::MoveModeEnum::Up => MAX_MATTER_LEVEL,
            level_control::MoveModeEnum::Down => 0,
        };
        self.apply_level_command(ctx, target)
    }

    fn handle_step(
        &self,
        ctx: impl InvokeContext,
        request: level_control::StepRequest<'_>,
    ) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        let target = self.step_level(
            endpoint,
            request.step_mode()? == level_control::StepModeEnum::Up,
            request.step_size()?,
        )?;
        self.notify_level_command(&ctx, endpoint, target > 0);
        Ok(())
    }

    fn handle_stop(
        &self,
        _ctx: impl InvokeContext,
        _request: level_control::StopRequest<'_>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn handle_move_to_level_with_on_off(
        &self,
        ctx: impl InvokeContext,
        request: level_control::MoveToLevelWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        self.apply_level_command(ctx, request.level()?)
    }

    fn handle_move_with_on_off(
        &self,
        ctx: impl InvokeContext,
        request: level_control::MoveWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        let target = match request.move_mode()? {
            level_control::MoveModeEnum::Up => MAX_MATTER_LEVEL,
            level_control::MoveModeEnum::Down => 0,
        };
        self.apply_level_command(ctx, target)
    }

    fn handle_step_with_on_off(
        &self,
        ctx: impl InvokeContext,
        request: level_control::StepWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        let target = self.step_level(
            endpoint,
            request.step_mode()? == level_control::StepModeEnum::Up,
            request.step_size()?,
        )?;
        self.notify_level_command(&ctx, endpoint, target > 0);
        Ok(())
    }

    fn handle_stop_with_on_off(
        &self,
        _ctx: impl InvokeContext,
        _request: level_control::StopWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn handle_move_to_closest_frequency(
        &self,
        _ctx: impl InvokeContext,
        _request: level_control::MoveToClosestFrequencyRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }
}

impl BridgeLights {
    fn apply_level_command(&self, ctx: impl InvokeContext, level: u8) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        let on = self.set_level(endpoint, level)?;
        self.notify_level_command(&ctx, endpoint, on);
        Ok(())
    }

    fn notify_level_command(&self, ctx: &impl InvokeContext, endpoint: u16, _on: bool) {
        ctx.notify_own_attr_changed(level_control::AttributeId::CurrentLevel as _);
        ctx.notify_attr_changed(
            endpoint,
            <Self as on_off::ClusterHandler>::CLUSTER.id,
            on_off::AttributeId::OnOff as _,
        );
    }
}

impl bridged_info::ClusterHandler for BridgeLights {
    const CLUSTER: Cluster<'static> = bridged_info::FULL_CLUSTER
        .with_features(0)
        .with_attrs(with!(required; bridged_info::AttributeId::ProductName))
        .with_cmds(with!());

    fn dataver(&self) -> u32 {
        self.bridged_info_dataver.get()
    }

    fn dataver_changed(&self) {
        self.bridged_info_dataver.changed();
    }

    fn product_name<P: TLVBuilderParent>(
        &self,
        ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        builder.set(&self.endpoint(ctx.attr().endpoint_id)?.name)
    }

    fn reachable(&self, ctx: impl ReadContext) -> Result<bool, Error> {
        self.endpoint(ctx.attr().endpoint_id).map(|_| true)
    }

    fn unique_id<P: TLVBuilderParent>(
        &self,
        ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        builder.set(&format!("tosklight-{}", ctx.attr().endpoint_id))
    }

    fn handle_keep_active(
        &self,
        _ctx: impl InvokeContext,
        _request: bridged_info::KeepActiveRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }
}

/// Builtin mDNS setup derived from rs-matter's official cross-platform example. Sockets are bound
/// before the transport reports `network_running`, so startup failures remain truthful.
struct BuiltinMdnsRuntime {
    ipv4_socket: async_io::Async<UdpSocket>,
    ipv6_socket: async_io::Async<UdpSocket>,
    ipv4: Ipv4Addr,
    ipv6: Ipv6Addr,
    interface: u32,
    ready: Cell<MdnsReady>,
}

#[derive(Clone, Copy)]
enum MdnsReady {
    Ipv4,
    Ipv6,
}

impl BuiltinMdnsRuntime {
    fn bind() -> Result<Self, String> {
        let (ipv4, ipv6, interface) = select_network_interface()
            .map_err(|error| format!("mDNS network interface selection failed: {error}"))?;
        let ipv6_socket = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))
            .map_err(|error| format!("mDNS socket creation failed: {error}"))?;
        ipv6_socket
            .set_reuse_address(true)
            .map_err(|error| format!("mDNS IPv6 SO_REUSEADDR failed: {error}"))?;
        #[cfg(unix)]
        ipv6_socket
            .set_reuse_port(true)
            .map_err(|error| format!("mDNS IPv6 SO_REUSEPORT failed: {error}"))?;
        ipv6_socket
            .set_only_v6(true)
            .map_err(|error| format!("mDNS IPv6-only socket setup failed: {error}"))?;
        ipv6_socket
            .bind(&MDNS_SOCKET_DEFAULT_BIND_ADDR.into())
            .map_err(|error| format!("mDNS IPv6 UDP bind failed: {error}"))?;
        ipv6_socket
            .set_multicast_if_v6(interface)
            .map_err(|error| format!("mDNS IPv6 multicast interface setup failed: {error}"))?;
        let ipv6_socket = async_io::Async::<UdpSocket>::new_nonblocking(ipv6_socket.into())
            .map_err(|error| format!("mDNS IPv6 async socket setup failed: {error}"))?;
        ipv6_socket
            .get_ref()
            .join_multicast_v6(&MDNS_IPV6_BROADCAST_ADDR, interface)
            .map_err(|error| format!("mDNS IPv6 multicast join failed: {error}"))?;
        let ipv4_socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .map_err(|error| format!("mDNS IPv4 socket creation failed: {error}"))?;
        ipv4_socket
            .set_reuse_address(true)
            .map_err(|error| format!("mDNS IPv4 SO_REUSEADDR failed: {error}"))?;
        #[cfg(unix)]
        ipv4_socket
            .set_reuse_port(true)
            .map_err(|error| format!("mDNS IPv4 SO_REUSEPORT failed: {error}"))?;
        let ipv4_bind = std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::UNSPECIFIED,
            MDNS_PORT,
        ));
        ipv4_socket
            .bind(&ipv4_bind.into())
            .map_err(|error| format!("mDNS IPv4 UDP bind failed: {error}"))?;
        ipv4_socket
            .set_multicast_if_v4(&ipv4)
            .map_err(|error| format!("mDNS IPv4 multicast interface setup failed: {error}"))?;
        let ipv4_socket = async_io::Async::<UdpSocket>::new_nonblocking(ipv4_socket.into())
            .map_err(|error| format!("mDNS IPv4 async socket setup failed: {error}"))?;
        ipv4_socket
            .get_ref()
            .join_multicast_v4(&MDNS_IPV4_BROADCAST_ADDR, &ipv4)
            .map_err(|error| format!("mDNS IPv4 multicast join failed: {error}"))?;
        Ok(Self {
            ipv4_socket,
            ipv6_socket,
            ipv4,
            ipv6,
            interface,
            ready: Cell::new(MdnsReady::Ipv4),
        })
    }

    async fn run<C: Crypto>(
        &self,
        matter: &Matter<'_>,
        crypto: C,
        serial: &str,
    ) -> Result<(), Error> {
        BuiltinMdns::new()
            .run(
                self,
                self,
                &Host {
                    hostname: serial,
                    ip: self.ipv4,
                    ipv6: self.ipv6,
                },
                Some(self.ipv4),
                Some(self.interface),
                matter,
                crypto,
            )
            .await
    }
}

impl NetworkSend for &BuiltinMdnsRuntime {
    async fn send_to(&mut self, data: &[u8], address: Address) -> Result<(), Error> {
        let address = address.udp().ok_or(ErrorCode::NoNetworkInterface)?;
        match address {
            std::net::SocketAddr::V4(_) => {
                self.ipv4_socket.send_to(data, address).await?;
            }
            std::net::SocketAddr::V6(_) => {
                self.ipv6_socket.send_to(data, address).await?;
            }
        }
        Ok(())
    }
}

impl NetworkReceive for &BuiltinMdnsRuntime {
    async fn wait_available(&mut self) -> Result<(), Error> {
        match select(self.ipv4_socket.readable(), self.ipv6_socket.readable()).await {
            Either::First(result) => {
                result?;
                self.ready.set(MdnsReady::Ipv4);
            }
            Either::Second(result) => {
                result?;
                self.ready.set(MdnsReady::Ipv6);
            }
        }
        Ok(())
    }

    async fn recv_from(&mut self, buffer: &mut [u8]) -> Result<(usize, Address), Error> {
        let (length, address) = match self.ready.get() {
            MdnsReady::Ipv4 => self.ipv4_socket.recv_from(buffer).await?,
            MdnsReady::Ipv6 => self.ipv6_socket.recv_from(buffer).await?,
        };
        Ok((length, Address::Udp(address)))
    }
}

fn select_network_interface() -> Result<(Ipv4Addr, Ipv6Addr, u32), Error> {
    let all = if_addrs::get_if_addrs().map_err(|_| ErrorCode::StdIoError)?;
    let candidate = [true, false].into_iter().find_map(|link_local_only| {
        all.iter()
            .filter(|interface| !interface.is_loopback())
            .filter_map(|interface| match interface.addr {
                if_addrs::IfAddr::V6(ref ipv6)
                    if !link_local_only || ipv6.ip.is_unicast_link_local() =>
                {
                    Some((
                        interface.name.clone(),
                        ipv6.ip,
                        interface.index.unwrap_or(0),
                    ))
                }
                _ => None,
            })
            .find_map(|(name, ipv6, index)| {
                all.iter()
                    .filter(|other| other.name == name && !other.is_loopback())
                    .find_map(|other| match other.addr {
                        if_addrs::IfAddr::V4(ref ipv4) => Some((ipv4.ip, ipv6, index)),
                        _ => None,
                    })
            })
    });
    let (ipv4, ipv6, interface) = candidate
        .or_else(|| {
            all.iter()
                .filter(|interface| !interface.is_loopback())
                .find_map(|interface| match interface.addr {
                    if_addrs::IfAddr::V4(ref ipv4) => Some((
                        ipv4.ip,
                        std::net::Ipv6Addr::UNSPECIFIED,
                        interface.index.unwrap_or(0),
                    )),
                    _ => None,
                })
        })
        .ok_or(ErrorCode::StdIoError)?;
    Ok((ipv4.octets().into(), ipv6.octets().into(), interface))
}

fn matter_string(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(endpoint_id: u16, name: &str, on: bool, level: u8) -> MatterPlaybackLight {
        MatterPlaybackLight {
            endpoint_id,
            page: 1,
            playback: endpoint_id as u8,
            playback_number: endpoint_id,
            name: name.into(),
            on,
            level,
        }
    }

    #[test]
    fn identity_and_pairing_material_are_stable_in_the_desk_data_directory() {
        let directory = std::env::temp_dir().join(format!("light-matter-{}", uuid::Uuid::new_v4()));
        let first = load_or_create_identity(&directory).unwrap();
        let second = load_or_create_identity(&directory).unwrap();
        assert_eq!(first, second);
        let pairing = pairing_data(&first).unwrap();
        assert!(pairing.qr_code.starts_with("MT:"));
        assert_eq!(pairing.manual_code.len(), 13);
        assert_eq!(pairing.discriminator, first.discriminator);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    #[ignore = "binds the standard Matter and mDNS ports"]
    fn commissionable_network_transport_smoke() {
        let directory =
            std::env::temp_dir().join(format!("light-matter-smoke-{}", uuid::Uuid::new_v4()));
        let transport = MatterTransport::new(&directory);
        let running = transport.reconcile(true, &[light(1, "First", true, 127)]);
        assert_eq!(
            running.lifecycle,
            MatterTransportLifecycle::Running,
            "{running:?}"
        );
        assert!(running.network_running);
        assert!(running.commissioning_window_open);
        assert!(running.commissionable);
        assert!(running.pairing.is_some());
        assert_eq!(running.endpoint_count, 1);

        let identity_path = directory.join("matter").join(IDENTITY_FILE);
        let identity = fs::read(&identity_path).unwrap();
        let value_only = transport.reconcile(true, &[light(1, "First", true, 64)]);
        assert_eq!(value_only.lifecycle, MatterTransportLifecycle::Running);
        assert_eq!(value_only.endpoint_count, 1);
        assert_eq!(value_only.pairing, running.pairing);

        let after_removal = transport.reconcile(true, &[]);
        assert_eq!(
            after_removal.lifecycle,
            MatterTransportLifecycle::Running,
            "{after_removal:?}"
        );
        assert_eq!(after_removal.endpoint_count, 0);
        assert_eq!(after_removal.pairing, running.pairing);
        assert_eq!(fs::read(identity_path).unwrap(), identity);

        transport.stop();
        assert_eq!(transport.snapshot(), MatterTransportSnapshot::default());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn endpoint_validation_preserves_stable_ids_and_rejects_reserved_or_duplicate_ids() {
        let validated = validate_lights(&[
            light(128, "Second page", false, 0),
            light(1, "First page", true, 254),
        ])
        .unwrap();
        assert_eq!(
            validated
                .iter()
                .map(|light| light.endpoint_id)
                .collect::<Vec<_>>(),
            vec![1, 128]
        );
        assert!(validate_lights(&[light(0, "Root", false, 0)]).is_err());
        assert!(validate_lights(&[light(1, "A", false, 0), light(1, "B", false, 0)]).is_err());
    }

    #[test]
    fn endpoint_metadata_removes_empty_playbacks_without_renumbering_survivors() {
        let initial = build_endpoints(&[
            EndpointShape {
                endpoint_id: 1,
                name: "First".into(),
            },
            EndpointShape {
                endpoint_id: 128,
                name: "Second page".into(),
            },
        ]);
        assert_eq!(
            initial
                .iter()
                .map(|endpoint| endpoint.id)
                .collect::<Vec<_>>(),
            vec![0, 1, 128, AGGREGATOR_ENDPOINT_ID]
        );

        let after_removal = build_endpoints(&[EndpointShape {
            endpoint_id: 128,
            name: "Second page".into(),
        }]);
        assert_eq!(
            after_removal
                .iter()
                .map(|endpoint| endpoint.id)
                .collect::<Vec<_>>(),
            vec![0, 128, AGGREGATOR_ENDPOINT_ID]
        );
    }

    #[test]
    fn outbound_tracking_updates_change_only_the_subscription_attributes_that_moved() {
        let (sender, _receiver) = mpsc::channel();
        let handler = BridgeLights::new(
            vec![TransportLight::from(&light(1, "Look", true, 127))],
            sender,
            Dataver::new(1),
            Dataver::new(2),
            Dataver::new(3),
        );
        let changes = handler.reconcile(vec![TransportLight::from(&light(1, "Look", false, 0))]);
        assert_eq!(
            changes,
            vec![AttributeChanges {
                endpoint_id: 1,
                on: true,
                level: false,
            }]
        );
        assert_eq!(handler.endpoint(1).unwrap().level, 127);
    }

    #[test]
    fn controller_mutations_are_forwarded_as_onoff_and_level_writes() {
        let (sender, receiver) = mpsc::channel();
        let handler = BridgeLights::new(
            vec![TransportLight::from(&light(128, "Look", false, 0))],
            sender,
            Dataver::new(1),
            Dataver::new(2),
            Dataver::new(3),
        );
        handler.set_level(128, 64).unwrap();
        assert_eq!(
            receiver.recv().unwrap(),
            MatterRemoteWrite {
                endpoint_id: 128,
                write: MatterPlaybackWrite {
                    on: Some(true),
                    level: Some(64),
                },
            }
        );
        handler.set_on(128, false).unwrap();
        assert_eq!(receiver.recv().unwrap().write.on, Some(false));
    }

    #[test]
    fn matter_names_are_valid_utf8_and_fit_the_cluster_limit() {
        assert_eq!(matter_string("A short name", 32), "A short name");
        let truncated = matter_string("Page 127 Playback 127: 🎭🎭🎭🎭", 32);
        assert!(truncated.len() <= 32);
        assert!(truncated.is_char_boundary(truncated.len()));
    }
}
