//! Drawing what the desk sends, when this process is the desk's helper.
//!
//! The other providers go and fetch: they open a connection, read a show, bind receivers. This one
//! is told. The desk owns the window, decides what is in it, and pushes scene, values and view
//! down the channel — so there is nothing here to connect to, retry, or resynchronise with.
//!
//! The reading happens on its own thread because the channel is a blocking pipe and the render
//! loop must not wait on it. What crosses between them is already-decoded state, so a desk that
//! stops sending leaves the last picture up rather than blocking a frame.

use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use viz_helper::channel::{Instruction, next_instruction};
use viz_helper::handshake::answer_desk;
use viz_helper::protocol::{ToHelper, decode};
use viz_scene::{
    ConnectionState, ProviderCapabilities, ProviderDiagnostics, ProviderEvent, ProviderKind, Scene,
    SceneProvider, SceneValues, ViewConfiguration,
};

/// What the reading thread hands to the render loop.
enum FromChannel {
    Scene(Box<Scene>),
    Values(Box<SceneValues>),
    View(Box<ViewConfiguration>),
    /// Where in the window this helper may draw.
    Pane(viz_helper::pane::PaneRect),
    /// Draw the desk's Stage pane rather than a window of this process's own.
    Embed(Embedding),
    /// What the operator did over the pane, forwarded because the webview above it takes the
    /// events the surface would otherwise have received.
    Input(viz_helper::protocol::PaneInput),
    /// The channel ended, with the reason to show.
    Finished(String),
}

/// The desk's instruction to draw its Stage pane, and how to hand it back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Embedding {
    pub pane: viz_helper::pane::PaneRect,
    pub scale: f32,
    pub transport: viz_helper::protocol::FrameTransport,
}

pub struct HelperSource {
    inbox: Receiver<FromChannel>,
    /// The window title the desk asked for, once the handshake has produced it.
    title: Option<String>,
    connection: ConnectionState,
    /// Held so the host can show which side is driving, as it does for a desk.
    diagnostics: ProviderDiagnostics,
    /// Set once the desk has sent a scene: values addressed to a scene that never arrived would
    /// be values for a rig this process does not have.
    have_scene: bool,
    finished: bool,
    /// The way back to the desk, kept after the greeting so rendered panes can be returned.
    ///
    /// A helper filling its own window presents to it and never uses this. One drawing the desk's
    /// Stage pane has no window of its own to present to, so its frames go back up the channel.
    to_desk: Box<dyn Write + Send>,
    /// The rectangle the desk's layout says is the Stage pane, when this helper is drawing one.
    ///
    /// `None` means the helper owns its whole window, which is the desk-opened case today. The
    /// embedded pane sets it, and the render loop scissors to it — until then it is carried so
    /// the two sides are already agreed when the compositing lands.
    pane: Option<viz_helper::pane::PaneRect>,
    /// Set once the desk has asked for its Stage pane. `None` is the desk-opened visualizer, which
    /// owns its window and presents to it.
    embedding: Option<Embedding>,
    /// Forwarded pane input, coalesced into one step per gesture so the render loop applies a
    /// gesture rather than replaying a pointer track.
    input: Vec<viz_helper::protocol::PaneInput>,
}

impl HelperSource {
    /// Start reading the desk's channel, having first answered its greeting.
    ///
    /// `renderer` is what this process is drawing with, which the desk shows in its diagnostics —
    /// the operator asking why the picture is slow should not have to guess which GPU answered.
    pub fn start(
        mut from_desk: impl Read + Send + 'static,
        mut to_desk: impl Write + Send + 'static,
        renderer: String,
    ) -> Result<Self, String> {
        let title = answer_desk(
            &mut from_desk,
            &mut to_desk,
            &renderer,
            &viz_helper::protocol::supported_transports(),
        )
        .map_err(|error| error.to_string())?;
        let (outbox, inbox) = channel();
        std::thread::Builder::new()
            .name("viz-helper-channel".into())
            .spawn(move || read_channel(from_desk, &outbox))
            .map_err(|error| error.to_string())?;
        Ok(Self {
            inbox,
            title: Some(title),
            connection: ConnectionState::Idle,
            diagnostics: ProviderDiagnostics {
                endpoint: "the desk".to_owned(),
                interface: "the desk's channel".to_owned(),
                ..ProviderDiagnostics::default()
            },
            have_scene: false,
            finished: false,
            to_desk: Box::new(to_desk),
            pane: None,
            embedding: None,
            input: Vec::new(),
        })
    }

