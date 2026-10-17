//! Sole-receiver handoff and bounded passive observations from the owned renderer child.
use super::{VisualizerLauncher, reuse_running_child};
use crate::dmx_input::{DmxInputMonitor, ReceivedDmx};
use parking_lot::Mutex;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use viz_dmx::InputMapping;
use viz_scene::input_monitor::{MAX_MONITOR_BYTES, MonitorContext, MonitorSnapshot, unix_millis};

const MAX_FEED_AGE: Duration = Duration::from_secs(1);
const UNAVAILABLE: &str =
    "Visualizer DMX observations unavailable; the renderer keeps sole network ownership.";

#[derive(Default)]
struct FeedState {
    latest: Option<(Instant, MonitorSnapshot)>,
    warning: Option<&'static str>,
}

pub(super) struct ParentMonitor {
    endpoint: std::net::SocketAddr,
    feed: MonitorFeed,
}

pub(super) struct MonitorFeed {
    state: Arc<Mutex<FeedState>>,
}

impl MonitorFeed {
    fn start(reader: impl Read + Send + 'static) -> Self {
        let state = Arc::new(Mutex::new(FeedState::default()));
        let worker = state.clone();
        if std::thread::Builder::new()
            .name("viz-input-observer".into())
            .spawn(move || read_feed(reader, worker))
            .is_err()
        {
            state.lock().warning = Some("Could not start Visualizer DMX observations.");
        }
        Self { state }
    }

    fn report(&self, context: &MonitorContext) -> ReceivedDmx {
        let state = self.state.lock();
        let Some((receipt, snapshot)) = &state.latest else {
            return unavailable(state.warning.unwrap_or(UNAVAILABLE));
        };
        if &snapshot.context != context {
            return unavailable(
                "Waiting for Visualizer DMX observations for the current show and inputs.",
            );
        }
        let age = unix_millis().checked_sub(snapshot.generated_millis);
        if state.warning.is_some()
            || receipt.elapsed() > MAX_FEED_AGE
            || age.is_none_or(|age| age > MAX_FEED_AGE.as_millis() as u64)
        {
            return snapshot
                .received
                .clone()
                .unavailable(state.warning.unwrap_or(
                    "Visualizer DMX observations stopped updating; held values are not live.",
                ));
        }
        snapshot.received.clone()
    }
}

fn unavailable(warning: &str) -> ReceivedDmx {
    ReceivedDmx {
        universes: vec![],
        inputs: vec![],
        warnings: vec![warning.into()],
    }
}

