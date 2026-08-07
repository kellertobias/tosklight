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
    /// The channel ended, with the reason to show.
    Finished(String),
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
}

impl HelperSource {
    /// Start reading the desk's channel, having first answered its greeting.
    ///
    /// `renderer` is what this process is drawing with, which the desk shows in its diagnostics —
    /// the operator asking why the picture is slow should not have to guess which GPU answered.
    pub fn start(
        mut from_desk: impl Read + Send + 'static,
        // Only the reader crosses into the thread; the writer is used for the greeting and then
        // finished with, so it need not outlive this call.
        mut to_desk: impl Write,
        renderer: String,
    ) -> Result<Self, String> {
        let title = answer_desk(&mut from_desk, &mut to_desk, &renderer)
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
        })
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
    /// The application does not read this: a finished channel already reports itself as a failed
    /// connection, which is what the window shows and what the host acts on. It exists so a test
    /// can assert the state directly rather than inferring it from an event.
    #[cfg(test)]
    fn is_finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_helper::framing::write_frame;
    use viz_helper::protocol::{FromHelper, PROTOCOL_MAJOR, PROTOCOL_MINOR, encode};

    /// A channel carrying a greeting followed by whatever the test wants to send.
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
        let mut answer = Vec::new();
        let mut source = HelperSource::start(
            std::io::Cursor::new(channel),
            &mut answer,
            "test renderer".to_owned(),
        )
        .expect("the handshake completes");
        assert_eq!(source.take_title().as_deref(), Some("ToskLight Visualizer"));
        // And the desk was answered, so it knows what is drawing.
        let ready: FromHelper = {
            let mut reader = answer.as_slice();
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
