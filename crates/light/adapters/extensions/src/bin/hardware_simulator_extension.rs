#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use light_extensions_contract::{
    CanonicalControlIntent, Configure, ControlInput, ControlInputEvent, DRAFT_PROTOCOL_V1,
    ExtensionCapability, ExtensionHello, FeedbackDelta, FeedbackSnapshot, Frame, FrameDecoder,
    HealthReport, HealthStatus, HighlightControlAction, Message, ModifierKey, NavigationAction,
    PlaybackControl, ProgrammerKey, SpeedGroupControl, encode_frame, validate_control_input,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use light_extensions_host::{
    ArtifactManifest, ControlDeclaration, ControlKind, DiscoveryOptions,
    ExtensionInstanceConfiguration, ExtensionManifest, ExtensionsConfiguration, FeedbackFeature,
    HostApiRange, LicenseManifest, Multiplicity, PackageFileManifest, PackageLimits, ProtocolRange,
    VendorManifest, install_staged_package, load_extensions_configuration,
    write_extensions_configuration,
};

const DEFAULT_RELAY_ADDRESS: &str = "127.0.0.1:49152";
const RELAY_ADDRESS_ENV: &str = "TOSKLIGHT_HARDWARE_SIMULATOR_RELAY_ADDR";
const RELAY_ADDRESS_SETTING: &str = "relay_address";
const MAX_RELAY_LINE_BYTES: usize = 64 * 1024;
const SIMULATOR_EXTENSION_ID: &str = "de.tosklight.hardware-simulator";
const SIMULATOR_INSTANCE_ID: &str = "hardware-simulator";

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| argument == "-h" || argument == "--help")
    {
        print_help();
        return;
    }
    let result = if let [flag, extensions_directory, configuration_path] = arguments.as_slice()
        && flag == "--install"
    {
        install(
            Path::new(extensions_directory),
            Path::new(configuration_path),
        )
    } else if arguments.is_empty() {
        run()
    } else {
        Err(
            "usage: tl-hardware-simulator-extension [--install <extensions-dir> <extensions.json>]"
                .into(),
        )
    };
    if let Err(error) = result {
        eprintln!("hardware simulator extension failed: {error}");
        std::process::exit(2);
    }
}

fn print_help() {
    println!(
        "tl-hardware-simulator-extension\n\
         \nA supervised ToskLight native control-surface extension for the Hardware Controls simulator.\n\
         It completes the native-extension handshake over stdin/stdout and accepts JSON Lines\n\
         control input on a loopback TCP relay.\n\
         \nRelay address precedence:\n\
         1. {RELAY_ADDRESS_ENV}\n\
         2. Configure setting `{RELAY_ADDRESS_SETTING}`\n\
         3. {DEFAULT_RELAY_ADDRESS}\n\
         \nThe relay must resolve to a loopback address."
    );
    println!(
        "\nInstall or update this executable as a locally approved simulator package:\n\
         tl-hardware-simulator-extension --install <extensions-dir> <extensions.json>"
    );
}