fn read_feed(reader: impl Read, state: Arc<Mutex<FeedState>>) {
    let mut reader = BufReader::new(reader);
    loop {
        let mut bytes = Vec::new();
        // `take` bounds allocation even when a child writes a line without a terminator.
        match reader
            .by_ref()
            .take((MAX_MONITOR_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
        {
            Ok(0) => {
                state.lock().warning = Some(UNAVAILABLE);
                return;
            }
            Ok(_) if bytes.len() > MAX_MONITOR_BYTES || !bytes.ends_with(b"\n") => {
                state.lock().warning =
                    Some("Visualizer DMX observation exceeded its bounded message size.");
                return;
            }
            Err(_) => {
                state.lock().warning = Some(UNAVAILABLE);
                return;
            }
            _ => {}
        }
        match serde_json::from_slice::<MonitorSnapshot>(&bytes) {
            Ok(snapshot) if snapshot.validate() && snapshot.generated_millis > 0 => {
                *state.lock() = FeedState {
                    latest: Some((Instant::now(), snapshot)),
                    warning: None,
                };
            }
            _ => {
                state.lock().warning = Some(
                    "Visualizer sent an invalid DMX observation; no monitor receiver was opened.",
                )
            }
        }
    }
}

impl VisualizerLauncher {
    pub(crate) fn for_editor_launch(
        address: std::net::SocketAddr,
        served: Option<std::net::SocketAddr>,
        mode: Option<&str>,
        reader: impl Read + Send + 'static,
    ) -> Self {
        if let Some(served) = served
            && mode == Some("stdin")
        {
            Self::with_parent_monitor(address, served, reader)
        } else {
            Self::new(address)
        }
    }

    pub(crate) fn with_parent_monitor(
        address: std::net::SocketAddr,
        served: std::net::SocketAddr,
        reader: impl Read + Send + 'static,
    ) -> Self {
        let mut launcher = Self::new(address);
        launcher.parent = Some(ParentMonitor {
            endpoint: served,
            feed: MonitorFeed::start(reader),
        });
        launcher
    }

    pub(super) fn parent_running(&self) -> Option<Result<bool, String>> {
        self.parent.as_ref().map(|parent| {
            let state = parent.feed.state.lock();
            let stale = state.latest.as_ref().is_some_and(|(receipt, snapshot)| {
                receipt.elapsed() > MAX_FEED_AGE || unix_millis().checked_sub(snapshot.generated_millis)
                    .is_none_or(|age| age > MAX_FEED_AGE.as_millis() as u64)
            });
            if state.warning.is_some() || stale {
                Err("The renderer that opened this Architect window is no longer providing DMX observations. Reopen Architect from that renderer; no second receiver will be started.".into())
            } else { Ok(true) }
        })
    }

    /// The child lifecycle lock always precedes the direct-receiver and passive-cache locks.
    /// This makes stopping the Editor receiver and installing the child one indivisible handoff.
    pub(super) fn open_command(
        &self,
        monitor: &DmxInputMonitor,
        mut command: Command,
    ) -> Result<(), String> {
        if let Some(status) = self.parent_running() {
            return status.map(|_| ());
        }
        let mut current = self.child.lock();
        if let Some(child) = current.as_mut()
            && reuse_running_child(child.try_wait())?
        {
            monitor.stop();
            return Ok(());
        }
        *current = None;
        *self.feed.lock() = None;
        monitor.stop();
        let mut child = command
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| format!("could not open Visualizer: {error}"))?;
        *self.feed.lock() = child.stdout.take().map(MonitorFeed::start);
        *current = Some(child);
        Ok(())
    }

    pub(crate) fn read_received(
        &self,
        monitor: &DmxInputMonitor,
        mappings: Vec<InputMapping>,
        warnings: Vec<String>,
        show_id: uuid::Uuid,
        revision: u64,
    ) -> ReceivedDmx {
        let mut current = self.child.lock();
        if let Some(parent) = &self.parent {
            monitor.stop();
            let context = monitor_context(parent.endpoint, &mappings, show_id, revision);
            let mut report = parent.feed.report(&context);
            report.warnings.extend(warnings);
            return report;
        }
        if let Some(child) = current.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    *current = None;
                    *self.feed.lock() = None;
                }
                status => {
                    monitor.stop();
                    if status.is_err() {
                        return unavailable(
                            "Could not inspect the owned Visualizer; DMX monitor remains passive.",
                        );
                    }
                    let context = monitor_context(self.address, &mappings, show_id, revision);
                    let mut report = self
                        .feed
                        .lock()
                        .as_ref()
                        .map(|feed| feed.report(&context))
                        .unwrap_or_else(|| unavailable(UNAVAILABLE));
                    report.warnings.extend(warnings);
                    return report;
                }
            }
        }
        monitor.read(mappings, warnings)
    }
}

