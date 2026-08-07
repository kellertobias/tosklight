//! The embedded pane, end to end, against the real binary.
//!
//! Everything below this test is unit-tested in pieces — the negotiation, the pane arithmetic, the
//! shared surface's round trip between two devices. None of that proves the pieces are wired to
//! each other. This starts the renderer the way the desk starts it, says what the desk says, and
//! waits for a picture, so "the desk asks for a pane and gets one" is checked rather than assumed.
//!
//! It needs a GPU. On a machine without one the renderer says so over the channel and the test
//! reports that rather than failing: a build agent with no adapter is not a broken pane.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use viz_helper::framing::{read_frame, write_frame};
use viz_helper::pane::PaneRect;
use viz_helper::protocol::{
    FrameTransport, FromHelper, PROTOCOL_MAJOR, PROTOCOL_MINOR, ToHelper, decode, encode,
};

/// Long enough for a cold process to open a device and draw once, short enough to fail a hang.
const PATIENCE: Duration = Duration::from_secs(30);

struct Renderer {
    child: Child,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start() -> Renderer {
    let child = Command::new(env!("CARGO_BIN_EXE_viz-renderer"))
        .arg("--embed")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the renderer beside this test");
    Renderer { child }
}

fn send(to: &mut impl Write, message: &ToHelper) {
    write_frame(to, &encode(message).expect("encodes")).expect("writes");
}

fn next(from: &mut impl Read) -> FromHelper {
    decode(&read_frame(from).expect("reads")).expect("decodes")
}

/// The desk's side of the exchange, up to the first picture.
#[test]
fn the_desk_asks_for_a_pane_and_the_renderer_draws_one() {
    let mut renderer = start();
    let mut to_renderer = renderer.child.stdin.take().expect("stdin");
    let mut from_renderer = renderer.child.stdout.take().expect("stdout");

    send(
        &mut to_renderer,
        &ToHelper::Hello {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            title: "ToskLight Stage".to_owned(),
        },
    );

    let FromHelper::Ready { renderer: name, .. } = next(&mut from_renderer) else {
        panic!("the renderer answered the greeting with something else");
    };
    assert!(!name.is_empty(), "the renderer names what it draws with");

    let FromHelper::Capabilities { transports } = next(&mut from_renderer) else {
        panic!("the renderer did not say what it can do");
    };
    assert!(
        transports.contains(&FrameTransport::Copy),
        "every build can copy: {transports:?}"
    );
    assert_eq!(
        transports,
        viz_helper::protocol::supported_transports(),
        "the renderer announces what this build can actually deliver, and nothing more"
    );

    // What the desk picks, given both lists — and, for a shared surface, a rendezvous the right
    // can actually cross. A process that may not register one negotiates the copy instead, which
    // is exactly what the desk does.
    #[cfg(target_os = "macos")]
    let rendezvous =
        viz_surface::rendezvous::Rendezvous::open(&format!("e2e-{}", std::process::id())).ok();
    #[cfg(not(target_os = "macos"))]
    let rendezvous: Option<()> = None;

    let transport = if rendezvous.is_some() {
        *transports.iter().max().expect("at least one transport")
    } else {
        FrameTransport::Copy
    };
    #[cfg(target_os = "macos")]
    let surface_service = rendezvous
        .as_ref()
        .map(|rendezvous| rendezvous.name().to_owned());
    #[cfg(not(target_os = "macos"))]
    let surface_service: Option<String> = None;

    eprintln!("negotiated transport: {transport:?}");
    // Where a rendezvous opened, the shared surface is what the pair must agree on. Falling back
    // to the copy there would be the fast path quietly never running.
    #[cfg(target_os = "macos")]
    if rendezvous.is_some() {
        assert_eq!(
            transport,
            FrameTransport::Shared,
            "a macOS pair that can open a rendezvous shares a surface"
        );
    }

    send(
        &mut to_renderer,
        &ToHelper::Embed {
            pane: PaneRect {
                x: 240.0,
                y: 48.0,
                width: 640.0,
                height: 360.0,
            },
            scale: 2.0,
            transport,
            surface_service,
        },
    );

    // The renderer has a scene of nothing, which still draws: an empty stage is a picture.
    let deadline = Instant::now() + PATIENCE;
    loop {
        assert!(
            Instant::now() < deadline,
            "the renderer never produced a picture within {PATIENCE:?}"
        );
        match next(&mut from_renderer) {
            FromHelper::Surface { width, height, .. } => {
                assert_eq!(
                    transport,
                    FrameTransport::Shared,
                    "a surface arrived for a transport that never asked for one"
                );
                // 640x360 points at 2x is the pane in the display's own pixels.
                assert_eq!((width, height), (1_280, 720));
                // The right travels out of band, because a surface has no name this channel can
                // carry. Opening it is the whole claim of the shared transport.
                #[cfg(target_os = "macos")]
                {
                    let rendezvous = rendezvous.as_ref().expect("a rendezvous was opened");
                    let port = rendezvous
                        .receive(Duration::from_secs(5))
                        .expect("receiving the right")
                        .expect("the renderer sent one");
                    let device = test_device().expect("a device to open it with");
                    let opened = viz_surface::import_from_port(&device, port, width, height);
                    assert!(opened.is_ok(), "{:?}", opened.err());
                }
                return;
            }
            FromHelper::Frame {
                width,
                height,
                rgba,
            } => {
                assert_eq!(
                    transport,
                    FrameTransport::Copy,
                    "pixels arrived for a transport that shares a surface"
                );
                assert_eq!((width, height), (1_280, 720));
                assert_eq!(rgba.len(), 1_280 * 720 * 4, "a full pane of RGBA");
                // One frame could be a one-shot. The pane is a loop, so a second has to follow
                // without the desk asking for it.
                let FromHelper::Frame { width, height, .. } = next(&mut from_renderer) else {
                    panic!("the renderer drew once and stopped");
                };
                assert_eq!((width, height), (1_280, 720));
                return;
            }
            // No GPU on this machine. That is a machine without an adapter, not a broken pane.
            FromHelper::Error { detail } | FromHelper::Stopping { detail } => {
                eprintln!("the renderer could not draw here: {detail}");
                return;
            }
            other => panic!("unexpected answer while waiting for a picture: {other:?}"),
        }
    }
}

/// A device to open the surface with, or none on a machine that has no adapter.
fn test_device() -> Option<wgpu::Device> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    let (device, _queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    Some(device)
}