fn install(extensions_directory: &Path, configuration_path: &Path) -> Result<(), String> {
    let loaded = load_extensions_configuration(configuration_path);
    if let Some(diagnostic) = loaded.diagnostic {
        return Err(format!(
            "refusing to replace invalid extensions configuration: {diagnostic}"
        ));
    }
    let mut configuration: ExtensionsConfiguration = loaded.configuration;
    fs::create_dir_all(extensions_directory)
        .map_err(|error| format!("cannot create extensions directory: {error}"))?;
    let executable_name = if cfg!(windows) {
        "tl-hardware-simulator-extension.exe"
    } else {
        "tl-hardware-simulator-extension"
    };
    let relative_executable = PathBuf::from("bin").join(executable_name);
    let staging = extensions_directory.join(format!(".staging-simulator-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(staging.join("bin"))
        .map_err(|error| format!("cannot create simulator staging directory: {error}"))?;
    let source = std::env::current_exe()
        .map_err(|error| format!("cannot locate simulator executable: {error}"))?;
    let staged_executable = staging.join(&relative_executable);
    if let Err(error) = fs::copy(&source, &staged_executable) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("cannot stage simulator executable: {error}"));
    }
    let executable_digest = sha256_file(&staged_executable)?;
    let (controls, bindings) = simulator_controls();
    let target = light_extensions_host::PlatformTarget::current();
    let manifest = ExtensionManifest {
        manifest_version: 1,
        id: SIMULATOR_EXTENSION_ID.into(),
        name: "ToskLight Hardware Simulator".into(),
        vendor: VendorManifest {
            name: "ToskLight".into(),
            url: None,
        },
        version: env!("CARGO_PKG_VERSION").into(),
        description: "Typed native-extension relay for the ToskLight Hardware Controls simulator"
            .into(),
        license: LicenseManifest {
            name: "ToskLight repository license".into(),
            url: None,
        },
        source_url: None,
        protocol: ProtocolRange {
            minimum: 1,
            maximum: 1,
        },
        host_api: HostApiRange {
            minimum: 1,
            maximum: 1,
        },
        files: vec![PackageFileManifest {
            path: relative_executable.to_string_lossy().replace('\\', "/"),
            sha256: executable_digest,
        }],
        artifacts: vec![ArtifactManifest {
            os: target.os,
            architecture: target.architecture,
            executable: relative_executable.to_string_lossy().replace('\\', "/"),
        }],
        capabilities: BTreeSet::from([ExtensionCapability::ControlSurface]),
        controls,
        telemetry_channels: Vec::new(),
        device_actions: Vec::new(),
        device_matches: Vec::new(),
        transport_metadata: Some(serde_json::json!({
            "protocol": "tcp-json-lines",
            "default_address": DEFAULT_RELAY_ADDRESS,
        })),
        feedback_features: vec![
            FeedbackFeature::Availability,
            FeedbackFeature::Enabled,
            FeedbackFeature::Selected,
            FeedbackFeature::Warning,
            FeedbackFeature::Error,
            FeedbackFeature::Lamp,
            FeedbackFeature::Blink,
            FeedbackFeature::SemanticColor,
            FeedbackFeature::RgbColor,
            FeedbackFeature::MotorValue,
            FeedbackFeature::EncoderRing,
            FeedbackFeature::Text,
        ],
        configuration_schema_version: 1,
        multiplicity: Multiplicity::Single,
        limits: PackageLimits::default(),
        reverse_engineered: false,
        signature: None,
    };
    fs::write(
        staging.join("extension.json"),
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("cannot encode simulator manifest: {error}"))?,
    )
    .map_err(|error| format!("cannot write simulator manifest: {error}"))?;

    let installed = install_staged_package(
        extensions_directory,
        &staging,
        SIMULATOR_EXTENSION_ID,
        &DiscoveryOptions::default(),
    )
    .map_err(|error| format!("cannot install simulator package: {error}"))?;

    configuration.approved_packages.insert(
        SIMULATOR_EXTENSION_ID.into(),
        installed.package_digest.clone(),
    );
    let existing = configuration
        .instances
        .iter()
        .find(|instance| instance.id == SIMULATOR_INSTANCE_ID)
        .cloned();
    configuration
        .instances
        .retain(|instance| instance.id != SIMULATOR_INSTANCE_ID);
    configuration
        .instances
        .push(ExtensionInstanceConfiguration {
            id: SIMULATOR_INSTANCE_ID.into(),
            extension_id: SIMULATOR_EXTENSION_ID.into(),
            enabled: true,
            desk_id: existing
                .as_ref()
                .and_then(|instance| instance.desk_id.clone())
                .or_else(|| Some("main".into())),
            device: None,
            settings: existing.map_or_else(BTreeMap::new, |instance| instance.settings),
            control_bindings: bindings,
            device_action_permissions: BTreeSet::new(),
        });
    write_extensions_configuration(configuration_path, &configuration)?;
    println!(
        "installed {} ({}) and enabled instance {}",
        installed.extension_id, installed.package_digest, SIMULATOR_INSTANCE_ID
    );
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read staged executable: {error}"))?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// The control surface the simulator offers while it is being assembled. It
/// carries both halves an extension has to declare: the controls the desk shows
/// on the simulated panel, and the intent each one fires when an operator hits
/// it.
struct ControlSheet {
    controls: Vec<ControlDeclaration>,
    bindings: BTreeMap<String, CanonicalControlIntent>,
}

impl ControlSheet {
    /// Records one control under an id and says what pressing or turning it
    /// means to the desk.
    fn add(&mut self, id: String, kind: ControlKind, intent: CanonicalControlIntent) {
        self.controls.push(ControlDeclaration {
            id: id.clone(),
            kind,
        });
        self.bindings.insert(id, intent);
    }
}

