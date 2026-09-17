use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use media_application::configuration::OutputConfiguration;
use media_domain::{ResolvedTempo, SpeedGroupId, TempoSource};
use media_net::SpeedGroupUpdate;
use media_net::speed_group_osc::{decode, encode};

use super::*;

fn free_port() -> SocketAddr {
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

fn configuration(listen: Option<SocketAddr>, source: TempoSource) -> MediaConfiguration {
    let mut output = OutputConfiguration::new("Main");
    output.tempo_source = source;
    let mut configuration = MediaConfiguration {
        outputs: vec![output],
        ..Default::default()
    };
    configuration.network.speed_group_endpoint = listen;
    configuration
}

/// Starts reception the way the process does, and returns what an edit goes through.
fn start(
    configuration: &MediaConfiguration,
    reception: &SharedSpeedGroups,
    shutdown: &Shutdown,
    started: Instant,
) -> (
    Arc<ArcSwap<MediaConfiguration>>,
    crate::live_settings::LiveSettings,
) {
    let live = Arc::new(ArcSwap::from_pointee(configuration.clone()));
    let settings = crate::live_settings::LiveSettings::new();
    spawn(
        live.clone(),
        settings.follow(),
        reception,
        shutdown,
        started,
    );
    (live, settings)
}

fn group(number: u32) -> TempoSource {
    TempoSource::SpeedGroup {
        group_id: SpeedGroupId::new(number),
    }
}

fn update(sequence: u32, number: u32, bpm: f64) -> SpeedGroupUpdate {
    SpeedGroupUpdate {
        source: "light-desk".to_owned(),
        sequence,
        group: number,
        bpm,
        beat_phase: 0.5,
        running: true,
    }
}

fn status(reception: &SharedSpeedGroups, started: Instant) -> media_http::SpeedGroupTelemetry {
    (diagnostics(reception, started))()
}

async fn wait_for(
    reception: &SharedSpeedGroups,
    started: Instant,
    ready: impl Fn(&media_http::SpeedGroupTelemetry) -> bool,
) -> media_http::SpeedGroupTelemetry {
    for _ in 0..200 {
        let current = status(reception, started);
        if ready(&current) {
            return current;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!(
        "reception never became ready: {:?}",
        status(reception, started)
    );
}

#[tokio::test]
async fn a_desk_datagram_retimes_the_output_that_follows_its_group() {
    let listen = free_port();
    let configuration = configuration(Some(listen), group(2));
    let reception = shared();
    let shutdown = Shutdown::new();
    let started = Instant::now();
    let _live = start(&configuration, &reception, &shutdown, started);
    assert_eq!(status(&reception, started).connection, "waiting");

    let desk = UdpSocket::bind("127.0.0.1:0").unwrap();
    desk.send_to(&encode(&update(1, 1, 90.0)), listen).unwrap();
    desk.send_to(&encode(&update(2, 2, 128.0)), listen).unwrap();
    desk.send_to(b"not osc", listen).unwrap();

    let heard = wait_for(&reception, started, |status| {
        status.groups.len() == 2 && status.rejected == 1
    })
    .await;
    assert_eq!(heard.connection, "connected");
    assert_eq!(heard.sender.as_deref(), Some("light-desk"));
    assert_eq!(
        heard.sender_address,
        Some(desk.local_addr().unwrap().to_string())
    );
    assert!(heard.rejections[0].reason.contains("not an OSC packet"));

    let output = configuration.outputs[0].id;
    let tempo = output_tempo(&configuration, output, &reception);
    assert_eq!(
        tempo.resolve(Some(60), stamp(started)),
        ResolvedTempo::Live { bpm: 128.0 },
        "the followed group wins and the layer channel is not consulted"
    );

    shutdown.request(crate::ShutdownReason::Requested);
}

#[test]
fn an_output_on_its_channel_ignores_every_speed_group() {
    let configuration = configuration(None, TempoSource::PlaybackBpmChannel);
    let reception = shared();
    let started = Instant::now();
    apply(
        &reception,
        (free_port(), Ok(vec![update(1, 1, 128.0)])),
        stamp(started),
    );
    let tempo = output_tempo(&configuration, configuration.outputs[0].id, &reception);
    assert_eq!(tempo.speed_group, None);
    assert_eq!(
        tempo.resolve(Some(100), stamp(started)),
        ResolvedTempo::Live { bpm: 100.0 }
    );
    assert_eq!(
        status(&reception, started).connection,
        "disabled",
        "no listen address means reception is off"
    );
}

#[test]
fn loss_holds_the_last_tempo_and_a_reconnect_resumes_live() {
    let configuration = configuration(None, group(1));
    let output = configuration.outputs[0].id;
    let reception = shared();
    *lock(&reception) = SpeedGroupReception::listening(free_port());
    let desk = free_port();
    let at = Timestamp::from_millis;

    apply(&reception, (desk, Ok(vec![update(10, 1, 120.0)])), at(0));
    let tempo = output_tempo(&configuration, output, &reception);
    assert_eq!(
        tempo.resolve(None, at(1_000)),
        ResolvedTempo::Live { bpm: 120.0 }
    );
    assert_eq!(
        tempo.resolve(None, at(2_000)),
        ResolvedTempo::Stale { bpm: 120.0 },
        "a lost desk leaves the output on its last tempo, flagged stale"
    );

    // The desk restarts its counter after the network comes back.
    apply(&reception, (desk, Ok(vec![update(1, 1, 96.0)])), at(3_000));
    let tempo = output_tempo(&configuration, output, &reception);
    assert_eq!(
        tempo.resolve(None, at(3_100)),
        ResolvedTempo::Live { bpm: 96.0 }
    );
}

#[test]
fn invalid_values_leave_the_tempo_alone_and_are_reported() {
    let configuration = configuration(None, group(1));
    let reception = shared();
    let started = Instant::now();
    let desk = free_port();
    apply(
        &reception,
        (desk, Ok(vec![update(1, 1, 120.0)])),
        stamp(started),
    );
    apply(
        &reception,
        (desk, Ok(vec![update(2, 1, f64::INFINITY)])),
        stamp(started),
    );
    let mut wrong = encode(&update(3, 1, 60.0));
    wrong[1] = b'x';
    apply(&reception, (desk, decode(&wrong)), stamp(started));

    let tempo = output_tempo(&configuration, configuration.outputs[0].id, &reception);
    assert_eq!(tempo.speed_group.map(|snapshot| snapshot.bpm), Some(120.0));
    let reported = status(&reception, started);
    assert_eq!(reported.accepted, 1);
    assert_eq!(reported.rejected, 2);
    assert!(
        reported.rejections[0]
            .reason
            .contains("unexpected OSC address")
    );
    assert!(reported.rejections[1].reason.contains("BPM"));
}

#[tokio::test]
async fn a_port_that_is_already_taken_is_reported_to_the_operator() {
    let taken = UdpSocket::bind("127.0.0.1:0").unwrap();
    let configuration = configuration(Some(taken.local_addr().unwrap()), group(1));
    let reception = shared();
    let started = Instant::now();
    let _live = start(&configuration, &reception, &Shutdown::new(), started);
    let reported = status(&reception, started);
    assert_eq!(reported.connection, "unavailable");
    assert!(reported.detail.unwrap().contains("Speed Groups"));
}

#[tokio::test]
async fn a_new_listen_address_is_taken_without_a_restart() {
    let first = free_port();
    let reception = shared();
    let shutdown = Shutdown::new();
    let started = Instant::now();
    let (live, settings) = start(
        &configuration(Some(first), group(1)),
        &reception,
        &shutdown,
        started,
    );

    let second = free_port();
    live.store(Arc::new(configuration(Some(second), group(1))));
    settings.changed();
    settings.settled().await;
    assert_eq!(
        status(&reception, started).listening,
        Some(second.to_string())
    );
    let desk = UdpSocket::bind("127.0.0.1:0").unwrap();
    desk.send_to(&encode(&update(1, 1, 100.0)), second).unwrap();
    wait_for(&reception, started, |status| status.accepted == 1).await;
    UdpSocket::bind(first).expect("the old Speed Group socket was closed");

    live.store(Arc::new(configuration(None, group(1))));
    settings.changed();
    settings.settled().await;
    assert_eq!(status(&reception, started).listening, None);
    UdpSocket::bind(second).expect("turning reception off closed the socket");
    shutdown.request(crate::shutdown::ShutdownReason::Requested);
}