    /// Hand a rendered pane back to the desk.
    ///
    /// Failure is not reported upwards: the desk has gone, and a helper without a desk is already
    /// finishing for that reason. Saying so twice would not help anybody.
    pub fn send_frame(&mut self, width: u32, height: u32, rgba: Vec<u8>) {
        let message = viz_helper::protocol::FromHelper::Frame {
            width,
            height,
            rgba,
        };
        self.send(&message);
    }

    /// Failure is not reported upwards: the desk has gone, and a helper without a desk is already
    /// finishing for that reason.
    fn send(&mut self, message: &viz_helper::protocol::FromHelper) {
        if let Ok(payload) = viz_helper::protocol::encode(message) {
            let _ = viz_helper::framing::write_frame(&mut self.to_desk, &payload);
        }
    }

    /// Where this helper may draw, if the desk has said. `None` means the whole window.
    pub fn pane(&self) -> Option<viz_helper::pane::PaneRect> {
        self.pane
    }

    /// The desk's Stage pane, if this helper is drawing one rather than a window of its own.
    ///
    /// Carries the pane last sent, so a resize that arrived as a bare `Pane` message is reflected
    /// here too — the render loop reads one thing rather than reconciling two.
    pub fn embedding(&self) -> Option<Embedding> {
        self.embedding.map(|embedding| Embedding {
            pane: self.pane.unwrap_or(embedding.pane),
            ..embedding
        })
    }

    /// Announce a surface the desk can sample, replacing whatever it held before.
    pub fn send_surface(
        &mut self,
        handle: viz_helper::protocol::SharedSurfaceHandle,
        width: u32,
        height: u32,
    ) {
        self.send(&viz_helper::protocol::FromHelper::Surface {
            handle,
            width,
            height,
        });
    }

    /// Tell the desk about something the operator has to see, without stopping.
    pub fn report(&mut self, detail: &str) {
        self.send(&viz_helper::protocol::FromHelper::Error {
            detail: detail.to_owned(),
        });
    }

    /// Take the pane input that arrived since the last frame.
    pub fn take_input(&mut self) -> Vec<viz_helper::protocol::PaneInput> {
        std::mem::take(&mut self.input)
    }

    /// The window title the desk asked for, taken once.
    pub fn take_title(&mut self) -> Option<String> {
        self.title.take()
    }
}

/// The reading thread: decode what arrives and pass it on, until the channel ends.
fn read_channel(mut from_desk: impl Read, outbox: &Sender<FromChannel>) {
    loop {
        let message = match next_instruction(&mut from_desk) {
            Instruction::Message(message) => message,
            Instruction::Finished(reason) => {
                let _ = outbox.send(FromChannel::Finished(reason));
                return;
            }
        };
        // A payload that will not decode is reported rather than ignored: unlike an unknown
        // message, this one was addressed to this build and should have been readable.
        let sent = match message {
            ToHelper::Scene { payload } => match decode::<Scene>(&payload) {
                Ok(scene) => outbox.send(FromChannel::Scene(Box::new(scene))),
                Err(detail) => outbox.send(FromChannel::Finished(format!("scene: {detail}"))),
            },
            ToHelper::Values { payload } => match decode::<SceneValues>(&payload) {
                Ok(values) => outbox.send(FromChannel::Values(Box::new(values))),
                Err(detail) => outbox.send(FromChannel::Finished(format!("values: {detail}"))),
            },
            ToHelper::View { payload } => match decode::<ViewConfiguration>(&payload) {
                Ok(view) => outbox.send(FromChannel::View(Box::new(view))),
                Err(detail) => outbox.send(FromChannel::Finished(format!("view: {detail}"))),
            },
            ToHelper::Pane { pane } => outbox.send(FromChannel::Pane(pane)),
            ToHelper::Embed {
                pane,
                scale,
                transport,
            } => outbox.send(FromChannel::Embed(Embedding {
                pane,
                scale,
                transport,
            })),
            ToHelper::Input { input } => outbox.send(FromChannel::Input(input)),
            // Handled by the channel loop, which turns it into `Finished`.
            ToHelper::Hello { .. } | ToHelper::Shutdown => Ok(()),
        };
        if sent.is_err() {
            // The render loop has gone; there is nobody left to draw what this reads.
            return;
        }
    }
}