/// Lays out the keypad half of the simulated desk: Record and Shift, the digits
/// and operators an operator types on the command line, and the Menu key that
/// leaves the command line for navigation.
fn add_programmer_keys(sheet: &mut ControlSheet) {
    sheet.add(
        "programmer-record".into(),
        ControlKind::Button,
        CanonicalControlIntent::ProgrammerKey {
            key: ProgrammerKey::Record,
        },
    );
    sheet.add(
        "programmer-shift".into(),
        ControlKind::Button,
        CanonicalControlIntent::Modifier {
            modifier: ModifierKey::Shift,
        },
    );
    for (name, key) in [
        ("digit-0", ProgrammerKey::Zero),
        ("digit-1", ProgrammerKey::One),
        ("digit-2", ProgrammerKey::Two),
        ("digit-3", ProgrammerKey::Three),
        ("digit-4", ProgrammerKey::Four),
        ("digit-5", ProgrammerKey::Five),
        ("digit-6", ProgrammerKey::Six),
        ("digit-7", ProgrammerKey::Seven),
        ("digit-8", ProgrammerKey::Eight),
        ("digit-9", ProgrammerKey::Nine),
        ("plus", ProgrammerKey::Plus),
        ("minus", ProgrammerKey::Minus),
        ("dot", ProgrammerKey::Point),
        ("at", ProgrammerKey::At),
        ("enter", ProgrammerKey::Enter),
        ("clear", ProgrammerKey::Clear),
        ("undo", ProgrammerKey::Undo),
        ("group", ProgrammerKey::Group),
        ("cue", ProgrammerKey::Cue),
        ("playback", ProgrammerKey::Playback),
        ("off", ProgrammerKey::Off),
        ("preload", ProgrammerKey::Preload),
        ("del", ProgrammerKey::Delete),
        ("cpy", ProgrammerKey::Copy),
        ("mov", ProgrammerKey::Move),
        ("set", ProgrammerKey::Set),
        ("time", ProgrammerKey::Time),
        ("thru", ProgrammerKey::Thru),
        ("div", ProgrammerKey::Divide),
        ("backspace", ProgrammerKey::Backspace),
        ("escape", ProgrammerKey::Escape),
        ("prog-playback", ProgrammerKey::EncoderPlayback),
        ("page-up", ProgrammerKey::PageUp),
        ("page-down", ProgrammerKey::PageDown),
        ("align", ProgrammerKey::Align),
    ] {
        sheet.add(
            format!("programmer-{name}"),
            ControlKind::Button,
            CanonicalControlIntent::ProgrammerKey { key },
        );
    }
    sheet.add(
        "programmer-menu".into(),
        ControlKind::Button,
        CanonicalControlIntent::Navigation {
            action: NavigationAction::Menu,
        },
    );
}

/// Lays out the wheels and the movement keys beside them: the six encoders,
/// each of which also presses, the four cursor keys, and the Highlight keys an
/// operator uses to walk through the current selection.
fn add_encoders_and_navigation(sheet: &mut ControlSheet) {
    for index in 1..=6 {
        let intent = CanonicalControlIntent::Encoder { index };
        sheet.add(
            format!("encoder-{index}-turn"),
            ControlKind::RelativeEncoder,
            intent.clone(),
        );
        sheet.add(
            format!("encoder-{index}-press"),
            ControlKind::Button,
            intent,
        );
    }
    for (name, action) in [
        ("up", NavigationAction::Up),
        ("down", NavigationAction::Down),
        ("left", NavigationAction::Left),
        ("right", NavigationAction::Right),
    ] {
        sheet.add(
            format!("navigation-{name}"),
            ControlKind::Button,
            CanonicalControlIntent::Navigation { action },
        );
    }
    for (name, action) in [
        ("toggle", HighlightControlAction::Toggle),
        ("previous", HighlightControlAction::Previous),
        ("next", HighlightControlAction::Next),
        ("all", HighlightControlAction::All),
    ] {
        sheet.add(
            format!("highlight-{name}"),
            ControlKind::Button,
            CanonicalControlIntent::Highlight { action },
        );
    }
}

