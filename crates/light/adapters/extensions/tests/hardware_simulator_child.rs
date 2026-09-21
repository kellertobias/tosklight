use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use light_extensions_contract::{
    CanonicalControlIntent, Configure, ControlInput, DRAFT_PROTOCOL_V1, ExtensionCapability,
    FeedbackChange, FeedbackContext, FeedbackDelta, FeedbackSnapshot, FeedbackValue, Frame,
    HealthStatus, HostHello, Message, PlaybackControl, Shutdown, ShutdownReason, encode_frame,
};
use light_extensions_host::ExtensionsConfiguration;

fn child_program() -> &'static str {
    env!("CARGO_BIN_EXE_tl-hardware-simulator-extension")
}

fn send(child: &mut Child, sequence: u64, message: Message) {
    let bytes = encode_frame(&Frame::v1(sequence, message)).expect("encode host frame");
    let input = child.stdin.as_mut().expect("child stdin");
    input.write_all(&bytes).expect("write host frame");
    input.flush().expect("flush host frame");
}

fn read_frame(output: &mut impl Read) -> Result<Frame, String> {
    let mut length = [0_u8; 4];
    output
        .read_exact(&mut length)
        .map_err(|error| format!("read frame length: {error}"))?;
    let mut payload = vec![0; u32::from_be_bytes(length) as usize];
    output
        .read_exact(&mut payload)
        .map_err(|error| format!("read frame payload: {error}"))?;
    serde_json::from_slice(&payload).map_err(|error| format!("decode child frame: {error}"))
}

fn receive(child: &mut Child, frames: &Receiver<Result<Frame, String>>) -> Frame {
    match frames.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(frame)) => frame,
        Ok(Err(error)) => panic!("simulator child output failed: {error}"),
        Err(error) => {
            let _ = child.kill();
            panic!("timed out waiting for simulator child frame: {error}")
        }
    }
}

fn relay_line(reader: &mut BufReader<TcpStream>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read relay line");
    assert!(!line.is_empty(), "relay closed before a JSON line arrived");
    serde_json::from_str(&line).expect("decode relay line")
}