impl SceneProvider for HelperSource {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            kind: ProviderKind::LightingDesk,
            available: true,
            unavailable_reason: None,
            default_host: "the desk".into(),
            default_port: 0,
            // The desk decodes DMX and sends values; this process binds no sockets of its own.
            uses_network_input: false,
        }
    }

    fn poll(&mut self) -> Vec<ProviderEvent> {
        let mut events = Vec::new();
        loop {
            match self.inbox.try_recv() {
                Ok(FromChannel::Scene(scene)) => {
                    self.have_scene = true;
                    self.connection = ConnectionState::Connected {
                        endpoint: "the desk".to_owned(),
                        revision: scene.revision,
                    };
                    events.push(ProviderEvent::Connection(self.connection.clone()));
                    events.push(ProviderEvent::Snapshot { scene, view: None });
                }
                Ok(FromChannel::Values(values)) => {
                    // Values name emitters by position in the scene. Without one they would light
                    // whatever happened to be at those indices, which is worse than nothing.
                    if self.have_scene {
                        events.push(ProviderEvent::Values(values));
                    }
                }
                Ok(FromChannel::View(view)) => events.push(ProviderEvent::View(*view)),
                // Geometry, not scene content: the render loop reads it when it draws rather than
                // it being an event the host has to act on.
                Ok(FromChannel::Pane(pane)) => self.pane = Some(pane),
                Ok(FromChannel::Embed(embedding)) => {
                    self.pane = Some(embedding.pane);
                    self.embedding = Some(embedding);
                }
                Ok(FromChannel::Input(input)) => self.input.push(input),
                Ok(FromChannel::Finished(reason)) => {
                    self.finished = true;
                    self.connection = ConnectionState::Failed {
                        boundary: "the desk's channel".into(),
                        detail: reason,
                    };
                    events.push(ProviderEvent::Connection(self.connection.clone()));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self.finished {
                        self.finished = true;
                        events.push(ProviderEvent::Connection(ConnectionState::Failed {
                            boundary: "the desk's channel".into(),
                            detail: "the channel reader stopped".into(),
                        }));
                    }
                    break;
                }
            }
        }
        if !events.is_empty() {
            events.push(ProviderEvent::Diagnostics(Box::new(
                self.diagnostics.clone(),
            )));
        }
        events
    }

    /// Nothing to ask for. The desk sends what it decides to send, and a helper that demanded a
    /// resynchronisation would be a helper with an opinion about the show.
    fn request_resync(&mut self) {}

    fn shutdown(&mut self) {
        self.finished = true;
    }
}

impl HelperSource {
    /// Whether the desk has ended this helper's channel.
    ///
    /// True once the desk's channel has ended.
    ///
    /// The windowed application does not read this — a finished channel already reports itself as
    /// a failed connection, which is what the window shows. The embedded pane has no window to
    /// show anything in, so it asks directly.
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_helper::framing::write_frame;
    use viz_helper::protocol::{FromHelper, PROTOCOL_MAJOR, PROTOCOL_MINOR, encode};