fn monitor_context(
    address: std::net::SocketAddr,
    mappings: &[InputMapping],
    show_id: uuid::Uuid,
    revision: u64,
) -> MonitorContext {
    let ip = if address.ip().is_unspecified() {
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
    } else {
        address.ip()
    };
    MonitorContext {
        producer: "network_receiver".into(),
        endpoint: format!("http://{}", std::net::SocketAddr::new(ip, address.port())),
        show_id: Some(show_id),
        show_revision: revision,
        mappings: mappings
            .iter()
            .map(|mapping| format!("{mapping:?}"))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    use viz_dmx::{Delivery, DmxReceiver, Protocol};

    fn mapping(port: u16) -> InputMapping {
        InputMapping {
            id: "owned-input".into(),
            protocol: Protocol::ArtNet,
            logical_universe: 1,
            destination_universe: 1,
            delivery: Delivery::Unicast,
            bind: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            priority: 100,
            enabled: true,
        }
    }

    // A genuine process with a genuine UDP receiver; no GPU, app launch, or external desk.
    #[test]
    #[ignore]
    fn udp_child_fixture() {
        let port = std::env::var("TL829_TEST_PORT").unwrap().parse().unwrap();
        let receiver = DmxReceiver::start(vec![mapping(port)], Instant::now());
        println!("READY");
        std::io::stdout().flush().unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            println!("COUNT {}", receiver.accepted_packets());
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn values_does_not_compete_with_the_owned_udp_child() {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        drop(socket);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "visualizer::input_monitor::tests::udp_child_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("TL829_TEST_PORT", port.to_string())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        while !line.contains("READY") {
            line.clear();
            output.read_line(&mut line).unwrap();
        }
        let launcher = VisualizerLauncher::new(SocketAddr::from((Ipv4Addr::LOCALHOST, 1)));
        *launcher.child.lock() = Some(child);
        let monitor = DmxInputMonitor::default();
        launcher.read_received(&monitor, vec![mapping(port)], vec![], uuid::Uuid::nil(), 0);
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        for sequence in 1..=32 {
            let mut packet = b"Art-Net\0".to_vec();
            packet.extend_from_slice(&0x5000_u16.to_le_bytes());
            packet.extend_from_slice(&[0, 14, sequence, 0, 1, 0, 0, 2, sequence, 0]);
            sender
                .send_to(&packet, (Ipv4Addr::LOCALHOST, port))
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        std::thread::sleep(Duration::from_millis(80));
        let received =
            launcher.read_received(&monitor, vec![mapping(port)], vec![], uuid::Uuid::nil(), 0);
        let parent_count: u64 = received.inputs.iter().map(|row| row.accepted_packets).sum();
        let mut child_count = 0;
        for _ in 0..25 {
            line.clear();
            output.read_line(&mut line).unwrap();
            if let Some(count) = line.trim().strip_prefix("COUNT ") {
                child_count = count.parse::<u64>().unwrap();
            }
        }
        launcher.stop().unwrap();
        monitor.stop();
        assert_eq!(
            child_count, 32,
            "opening Values must leave all fixed-source packets with renderer (parent saw {parent_count})"
        );
    }

    fn free_port() -> u16 {
        UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    fn context(port: u16) -> MonitorContext {
        MonitorContext {
            producer: "network_receiver".into(),
            endpoint: "http://127.0.0.1:1".into(),
            show_id: Some(uuid::Uuid::nil()),
            show_revision: 7,
            mappings: vec![format!("{:?}", mapping(port))],
        }
    }

    fn sample(port: u16) -> MonitorSnapshot {
        use viz_scene::input_monitor::ReceivedUniverse;
        MonitorSnapshot {
            version: 1,
            generated_millis: unix_millis(),
            context: context(port),
            received: ReceivedDmx {
                universes: vec![ReceivedUniverse {
                    universe: 1,
                    slots: Some(vec![123; 512]),
                    live: true,
                    rate_hz: 44.0,
                    protocol: Some("artnet".into()),
                }],
                inputs: vec![],
                warnings: vec![],
            },
        }
    }

    fn read(launcher: &VisualizerLauncher, monitor: &DmxInputMonitor, port: u16) -> ReceivedDmx {
        launcher.read_received(monitor, vec![mapping(port)], vec![], uuid::Uuid::nil(), 7)
    }

    #[track_caller]
    fn until(mut check: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while !check() {
            assert!(
                Instant::now() < deadline,
                "bounded observation did not arrive"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn send(sender: &UdpSocket, port: u16, sequence: u8) {
        let mut packet = b"Art-Net\0".to_vec();
        packet.extend_from_slice(&0x5000_u16.to_le_bytes());
        packet.extend_from_slice(&[0, 14, sequence, 0, 1, 0, 0, 2, 123, 42]);
        sender
            .send_to(&packet, (Ipv4Addr::LOCALHOST, port))
            .unwrap();
    }

    fn fixture_command(port: u16) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "visualizer::input_monitor::tests::passive_udp_child_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("TL829_TEST_PORT", port.to_string());
        command
    }

    #[test]
    #[ignore]
    fn passive_udp_child_fixture() {
        let port = std::env::var("TL829_TEST_PORT").unwrap().parse().unwrap();
        let monitor = DmxInputMonitor::default();
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            let snapshot = MonitorSnapshot {
                version: 1,
                generated_millis: unix_millis(),
                context: context(port),
                received: monitor.read(vec![mapping(port)], vec![]),
            };
            println!("{}", serde_json::to_string(&snapshot).unwrap());
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn real_child_passive_feed_keeps_udp_source_and_slots_then_restores_standalone_values() {
        let port = free_port();
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let launcher = VisualizerLauncher::new(SocketAddr::from((Ipv4Addr::LOCALHOST, 1)));
        let monitor = DmxInputMonitor::default();
        // Existing standalone Values receiver is live before Open Viz performs the handoff.
        read(&launcher, &monitor, port);
        send(&sender, port, 1);
        until(|| {
            read(&launcher, &monitor, port)
                .universes
                .iter()
                .any(|row| row.live)
        });
        launcher
            .open_command(&monitor, fixture_command(port))
            .unwrap();
        until(|| read(&launcher, &monitor, port).inputs.len() == 1);
        // Repeated Open Viz is idempotent, including while Values polls.
        let pid = launcher.child.lock().as_ref().unwrap().id();
        launcher
            .open_command(&monitor, fixture_command(port))
            .unwrap();
        assert_eq!(launcher.child.lock().as_ref().unwrap().id(), pid);
        // Held refreshes arrive outside the receiver's existing 6 ms duplicate window.
        for sequence in 1..=32 {
            send(&sender, port, sequence);
            std::thread::sleep(Duration::from_millis(10));
        }
        until(|| {
            read(&launcher, &monitor, port)
                .inputs
                .iter()
                .any(|row| row.accepted_packets == 32)
        });
        let report = read(&launcher, &monitor, port);
        assert_eq!(report.universes[0].slots.as_ref().unwrap()[..2], [123, 42]);
        assert!(report.universes[0].live);
        assert!(
            report.inputs[0]
                .source
                .as_ref()
                .unwrap()
                .contains(&sender.local_addr().unwrap().to_string())
        );
        assert_eq!(
            report.inputs[0].accepted_packets, 32,
            "passive polling did not change accepted receipts"
        );
        launcher.stop().unwrap();
        read(&launcher, &monitor, port);
        send(&sender, port, 1);
        until(|| {
            read(&launcher, &monitor, port)
                .inputs
                .iter()
                .any(|row| row.accepted_packets == 1)
        });
        monitor.stop();
    }

    #[test]
    fn concurrent_values_poll_and_open_cannot_leave_a_competing_editor_receiver() {
        let port = free_port();
        let launcher = Arc::new(VisualizerLauncher::new(SocketAddr::from((
            Ipv4Addr::LOCALHOST,
            1,
        ))));
        let monitor = Arc::new(DmxInputMonitor::default());
        let polling = launcher.clone();
        let values = monitor.clone();
        let reader = std::thread::spawn(move || {
            for _ in 0..100 {
                read(&polling, &values, port);
                std::thread::yield_now();
            }
        });
        launcher
            .open_command(&monitor, fixture_command(port))
            .unwrap();
        reader.join().unwrap();
        until(|| read(&launcher, &monitor, port).inputs.len() == 1);
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        // Held refreshes arrive outside the receiver's existing 6 ms duplicate window.
        for sequence in 1..=32 {
            send(&sender, port, sequence);
            std::thread::sleep(Duration::from_millis(10));
        }
        until(|| {
            read(&launcher, &monitor, port)
                .inputs
                .iter()
                .any(|row| row.accepted_packets == 32)
        });
        launcher.stop().unwrap();
        monitor.stop();
    }

    #[test]
    fn feed_must_match_current_provider_show_revision_and_every_effective_mapping() {
        let port = free_port();
        let snapshot = sample(port);
        let feed = MonitorFeed {
            state: Arc::new(Mutex::new(FeedState {
                latest: Some((Instant::now(), snapshot.clone())),
                warning: None,
            })),
        };
        assert!(feed.report(&context(port)).universes[0].live);
        let original = context(port);
        for field in 0..5 {
            let mut changed = original.clone();
            match field {
                0 => changed.producer = "preview".into(),
                1 => changed.endpoint = "other".into(),
                2 => changed.show_id = Some(uuid::Uuid::new_v4()),
                3 => changed.show_revision += 1,
                _ => changed.mappings[0].push_str("changed priority/interface/destination"),
            }
            let report = feed.report(&changed);
            assert!(
                report.universes.is_empty(),
                "old frames cannot cross context {field}"
            );
            assert!(!report.warnings.is_empty());
        }
    }

    #[test]
    fn stalled_or_delayed_feed_ages_without_polls_refreshing_held_freshness() {
        let port = free_port();
        let snapshot = sample(port);
        let feed = MonitorFeed {
            state: Arc::new(Mutex::new(FeedState {
                latest: Some((Instant::now() - Duration::from_secs(2), snapshot)),
                warning: None,
            })),
        };
        for _ in 0..3 {
            let report = feed.report(&context(port));
            assert!(!report.universes[0].live);
            assert_eq!(report.universes[0].rate_hz, 0.0);
            assert_eq!(report.universes[0].slots.as_ref().unwrap()[0], 123);
        }
        let mut delayed = sample(port);
        delayed.generated_millis -= 2000;
        feed.state.lock().latest = Some((Instant::now(), delayed));
        assert!(
            !feed.report(&context(port)).universes[0].live,
            "new arrival cannot bless an old blocked-writer snapshot"
        );
        feed.state
            .lock()
            .latest
            .as_mut()
            .unwrap()
            .1
            .generated_millis = unix_millis() + 10000;
        assert!(
            !feed.report(&context(port)).universes[0].live,
            "clock reversal is unavailable"
        );
    }

    #[test]
    fn malformed_oversized_and_closed_feed_never_open_fallback_while_child_lives() {
        let port = free_port();
        let launcher = VisualizerLauncher::new(SocketAddr::from((Ipv4Addr::LOCALHOST, 1)));
        *launcher.child.lock() = Some(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "visualizer::input_monitor::tests::udp_child_fixture",
                    "--ignored",
                    "--nocapture",
                ])
                .env("TL829_TEST_PORT", free_port().to_string())
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let monitor = DmxInputMonitor::default();
        for bytes in [b"not JSON\n".to_vec(), vec![b'x'; MAX_MONITOR_BYTES + 1], {
            let mut bytes = serde_json::to_vec(&sample(port)).unwrap();
            bytes.push(b'\n');
            bytes
        }] {
            let state = Arc::new(Mutex::new(FeedState::default()));
            read_feed(std::io::Cursor::new(bytes), state.clone());
            assert!(state.lock().warning.is_some());
            *launcher.feed.lock() = Some(MonitorFeed { state });
            let report = read(&launcher, &monitor, port);
            assert!(!report.warnings.is_empty());
            assert!(report.universes.iter().all(|row| !row.live));
            let probe = UdpSocket::bind((Ipv4Addr::LOCALHOST, port))
                .expect("no fallback listener was opened");
            drop(probe);
        }
        launcher.stop().unwrap();
    }

    #[test]
    fn failed_spawn_and_confirmed_child_exit_restore_direct_values() {
        let port = free_port();
        let launcher = VisualizerLauncher::new(SocketAddr::from((Ipv4Addr::LOCALHOST, 1)));
        let monitor = DmxInputMonitor::default();
        assert!(
            launcher
                .open_command(&monitor, Command::new("/no-such-tl829-renderer"))
                .is_err()
        );
        assert_eq!(read(&launcher, &monitor, port).inputs.len(), 1);
        monitor.stop();
        let child = Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        *launcher.child.lock() = Some(child);
        until(|| {
            read(&launcher, &monitor, port);
            launcher.child.lock().is_none()
        });
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        send(&sender, port, 1);
        until(|| {
            read(&launcher, &monitor, port)
                .inputs
                .iter()
                .any(|row| row.accepted_packets == 1)
        });
        monitor.stop();
    }

    #[test]
    #[ignore]
    fn reverse_udp_child_fixture() {
        let port = std::env::var("TL829_TEST_PORT").unwrap().parse().unwrap();
        let launcher = VisualizerLauncher::with_parent_monitor(
            SocketAddr::from((Ipv4Addr::LOCALHOST, 9)),
            SocketAddr::from((Ipv4Addr::LOCALHOST, 1)),
            std::io::stdin(),
        );
        let monitor = DmxInputMonitor::default();
        read(&launcher, &monitor, port);
        println!("READY");
        std::io::stdout().flush().unwrap();
        let until = Instant::now() + Duration::from_secs(3);
        while Instant::now() < until {
            let report = read(&launcher, &monitor, port);
            println!(
                "COUNT {}",
                report
                    .inputs
                    .iter()
                    .map(|row| row.accepted_packets)
                    .sum::<u64>()
            );
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn reverse_owned_child_observes_parent_udp_without_stealing_a_single_packet() {
        let port = free_port();
        let receiver = DmxReceiver::start(vec![mapping(port)], Instant::now());
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "visualizer::input_monitor::tests::reverse_udp_child_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("TL829_TEST_PORT", port.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        while !line.contains("READY") {
            line.clear();
            output.read_line(&mut line).unwrap();
        }
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut frames = std::collections::BTreeMap::new();
        for sequence in 1..=32 {
            send(&sender, port, sequence);
            std::thread::sleep(Duration::from_millis(10));
            for frame in receiver.drain_changed() {
                frames.insert(frame.logical_universe, frame.slots.to_vec());
            }
            let snapshot = MonitorSnapshot {
                version: 1,
                generated_millis: unix_millis(),
                context: context(port),
                received: ReceivedDmx::from_receiver(
                    &frames,
                    std::iter::once(1),
                    receiver.universes(),
                    receiver.status(),
                    vec![],
                ),
            };
            // In fail-first the legacy child ignores stdin; BrokenPipe must not abort the real UDP proof.
            let _ = writeln!(input, "{}", serde_json::to_string(&snapshot).unwrap());
        }
        let parent_count = receiver.accepted_packets();
        let mut child_count = 0;
        for _ in 0..60 {
            line.clear();
            output.read_line(&mut line).unwrap();
            if let Some(count) = line.trim().strip_prefix("COUNT ") {
                child_count = count.parse::<u64>().unwrap();
            }
            if child_count == 32 {
                break;
            }
        }
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(
            (parent_count, child_count),
            (32, 32),
            "Editor child must passively observe all actual parent receipts"
        );
    }

    #[test]
    fn explicit_parent_attachment_reuses_parent_and_cannot_fallback_after_feed_loss() {
        let port = free_port();
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 9));
        let mut launcher = VisualizerLauncher::with_parent_monitor(
            address,
            SocketAddr::from((Ipv4Addr::LOCALHOST, 1)),
            std::io::empty(),
        );
        until(|| {
            launcher
                .parent
                .as_ref()
                .unwrap()
                .feed
                .state
                .lock()
                .warning
                .is_some()
        });
        let parent = launcher.parent.as_mut().unwrap();
        *parent.feed.state.lock() = FeedState {
            latest: Some((Instant::now(), sample(port))),
            warning: None,
        };
        assert!(launcher.parent_running().unwrap().unwrap());
        let monitor = DmxInputMonitor::default();
        // A nonexistent executable proves Open Viz reused the parent instead of spawning.
        launcher
            .open_command(&monitor, Command::new("/no-such-renderer"))
            .unwrap();
        assert!(launcher.child.lock().is_none());
        assert!(
            read(&launcher, &monitor, port).universes[0].live,
            "actual --serve endpoint, not random Open Viz endpoint"
        );
        for warning in ["closed", "malformed", "oversized"] {
            launcher.parent.as_ref().unwrap().feed.state.lock().warning = Some(warning);
            assert!(launcher.parent_running().unwrap().is_err());
            assert!(
                launcher
                    .open_command(&monitor, Command::new("/no-such-renderer"))
                    .is_err()
            );
            let report = read(&launcher, &monitor, port);
            assert!(!report.universes[0].live);
            let probe = UdpSocket::bind((Ipv4Addr::LOCALHOST, port))
                .expect("permanent passive attachment never falls back after EOF/error");
            drop(probe);
        }
        launcher.stop().unwrap();
        assert!(
            launcher.parent.is_some(),
            "closing child ownership never guesses parent death"
        );
    }

    #[test]
    fn parent_passivity_requires_both_explicit_opt_in_and_served_endpoint() {
        struct NotRead;
        impl Read for NotRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                panic!("standalone must not read stdin")
            }
        }
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 1));
        for (served, mode) in [
            (Some(address), None),
            (Some(address), Some("1")),
            (None, Some("stdin")),
        ] {
            let launcher = VisualizerLauncher::for_editor_launch(address, served, mode, NotRead);
            assert!(launcher.parent.is_none());
            let monitor = DmxInputMonitor::default();
            assert_eq!(read(&launcher, &monitor, free_port()).inputs.len(), 1);
            monitor.stop();
        }
    }
}