/// Lays out the playback wing: ninety-six slots on the current page, each with
/// three buttons and a motorised master fader, plus the five speed groups with
/// their tap button and level encoder.
fn add_playback_and_speed_groups(sheet: &mut ControlSheet) {
    for slot in 1..=96 {
        for (number, control) in [
            (1, PlaybackControl::ButtonOne),
            (2, PlaybackControl::ButtonTwo),
            (3, PlaybackControl::ButtonThree),
        ] {
            sheet.add(
                format!("page-playback-{slot}-button-{number}"),
                ControlKind::Button,
                CanonicalControlIntent::PlaybackCurrent { slot, control },
            );
        }
        sheet.add(
            format!("page-playback-{slot}-fader"),
            ControlKind::MotorFader,
            CanonicalControlIntent::PlaybackCurrent {
                slot,
                control: PlaybackControl::Master,
            },
        );
    }
    for (index, group) in ['A', 'B', 'C', 'D', 'E'].into_iter().enumerate() {
        let number = index + 1;
        sheet.add(
            format!("speed-group-{number}-button"),
            ControlKind::Button,
            CanonicalControlIntent::SpeedGroup {
                group,
                control: SpeedGroupControl::Tap,
            },
        );
        sheet.add(
            format!("speed-group-{number}-encoder"),
            ControlKind::AbsoluteEncoder,
            CanonicalControlIntent::SpeedGroup {
                group,
                control: SpeedGroupControl::Level,
            },
        );
    }
}

/// Builds the full simulated control surface so the desk treats the simulator
/// exactly like an attached hardware panel.
fn simulator_controls() -> (
    Vec<ControlDeclaration>,
    BTreeMap<String, CanonicalControlIntent>,
) {
    let mut sheet = ControlSheet {
        controls: Vec::new(),
        bindings: BTreeMap::new(),
    };
    add_programmer_keys(&mut sheet);
    add_encoders_and_navigation(&mut sheet);
    add_playback_and_speed_groups(&mut sheet);
    (sheet.controls, sheet.bindings)
}

fn run() -> Result<(), String> {
    let input = std::io::stdin();
    let mut output = std::io::stdout().lock();
    let mut input = WireReader::new(input);

    let host_hello = match input.next_message()? {
        Message::HostHello(hello) => hello,
        other => return Err(format!("expected host hello, received {other:?}")),
    };
    if !host_hello.supported_versions.contains(&DRAFT_PROTOCOL_V1) {
        return Err("host does not offer native extension protocol v1".into());
    }

    let token = required_environment("TOSKLIGHT_EXTENSION_CHANNEL_CREDENTIAL")?;
    let extension_hello = ExtensionHello {
        extension_id: required_environment("TOSKLIGHT_EXTENSION_ID")?,
        extension_instance_id: required_environment("TOSKLIGHT_EXTENSION_INSTANCE_ID")?,
        extension_version: env!("CARGO_PKG_VERSION").into(),
        package_digest: required_environment("TOSKLIGHT_EXTENSION_PACKAGE_DIGEST")?,
        selected_version: DRAFT_PROTOCOL_V1,
        capabilities: BTreeSet::from([ExtensionCapability::ControlSurface]),
        channel_response: light_extensions_host::channel_response(
            &token,
            &host_hello.channel_challenge,
        ),
    };
    send(&mut output, 0, Message::ExtensionHello(extension_hello))?;

    let configure = match input.next_message()? {
        Message::Configure(configure) => configure,
        other => return Err(format!("expected configure, received {other:?}")),
    };
    validate_configuration(&configure)?;
    let relay_address = relay_address(&configure)?;
    let listener = TcpListener::bind(relay_address)
        .map_err(|error| format!("cannot bind simulator relay {relay_address}: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("cannot configure simulator relay: {error}"))?;

    let initial_snapshot = configure.feedback.clone().expect("validated above");
    let (relay_inputs_tx, relay_inputs_rx) = mpsc::sync_channel(256);
    let relay = Relay::start(listener, relay_inputs_tx, &initial_snapshot);
    eprintln!("hardware simulator relay listening on {}", relay.address());

    send(
        &mut output,
        1,
        Message::Health(HealthReport {
            status: HealthStatus::Ready,
            detail: Some(format!("simulator relay listening on {}", relay.address())),
            counters: BTreeMap::new(),
        }),
    )?;

    let (host_tx, host_rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let result = input.next_message();
            let stopped = result.is_err();
            if host_tx.send(result).is_err() || stopped {
                break;
            }
        }
    });

    let mut frame_sequence = 2_u64;
    let mut input_id = 1_u64;
    loop {
        while let Ok(relay_input) = relay_inputs_rx.try_recv() {
            let event = ControlInputEvent {
                input_id,
                occurred_at_micros: relay_input
                    .occurred_at_micros
                    .unwrap_or_else(current_time_micros),
                control: relay_input.control,
            };
            if let Err(error) = validate_control_input(&event, &configure.control_bindings) {
                relay.broadcast_error(&error.to_string());
                continue;
            }
            send(&mut output, frame_sequence, Message::ControlInput(event))?;
            frame_sequence = frame_sequence
                .checked_add(1)
                .ok_or_else(|| "outbound frame sequence overflow".to_string())?;
            input_id = input_id
                .checked_add(1)
                .ok_or_else(|| "control input sequence overflow".to_string())?;
        }

        match host_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(Ok(Message::FeedbackSnapshot(snapshot))) => relay.broadcast_snapshot(&snapshot),
            Ok(Ok(Message::FeedbackDelta(delta))) => relay.broadcast_delta(&delta),
            Ok(Ok(Message::Shutdown(_))) => return Ok(()),
            Ok(Ok(Message::ProtocolError(error))) => {
                return Err(format!("host reported protocol error: {}", error.detail));
            }
            Ok(Ok(other)) => return Err(format!("unexpected host message: {other:?}")),
            Ok(Err(error)) => return Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("host channel reader stopped".into());
            }
        }
    }
}

