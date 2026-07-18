use super::bridge::BridgeLights;
use super::commissioning::{basic_info, commissioning_data};
use super::mdns::BuiltinMdnsRuntime;
use super::model::{
    AGGREGATOR_ENDPOINT_ID, ControlCommand, EndpointShape, MatterIdentity, MatterRemoteWrite,
    MatterTransportSnapshot, StartupReady, TransportLight,
};
use super::node::build_endpoints;
use embassy_futures::select::select3;
use futures_lite::future::block_on;
use parking_lot::RwLock;
use rs_matter::crypto::{Crypto, default_crypto};
use rs_matter::dm::clusters::decl::{
    bridged_device_basic_information as bridged_info, level_control, on_off,
};
use rs_matter::dm::clusters::desc::{self, ClusterHandler as _};
use rs_matter::dm::clusters::groups::{self, ClusterHandler as _};
use rs_matter::dm::devices::test::{DAC_PRIVKEY, TEST_DEV_ATT};
use rs_matter::dm::endpoints;
use rs_matter::dm::networks::SysNetifs;
use rs_matter::dm::networks::eth::EthNetwork;
use rs_matter::dm::{Async, AttrChangeNotifier, Dataver, EpClMatcher, Node};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::im::{EthInteractionModelState, InteractionModel};
use rs_matter::persist::DirKvBlobStore;
use rs_matter::respond::DefaultResponder;
use rs_matter::sc::pase::MAX_COMM_WINDOW_TIMEOUT_SECS;
use rs_matter::transport::MATTER_SOCKET_BIND_ADDR;
use rs_matter::transport::exchange::MatterBuffers;
use rs_matter::transport::network::MatterLocalService;
use rs_matter::utils::select::Coalesce;
use rs_matter::{MATTER_PORT, Matter};
use std::fs;
use std::net::UdpSocket;
use std::path::Path;
use std::pin::pin;
use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::time::Duration;

const COMPOSITION_FILE: &str = "composition.json";
const KV_DIRECTORY: &str = "kv";
const CONTROL_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[allow(clippy::too_many_arguments)]
pub(super) fn run_transport(
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
                ControlCommand::Reconcile(values) => notify_changes(notifier, lights, values),
                ControlCommand::Shutdown => return Ok(()),
            }
        }
        update_snapshot(matter, shared_snapshot)?;
        async_io::Timer::after(CONTROL_POLL_INTERVAL).await;
    }
}

fn notify_changes(
    notifier: &dyn AttrChangeNotifier,
    lights: &BridgeLights,
    values: Vec<TransportLight>,
) {
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

fn update_snapshot(
    matter: &Matter<'_>,
    shared_snapshot: &RwLock<MatterTransportSnapshot>,
) -> Result<(), Error> {
    let commissioned = matter.is_commissioned();
    let window = commissioning_window_open(matter)?;
    let mut snapshot = shared_snapshot.write();
    snapshot.commissioned = commissioned;
    snapshot.commissioning_window_open = window;
    snapshot.commissionable = snapshot.network_running && window;
    Ok(())
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
