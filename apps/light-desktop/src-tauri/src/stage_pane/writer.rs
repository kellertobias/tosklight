//! A bounded command queue keeps helper pipe writes off the Control thread.
use std::{
    io::Write,
    process::ChildStdin,
    sync::{
        Arc, OnceLock,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
};
use viz_helper::{
    framing,
    protocol::{ToHelper, encode},
};

const COMMAND_CAPACITY: usize = 128;

pub(super) struct PaneWriter {
    commands: SyncSender<ToHelper>,
    failure: Arc<OnceLock<String>>,
}

impl PaneWriter {
    pub(super) fn start(stdin: ChildStdin) -> Result<Self, String> {
        Self::with_writer(stdin, encode::<ToHelper>).map(|(writer, _worker)| writer)
    }

    pub(super) fn failure(&self) -> Option<&str> {
        self.failure.get().map(String::as_str)
    }

    /// Queued is not delivered: a later pipe failure is returned by subsequent calls and closes
    /// the helper's stdin. Overload fails explicitly; no command is silently coalesced or lost.
    pub(super) fn send(&self, message: ToHelper) -> Result<(), String> {
        if let Some(reason) = self.failure.get() {
            return Err(reason.clone());
        }
        self.commands.try_send(message).map_err(|error| match error {
            TrySendError::Full(_) => format!(
                "Stage renderer command queue is full ({COMMAND_CAPACITY} pending); restart the Stage instead of dropping an operator command"
            ),
            TrySendError::Disconnected(_) => self.failure.get().cloned().unwrap_or_else(||
                "Stage renderer command writer disconnected; restart the Stage".into()),
        })
    }

    fn with_writer<W, Encode>(
        writer: W,
        encode_message: Encode,
    ) -> Result<(Self, JoinHandle<()>), String>
    where
        W: Write + Send + 'static,
        Encode: Fn(&ToHelper) -> Result<Vec<u8>, String> + Send + 'static,
    {
        let (commands, messages) = mpsc::sync_channel(COMMAND_CAPACITY);
        let failure = Arc::new(OnceLock::new());
        let worker_failure = failure.clone();
        let worker = thread::Builder::new()
            .name("stage-pane-writer".into())
            .spawn(move || {
                if let Err(error) = write_commands(writer, messages, encode_message) {
                    let _ = worker_failure.set(error);
                }
            })
            .map_err(|error| format!("Cannot start Stage renderer command writer: {error}"))?;
        Ok((Self { commands, failure }, worker))
    }
}

fn write_commands<W, Encode>(
    mut writer: W,
    messages: Receiver<ToHelper>,
    encode_message: Encode,
) -> Result<(), String>
where
    W: Write,
    Encode: Fn(&ToHelper) -> Result<Vec<u8>, String>,
{
    for message in messages {
        let payload = encode_message(&message).map_err(|error| {
            format!("Stage renderer command encoding failed: {error}; restart the Stage")
        })?;
        framing::write_frame(&mut writer, &payload).map_err(|error| {
            format!("Stage renderer command pipe failed: {error}; restart the Stage")
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{self, Cursor},
        sync::Mutex,
        time::Duration,
    };
    use viz_helper::protocol::{PROTOCOL_MAJOR, PROTOCOL_MINOR, decode};

    #[derive(Clone)]
    struct Recording(Arc<Mutex<Vec<u8>>>);
    impl Write for Recording {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn hello() -> ToHelper {
        ToHelper::Hello {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            title: "Writer test".into(),
        }
    }

    #[test]
    fn preserves_order_and_framing_then_exits_when_sender_is_dropped() {
        let recording = Recording(Arc::new(Mutex::new(Vec::new())));
        let output = recording.clone();
        let (writer, worker) = PaneWriter::with_writer(recording, encode::<ToHelper>).unwrap();
        let first = hello();
        writer.send(first.clone()).unwrap();
        writer.send(ToHelper::Shutdown).unwrap();
        drop(writer);
        worker.join().unwrap();
        let bytes = output.0.lock().unwrap().clone();
        let mut reader = Cursor::new(bytes);
        assert_eq!(
            decode::<ToHelper>(&framing::read_frame(&mut reader).unwrap()).unwrap(),
            first
        );
        assert_eq!(
            decode::<ToHelper>(&framing::read_frame(&mut reader).unwrap()).unwrap(),
            ToHelper::Shutdown
        );
        assert!(matches!(
            framing::read_frame(&mut reader),
            Err(framing::FramingError::Closed)
        ));
    }

    #[test]
    fn encoding_failure_is_actionable_and_disconnects_the_queue() {
        let (writer, worker) =
            PaneWriter::with_writer(Vec::new(), |_| Err("injected encode failure".into())).unwrap();
        writer.send(hello()).unwrap();
        worker.join().unwrap();
        let error = writer.send(ToHelper::Shutdown).unwrap_err();
        assert!(error.contains("encoding failed") && error.contains("injected encode failure"));
    }

    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected pipe failure",
            ))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn pipe_failure_is_actionable_and_no_longer_accepts_commands() {
        let (writer, worker) = PaneWriter::with_writer(Broken, encode::<ToHelper>).unwrap();
        writer.send(hello()).unwrap();
        worker.join().unwrap();
        let error = writer.send(ToHelper::Shutdown).unwrap_err();
        assert!(error.contains("pipe failed") && error.contains("injected pipe failure"));
    }

    #[test]
    fn disconnected_writer_is_an_explicit_failure() {
        let (commands, messages) = mpsc::sync_channel(COMMAND_CAPACITY);
        drop(messages);
        let writer = PaneWriter {
            commands,
            failure: Arc::new(OnceLock::new()),
        };
        assert!(
            writer
                .send(ToHelper::Shutdown)
                .unwrap_err()
                .contains("disconnected")
        );
    }

    struct Gated {
        entered: Option<mpsc::Sender<()>>,
        release: Option<Receiver<()>>,
    }
    impl Write for Gated {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if let Some(entered) = self.entered.take() {
                entered.send(()).map_err(io::Error::other)?;
                self.release
                    .take()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(2))
                    .map_err(io::Error::other)?;
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn blocked_pipe_keeps_ui_send_nonblocking_and_overload_bounded() {
        let (entered, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (writer, worker) = PaneWriter::with_writer(
            Gated {
                entered: Some(entered),
                release: Some(gate),
            },
            encode::<ToHelper>,
        )
        .unwrap();
        writer.send(hello()).unwrap();
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        // The worker cannot consume another message until released. Exactly 128 queue entries fit.
        for _ in 0..COMMAND_CAPACITY {
            writer.send(ToHelper::Shutdown).unwrap();
        }
        let error = writer.send(ToHelper::Shutdown).unwrap_err();
        assert!(error.contains("queue is full (128 pending)"));
        release.send(()).unwrap();
        drop(writer);
        worker.join().unwrap();
    }
}
