use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use media_application::configuration::OutputConfiguration;
use media_domain::{LayerPersonality, MediaState, OutputState};

use super::*;
use crate::live_settings::LiveSettings;

fn free_port() -> SocketAddr {
    // Bound and released, so the listener under test can take it.
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

fn configuration(protocol: DmxProtocol, universe: u16, listen: SocketAddr) -> MediaConfiguration {
    let mut output = OutputConfiguration::new("Main");
    output.protocol = protocol;
    output.universe = universe;
    output.start_address = 1;
    output.personality = LayerPersonality::TwoLayers;
    let mut configuration = MediaConfiguration {
        outputs: vec![output],
        ..Default::default()
    };
    configuration.network.art_net_listen = listen;
    configuration.network.sacn_listen = listen;
    configuration
}

fn state_for(configuration: &MediaConfiguration) -> SharedState {
    Arc::new(ArcSwap::from_pointee(MediaState::with_outputs(
        configuration
            .outputs
            .iter()
            .map(|output| OutputState::new(output.id, output.personality))
            .collect(),
    )))
}

/// Layer one selects folder 1, file `file`, at full dimmer.
fn slots(file: u8) -> Vec<u8> {
    let mut slots = vec![0u8; 512];
    slots[0] = 1;
    slots[1] = file;
    slots[14] = 255;
    slots
}

struct Running {
    live: Arc<ArcSwap<MediaConfiguration>>,
    settings: LiveSettings,
    state: SharedState,
    warnings: SharedWarnings,
    shutdown: Shutdown,
    output: media_domain::OutputId,
}

impl Running {
    async fn start(configuration: MediaConfiguration) -> Self {
        let settings = LiveSettings::new();
        let state = state_for(&configuration);
        let output = configuration.outputs[0].id;
        let live = Arc::new(ArcSwap::from_pointee(configuration));
        let warnings = SharedWarnings::default();
        let shutdown = Shutdown::new();
        spawn(
            live.clone(),
            settings.follow(),
            state.clone(),
            shutdown.clone(),
            std::time::Instant::now(),
            crate::dmx::diagnostics(),
            crate::dmx::universe_inputs(),
            warnings.clone(),
        )
        .await;
        Self {
            live,
            settings,
            state,
            warnings,
            shutdown,
            output,
        }
    }

    async fn edit(&self, change: impl FnOnce(&mut MediaConfiguration)) {
        let mut next = MediaConfiguration::clone(&self.live.load());
        change(&mut next);
        self.live.store(Arc::new(next));
        self.settings.changed();
        self.settings.settled().await;
    }

    fn file(&self) -> u8 {
        self.state.load().output(self.output).unwrap().layers[0]
            .address
            .file
    }

    /// Sends one packet at a time until layer one shows `file`, or gives up.
    async fn shows_after_sending(&self, packet: &[u8], to: SocketAddr, file: u8) -> bool {
        let sender = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        for _ in 0..40 {
            sender.send_to(packet, to).await.unwrap();
            tokio::time::sleep(Duration::from_millis(25)).await;
            if self.file() == file {
                return true;
            }
        }
        false
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.shutdown
            .request(crate::shutdown::ShutdownReason::Requested);
    }
}

#[tokio::test]
async fn an_unavailable_art_net_socket_leaves_the_server_able_to_start() {
    let occupied = UdpSocket::bind("127.0.0.1:0").unwrap();
    let running = Running::start(configuration(
        DmxProtocol::ArtNet,
        3,
        occupied.local_addr().unwrap(),
    ))
    .await;
    let warnings = running.warnings.lock().unwrap().clone();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("Pixel is running without Art-Net input"));
}

#[tokio::test]
async fn a_new_universe_and_start_address_apply_without_rebinding() {
    let listen = free_port();
    let running = Running::start(configuration(DmxProtocol::ArtNet, 3, listen)).await;
    assert!(
        running
            .shows_after_sending(&media_net::artnet::encode(3, 0, &slots(4)), listen, 4)
            .await
    );

    running
        .edit(|configuration| {
            configuration.outputs[0].universe = 9;
            configuration.outputs[0].start_address = 11;
        })
        .await;
    let mut moved = vec![0u8; 512];
    moved[10] = 1;
    moved[11] = 6;
    moved[24] = 255;
    assert!(
        running
            .shows_after_sending(&media_net::artnet::encode(9, 1, &moved), listen, 6)
            .await,
        "the same socket now routes universe 9 from address 11"
    );
    assert!(
        !running
            .shows_after_sending(&media_net::artnet::encode(3, 2, &slots(8)), listen, 8)
            .await,
        "the old universe no longer drives the output"
    );
}

#[tokio::test]
async fn a_protocol_or_listen_address_change_rebinds_live() {
    let first = free_port();
    let running = Running::start(configuration(DmxProtocol::ArtNet, 3, first)).await;

    let second = free_port();
    running
        .edit(|configuration| {
            configuration.outputs[0].protocol = DmxProtocol::Sacn;
            configuration.network.sacn_listen = second;
        })
        .await;
    assert!(running.warnings.lock().unwrap().is_empty());
    assert!(
        running
            .shows_after_sending(
                &media_net::sacn::encode(3, 100, 1, [7; 16], &slots(5)),
                second,
                5
            )
            .await,
        "sACN is received on the new address without a restart"
    );
    // The Art-Net port was released, so something else can now take it.
    UdpSocket::bind(first).expect("the unused Art-Net socket was closed");
}

#[tokio::test]
async fn a_live_rebind_that_fails_is_reported_and_recovers() {
    let listen = free_port();
    let running = Running::start(configuration(DmxProtocol::ArtNet, 3, listen)).await;
    let occupied = UdpSocket::bind("127.0.0.1:0").unwrap();
    let taken = occupied.local_addr().unwrap();

    running
        .edit(|configuration| configuration.network.art_net_listen = taken)
        .await;
    assert_eq!(running.warnings.lock().unwrap().len(), 1);

    running
        .edit(|configuration| configuration.network.art_net_listen = listen)
        .await;
    assert!(running.warnings.lock().unwrap().is_empty());
    assert!(
        running
            .shows_after_sending(&media_net::artnet::encode(3, 0, &slots(2)), listen, 2)
            .await
    );
}