    /// A channel carrying a greeting followed by whatever the test wants to send.
    /// A writer whose bytes a test can read back, standing in for the pipe to the desk.
    #[derive(Clone)]
    struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl Recorder {
        fn new() -> Self {
            Self(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())))
        }

        fn written(&self) -> Vec<u8> {
            self.0.lock().expect("writer").clone()
        }
    }

    impl std::io::Write for Recorder {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("writer").extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn desk_channel(messages: &[ToHelper]) -> Vec<u8> {
        let mut buffer = Vec::new();
        let hello = ToHelper::Hello {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            title: "ToskLight Visualizer".to_owned(),
        };
        write_frame(&mut buffer, &encode(&hello).expect("encodes")).expect("writes");
        for message in messages {
            write_frame(&mut buffer, &encode(message).expect("encodes")).expect("writes");
        }
        buffer
    }

    fn scene_message(revision: u64) -> ToHelper {
        let scene = Scene {
            revision,
            show_name: "Demo Show".to_owned(),
            ..Scene::default()
        };
        ToHelper::Scene {
            payload: encode(&scene).expect("encodes"),
        }
    }

    /// Drain the provider until it has produced something, since the reader runs on its own thread.
    fn drain(source: &mut HelperSource) -> Vec<ProviderEvent> {
        for _ in 0..200 {
            let events = source.poll();
            if !events.is_empty() {
                return events;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Vec::new()
    }

    #[test]
    fn the_greeting_yields_the_title_the_desk_asked_for() {
        let channel = desk_channel(&[]);
        let answer = Recorder::new();
        let mut source = HelperSource::start(
            std::io::Cursor::new(channel),
            answer.clone(),
            "test renderer".to_owned(),
        )
        .expect("the handshake completes");
        assert_eq!(source.take_title().as_deref(), Some("ToskLight Visualizer"));
        // And the desk was answered, so it knows what is drawing.
        let written = answer.written();
        let ready: FromHelper = {
            let mut reader = written.as_slice();
            let frame = viz_helper::framing::read_frame(&mut reader).expect("reads");
            viz_helper::protocol::decode(&frame).expect("decodes")
        };
        assert!(matches!(ready, FromHelper::Ready { .. }));
    }

    #[test]
    fn a_scene_from_the_desk_becomes_a_snapshot() {
        let channel = desk_channel(&[scene_message(4)]);
        let mut source =
            HelperSource::start(std::io::Cursor::new(channel), Vec::new(), "test".to_owned())
                .expect("the handshake completes");
        let events = drain(&mut source);
        assert!(
            events.iter().any(|event| matches!(
                event,
                ProviderEvent::Snapshot { scene, .. } if scene.revision == 4
            )),
            "the scene the desk sent became a snapshot"
        );
    }

    /// Values name emitters by position in a scene. Without one they would light whatever happened
    /// to be at those indices, which is worse than drawing nothing.
    #[test]
    fn values_before_a_scene_are_dropped() {
        let values = SceneValues::default();
        let channel = desk_channel(&[ToHelper::Values {
            payload: encode(&values).expect("encodes"),
        }]);
        let mut source =
            HelperSource::start(std::io::Cursor::new(channel), Vec::new(), "test".to_owned())
                .expect("the handshake completes");
        let events = drain(&mut source);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, ProviderEvent::Values(_))),
            "values arriving before a scene were dropped"
        );
    }

    /// The desk closing the channel ends the helper: a window nobody is driving cannot be closed
    /// from anywhere else.
    #[test]
    fn the_desk_going_away_finishes_the_source() {
        let channel = desk_channel(&[ToHelper::Shutdown]);
        let mut source =
            HelperSource::start(std::io::Cursor::new(channel), Vec::new(), "test".to_owned())
                .expect("the handshake completes");
        drain(&mut source);
        assert!(source.is_finished(), "the helper knows it is finished");
    }

    /// A helper owns its whole window until the desk says otherwise, which is the desk-opened
    /// case today. The embedded pane is what sends one.
    #[test]
    fn a_helper_draws_the_whole_window_until_it_is_given_a_pane() {
        let channel = desk_channel(&[]);
        let mut source =
            HelperSource::start(std::io::Cursor::new(channel), Vec::new(), "test".to_owned())
                .expect("the handshake completes");
        assert_eq!(source.pane(), None);
    }

    #[test]
    fn a_pane_from_the_desk_is_remembered_for_the_render_loop() {
        let pane = viz_helper::pane::PaneRect {
            x: 224.0,
            y: 40.0,
            width: 960.0,
            height: 540.0,
        };
        let channel = desk_channel(&[ToHelper::Pane { pane }]);
        let mut source =
            HelperSource::start(std::io::Cursor::new(channel), Vec::new(), "test".to_owned())
                .expect("the handshake completes");
        for _ in 0..200 {
            source.poll();
            if source.pane().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(source.pane(), Some(pane));
    }

    /// A pane the helper rendered goes back up the channel, because a helper drawing the desk's
    /// Stage has no window of its own to present to.
    #[test]
    fn a_rendered_pane_is_returned_to_the_desk() {
        use viz_helper::protocol::FromHelper;
        let channel = desk_channel(&[]);
        let returned = Recorder::new();
        let mut source = HelperSource::start(
            std::io::Cursor::new(channel),
            returned.clone(),
            "test".to_owned(),
        )
        .expect("the handshake completes");

        source.send_frame(2, 1, vec![9; 2 * 4]);

        // The greeting is first on the wire — the answer and then what this helper can do — and
        // the frame follows both.
        let written = returned.written();
        let mut reader = written.as_slice();
        let _ready = viz_helper::framing::read_frame(&mut reader).expect("the greeting answer");
        let capabilities =
            viz_helper::framing::read_frame(&mut reader).expect("what this helper can do");
        assert_eq!(
            viz_helper::protocol::decode::<FromHelper>(&capabilities).expect("decodes"),
            FromHelper::Capabilities {
                transports: viz_helper::protocol::supported_transports(),
            },
            "the desk cannot choose a transport it was never told about"
        );
        let frame = viz_helper::framing::read_frame(&mut reader).expect("the rendered pane");
        let decoded: FromHelper = viz_helper::protocol::decode(&frame).expect("decodes");
        assert_eq!(
            decoded,
            FromHelper::Frame {
                width: 2,
                height: 1,
                rgba: vec![9; 8]
            }
        );
    }

    /// A helper never asks the desk for anything: it draws what it is sent.
    #[test]
    fn a_helper_never_asks_for_a_resynchronisation() {
        let channel = desk_channel(&[]);
        let mut source =
            HelperSource::start(std::io::Cursor::new(channel), Vec::new(), "test".to_owned())
                .expect("the handshake completes");
        source.request_resync();
        assert!(
            !source.capabilities().uses_network_input,
            "the desk decodes DMX, not this"
        );
    }
}