fn validate_configuration(configure: &Configure) -> Result<(), String> {
    if !configure
        .enabled_capabilities
        .contains(&ExtensionCapability::ControlSurface)
    {
        return Err("configuration does not enable control_surface".into());
    }
    if configure.feedback.is_none() {
        return Err("configuration has no initial feedback snapshot".into());
    }
    Ok(())
}

fn relay_address(configure: &Configure) -> Result<SocketAddr, String> {
    let configured = std::env::var(RELAY_ADDRESS_ENV).ok().or_else(|| {
        configure
            .settings
            .get(RELAY_ADDRESS_SETTING)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    });
    let address = configured.as_deref().unwrap_or(DEFAULT_RELAY_ADDRESS);
    let address: SocketAddr = address
        .parse()
        .map_err(|error| format!("invalid simulator relay address `{address}`: {error}"))?;
    if !address.ip().is_loopback() {
        return Err(format!(
            "simulator relay address `{address}` is not a loopback address"
        ));
    }
    Ok(address)
}

fn current_time_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RelayInput {
    ControlInput {
        control: ControlInput,
        #[serde(default)]
        occurred_at_micros: Option<u64>,
    },
}

impl RelayInput {
    fn into_parts(self) -> RelayControlInput {
        match self {
            Self::ControlInput {
                control,
                occurred_at_micros,
            } => RelayControlInput {
                control,
                occurred_at_micros,
            },
        }
    }
}

struct RelayControlInput {
    control: ControlInput,
    occurred_at_micros: Option<u64>,
}

struct Relay {
    address: SocketAddr,
    clients: Arc<Mutex<Vec<TcpStream>>>,
    latest_snapshot: Arc<Mutex<String>>,
    stop: Arc<AtomicBool>,
}

