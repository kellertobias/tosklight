use super::*;
use std::sync::{
    Mutex,
    mpsc::{Receiver, Sender, channel},
};
use std::time::Duration;
use viz_helper::protocol::{FrameId, SharedSurfaceHandle};

struct Gated {
    started: Sender<()>,
    release: Receiver<()>,
}
impl Write for Gated {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let _ = self.started.send(());
        self.release
            .recv()
            .map_err(|_| std::io::ErrorKind::BrokenPipe)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn ready(sequence: u64) -> FromHelper {
    FromHelper::FrameReady {
        frame: FrameId {
            generation: 1,
            slot: 0,
            sequence,
        },
    }
}

#[test]
fn blocked_pipe_never_blocks_enqueue_or_drop_and_overflow_is_terminal() {
    let (started, entered) = channel();
    let (release, gate) = channel();
    let writer = Writer::with_limits(
        Gated {
            started,
            release: gate,
        },
        1,
        4096,
    )
    .unwrap();
    writer.send(ready(1));
    entered.recv_timeout(Duration::from_secs(1)).unwrap();
    writer.send(ready(2));
    writer.send(ready(3));
    assert!(writer.failure().unwrap().contains("cannot accept"));
    writer.send(ready(4));
    // Dropping does not join the worker, even while the owned output is blocked.
    let (dropped, observed) = channel();
    std::thread::spawn(move || {
        drop(writer);
        dropped.send(()).unwrap();
    });
    observed.recv_timeout(Duration::from_secs(1)).unwrap();
    drop(release);
}

#[test]
fn byte_budget_includes_in_progress_owned_rgba() {
    let (started, entered) = channel();
    let (release, gate) = channel();
    let cost = retained_bytes(&ready(1));
    let writer = Writer::with_limits(
        Gated {
            started,
            release: gate,
        },
        32,
        cost + 32,
    )
    .unwrap();
    writer.send(ready(1));
    entered.recv_timeout(Duration::from_secs(1)).unwrap();
    writer.send(FromHelper::Frame {
        frame: FrameId {
            generation: 1,
            slot: 0,
            sequence: 2,
        },
        width: 2,
        height: 1,
        rgba: vec![0; 32],
    });
    assert!(writer.failure().unwrap().contains("byte budget"));
    drop(release);
}

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn broken_pipe_latches_failure_without_another_send() {
    let writer = Writer::start(Broken).unwrap();
    writer.send(ready(1));
    for _ in 0..200 {
        if writer.failure().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(writer.failure().unwrap().contains("writer failed"));
}

struct Recorder(Arc<Mutex<Vec<u8>>>, Sender<()>);
impl Write for Recorder {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.1.send(()).unwrap();
        Ok(())
    }
}

#[test]
fn surface_frame_and_ready_keep_fifo_order_and_move_pixels() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let (flushed, observed) = channel();
    let writer = Writer::start(Recorder(bytes.clone(), flushed)).unwrap();
    let frame = FrameId {
        generation: 1,
        slot: 0,
        sequence: 1,
    };
    let messages = [
        FromHelper::Surface {
            generation: 1,
            slot: 0,
            handle: SharedSurfaceHandle::IoSurfaceId(7),
            width: 2,
            height: 1,
        },
        FromHelper::Frame {
            frame,
            width: 2,
            height: 1,
            rgba: vec![9; 8],
        },
        ready(1),
    ];
    let expected = messages.clone();
    for message in messages {
        writer.send(message);
    }
    for _ in 0..3 {
        observed.recv_timeout(Duration::from_secs(1)).unwrap();
    }
    let data = bytes.lock().unwrap();
    let mut reader = data.as_slice();
    for message in expected {
        let payload = viz_helper::framing::read_frame(&mut reader).unwrap();
        assert_eq!(
            viz_helper::protocol::decode::<FromHelper>(&payload).unwrap(),
            message
        );
    }
    assert!(reader.is_empty());
}