#[test]
fn native_handshake_relays_typed_controls_and_authoritative_feedback() {
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve relay address");
    let relay_address = reservation.local_addr().expect("reserved address");
    drop(reservation);

    let mut child = Command::new(child_program())
        .env("TOSKLIGHT_EXTENSION_ID", "test.hardware-simulator")
        .env("TOSKLIGHT_EXTENSION_INSTANCE_ID", "simulator-one")
        .env("TOSKLIGHT_EXTENSION_PACKAGE_DIGEST", "sha256:test")
        .env("TOSKLIGHT_EXTENSION_CHANNEL_CREDENTIAL", "secret")
        .env(
            "TOSKLIGHT_HARDWARE_SIMULATOR_RELAY_ADDR",
            relay_address.to_string(),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn simulator extension");
    let mut child_stdout = child.stdout.take().expect("child stdout");
    let (frames_tx, frames_rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let frame = read_frame(&mut child_stdout);
            let stopped = frame.is_err();
            if frames_tx.send(frame).is_err() || stopped {
                break;
            }
        }
    });

    send(
        &mut child,
        0,
        Message::HostHello(HostHello {
            host_name: "ToskLight test".into(),
            host_instance_id: "host-one".into(),
            supported_versions: vec![DRAFT_PROTOCOL_V1],
            requested_capabilities: BTreeSet::from([ExtensionCapability::ControlSurface]),
            channel_challenge: "challenge".into(),
        }),
    );
    let hello = receive(&mut child, &frames_rx);
    assert_eq!(hello.sequence, 0);
    let Message::ExtensionHello(hello) = hello.message else {
        panic!("expected extension hello")
    };
    assert_eq!(hello.extension_id, "test.hardware-simulator");
    assert_eq!(
        hello.channel_response,
        light_extensions_host::channel_response("secret", "challenge")
    );

    let context = FeedbackContext {
        desk_id: "desk-main".into(),
        show_id: Some("show-one".into()),
        show_generation: 1,
    };
    send(
        &mut child,
        1,
        Message::Configure(Configure {
            enabled_capabilities: BTreeSet::from([ExtensionCapability::ControlSurface]),
            feedback: Some(FeedbackSnapshot {
                context: context.clone(),
                revision: 7,
                controls: BTreeMap::from([("go".into(), FeedbackValue::Boolean(false))]),
            }),
            telemetry_channels: Vec::new(),
            device_actions: Vec::new(),
            control_bindings: BTreeMap::from([(
                "go".into(),
                CanonicalControlIntent::PlaybackCurrent {
                    slot: 1,
                    control: PlaybackControl::ButtonOne,
                },
            )]),
            settings: BTreeMap::new(),
        }),
    );
    let health = receive(&mut child, &frames_rx);
    let Message::Health(health) = health.message else {
        panic!("expected ready health")
    };
    assert_eq!(health.status, HealthStatus::Ready);

    let mut relay = TcpStream::connect(relay_address).expect("connect simulator relay");
    relay
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set relay timeout");
    let mut relay_reader = BufReader::new(relay.try_clone().expect("clone relay"));
    assert_eq!(relay_line(&mut relay_reader)["type"], "ready");
    let initial = relay_line(&mut relay_reader);
    assert_eq!(initial["type"], "feedback_snapshot");
    assert_eq!(initial["body"]["revision"], 7);

    relay
        .write_all(
            b"{\"type\":\"control_input\",\"control\":{\"kind\":\"button\",\"control_id\":\"go\",\"pressed\":true}}\n",
        )
        .expect("send typed relay control");
    relay.flush().expect("flush typed relay control");
    let control = receive(&mut child, &frames_rx);
    assert_eq!(control.sequence, 2);
    let Message::ControlInput(control) = control.message else {
        panic!("expected native control input")
    };
    assert_eq!(control.input_id, 1);
    assert_eq!(
        control.control,
        ControlInput::Button {
            control_id: "go".into(),
            pressed: true,
        }
    );

    send(
        &mut child,
        2,
        Message::FeedbackDelta(FeedbackDelta {
            context,
            base_revision: 7,
            revision: 8,
            changes: vec![FeedbackChange {
                control_id: "go".into(),
                value: Some(FeedbackValue::Boolean(true)),
            }],
        }),
    );
    let delta = relay_line(&mut relay_reader);
    assert_eq!(delta["type"], "feedback_delta");
    assert_eq!(delta["body"]["revision"], 8);

    send(
        &mut child,
        3,
        Message::Shutdown(Shutdown {
            reason: ShutdownReason::HostRequested,
            detail: None,
        }),
    );
    assert!(
        child
            .wait_timeout(Duration::from_secs(2))
            .expect("wait for child")
            .is_some(),
        "simulator child did not honor shutdown"
    );
}

#[test]
fn install_mode_packages_approves_and_enables_the_simulator() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root =
        PathBuf::from(".artifacts/tmp/hardware-simulator-install-tests").join(unique.to_string());
    let extensions = root.join("extensions");
    let configuration_path = root.join("extensions.json");

    let output = Command::new(child_program())
        .arg("--install")
        .arg(&extensions)
        .arg(&configuration_path)
        .output()
        .expect("run simulator installer");
    assert!(
        output.status.success(),
        "installer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let configuration = ExtensionsConfiguration::from_json(
        &std::fs::read(&configuration_path).expect("read installed configuration"),
    )
    .expect("valid installed configuration");
    let digest = configuration
        .approved_packages
        .get("de.tosklight.hardware-simulator")
        .expect("approved simulator package");
    assert_eq!(digest.len(), 64);
    let instance = configuration
        .instances
        .iter()
        .find(|instance| instance.id == "hardware-simulator")
        .expect("stable simulator instance");
    assert!(instance.enabled);
    assert_eq!(instance.desk_id.as_deref(), Some("main"));
    for id in [
        "programmer-record",
        "programmer-shift",
        "programmer-page-up",
        "encoder-1-turn",
        "encoder-6-press",
        "navigation-left",
        "highlight-toggle",
        "page-playback-96-button-1",
        "page-playback-20-fader",
        "speed-group-5-encoder",
    ] {
        assert!(
            instance.control_bindings.contains_key(id),
            "installer omitted {id}"
        );
    }
    assert!(
        extensions
            .join("de.tosklight.hardware-simulator/extension.json")
            .is_file()
    );
    let _ = std::fs::remove_dir_all(root);
}

trait ChildWaitTimeout {
    fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<std::process::ExitStatus>>;
}

impl ChildWaitTimeout for Child {
    fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<std::process::ExitStatus>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(Some(status));
            }
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