impl Relay {
    fn start(
        listener: TcpListener,
        input_tx: mpsc::SyncSender<RelayControlInput>,
        initial_snapshot: &FeedbackSnapshot,
    ) -> Self {
        let address = listener
            .local_addr()
            .expect("bound listener always has a local address");
        let clients = Arc::new(Mutex::new(Vec::new()));
        let latest_snapshot = Arc::new(Mutex::new(relay_message(
            "feedback_snapshot",
            initial_snapshot,
        )));
        let stop = Arc::new(AtomicBool::new(false));
        let accept_clients = Arc::clone(&clients);
        let accept_snapshot = Arc::clone(&latest_snapshot);
        let accept_stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            accept_loop(
                listener,
                input_tx,
                accept_clients,
                accept_snapshot,
                accept_stop,
            );
        });
        Self {
            address,
            clients,
            latest_snapshot,
            stop,
        }
    }

    fn address(&self) -> SocketAddr {
        self.address
    }

    fn broadcast_snapshot(&self, snapshot: &FeedbackSnapshot) {
        let message = relay_message("feedback_snapshot", snapshot);
        *self.latest_snapshot.lock().expect("snapshot mutex") = message.clone();
        broadcast(&self.clients, &message);
    }

    fn broadcast_delta(&self, delta: &FeedbackDelta) {
        broadcast(&self.clients, &relay_message("feedback_delta", delta));
    }

    fn broadcast_error(&self, detail: &str) {
        broadcast(
            &self.clients,
            &serde_json::json!({ "type": "error", "detail": detail }).to_string(),
        );
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn accept_loop(
    listener: TcpListener,
    input_tx: mpsc::SyncSender<RelayControlInput>,
    clients: Arc<Mutex<Vec<TcpStream>>>,
    latest_snapshot: Arc<Mutex<String>>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if let Err(error) = stream.set_nonblocking(false) {
                    eprintln!("hardware simulator relay client setup failed: {error}");
                    continue;
                }
                let snapshot = latest_snapshot.lock().expect("snapshot mutex").clone();
                if write_lines(&mut stream, &[r#"{"type":"ready"}"#.to_string(), snapshot]).is_err()
                {
                    continue;
                }
                let Ok(reader) = stream.try_clone() else {
                    continue;
                };
                clients.lock().expect("relay clients mutex").push(stream);
                let client_tx = input_tx.clone();
                std::thread::spawn(move || read_relay_client(reader, client_tx));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                eprintln!("hardware simulator relay accept failed: {error}");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn read_relay_client(mut stream: TcpStream, input_tx: mpsc::SyncSender<RelayControlInput>) {
    let mut pending = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let count = match stream.read(&mut chunk) {
            Ok(0) => return,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        };
        pending.extend_from_slice(&chunk[..count]);
        if pending.len() > MAX_RELAY_LINE_BYTES && !pending.contains(&b'\n') {
            let _ = write_lines(
                &mut stream,
                &[serde_json::json!({
                    "type": "error",
                    "detail": format!("relay line exceeds {MAX_RELAY_LINE_BYTES} bytes")
                })
                .to_string()],
            );
            return;
        }
        while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = pending.drain(..=newline).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                continue;
            }
            if line.len() > MAX_RELAY_LINE_BYTES {
                return;
            }
            match serde_json::from_slice::<RelayInput>(&line) {
                Ok(input) => {
                    if input_tx.send(input.into_parts()).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = write_lines(
                        &mut stream,
                        &[serde_json::json!({
                            "type": "error",
                            "detail": format!("invalid control_input: {error}")
                        })
                        .to_string()],
                    );
                }
            }
        }
    }
}

fn relay_message<T: serde::Serialize>(kind: &str, body: &T) -> String {
    serde_json::json!({ "type": kind, "body": body }).to_string()
}

fn broadcast(clients: &Mutex<Vec<TcpStream>>, message: &str) {
    clients
        .lock()
        .expect("relay clients mutex")
        .retain_mut(|stream| write_lines(stream, &[message.to_owned()]).is_ok());
}

fn write_lines(stream: &mut TcpStream, lines: &[String]) -> std::io::Result<()> {
    for line in lines {
        stream.write_all(line.as_bytes())?;
        stream.write_all(b"\n")?;
    }
    stream.flush()
}

fn send(output: &mut impl Write, sequence: u64, message: Message) -> Result<(), String> {
    let bytes = encode_frame(&Frame::v1(sequence, message)).map_err(|error| error.to_string())?;
    output.write_all(&bytes).map_err(io_error)?;
    output.flush().map_err(io_error)
}

struct WireReader<R> {
    input: R,
    decoder: FrameDecoder,
    pending: VecDeque<Frame>,
}

impl<R: Read> WireReader<R> {
    fn new(input: R) -> Self {
        Self {
            input,
            decoder: FrameDecoder::new(0),
            pending: VecDeque::new(),
        }
    }

    fn next_message(&mut self) -> Result<Message, String> {
        let mut bytes = [0_u8; 8 * 1024];
        loop {
            if let Some(frame) = self.pending.pop_front() {
                return Ok(frame.message);
            }
            let count = self.input.read(&mut bytes).map_err(io_error)?;
            if count == 0 {
                return Err("host channel closed".into());
            }
            self.pending.extend(
                self.decoder
                    .push(&bytes[..count])
                    .map_err(|error| error.to_string())?,
            );
        }
    }
}

fn required_environment(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("missing {name}"))
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
