use std::io::{Read, Write};
use std::process::{ChildStderr, ChildStdin, ChildStdout};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};

use light_extensions_contract::{CodecError, Frame, FrameDecoder, Message, encode_frame};

#[derive(Debug)]
pub(crate) enum ReaderEvent {
    Frame(Frame),
    Protocol(String),
    Closed,
}

#[derive(Debug)]
pub(crate) enum WriterCommand {
    Message(Box<Message>),
    Close,
}

pub(crate) fn reader_loop(
    mut stdout: ChildStdout,
    events: SyncSender<ReaderEvent>,
    dropped: std::sync::Arc<AtomicU64>,
) {
    let mut decoder = FrameDecoder::new(0);
    let mut bytes = [0_u8; 8 * 1024];
    loop {
        let count = match stdout.read(&mut bytes) {
            Ok(0) => {
                let _ = events.try_send(ReaderEvent::Closed);
                return;
            }
            Ok(count) => count,
            Err(error) => {
                let _ = events.try_send(ReaderEvent::Protocol(format!(
                    "stdout read failed: {error}"
                )));
                return;
            }
        };
        match decoder.push(&bytes[..count]) {
            Ok(frames) => {
                for frame in frames {
                    if let Err(error) = events.try_send(ReaderEvent::Frame(frame)) {
                        match error {
                            TrySendError::Full(_) => {
                                dropped.fetch_add(1, Ordering::Relaxed);
                                // Control input can be edge-sensitive. Once a complete frame did
                                // not fit, continuing with later sequence numbers would silently
                                // guess state. End the session so reconnect begins from a full
                                // authoritative snapshot.
                                return;
                            }
                            TrySendError::Disconnected(_) => return,
                        }
                    }
                }
            }
            Err(error) => {
                let _ = events.try_send(ReaderEvent::Protocol(codec_detail(error)));
                return;
            }
        }
    }
}

fn codec_detail(error: CodecError) -> String {
    error.to_string()
}

pub(crate) fn writer_loop(mut stdin: ChildStdin, commands: Receiver<WriterCommand>) {
    let mut sequence = 0_u64;
    while let Ok(command) = commands.recv() {
        match command {
            WriterCommand::Message(message) => {
                let frame = Frame::v1(sequence, *message);
                let Ok(bytes) = encode_frame(&frame) else {
                    return;
                };
                if stdin.write_all(&bytes).is_err() || stdin.flush().is_err() {
                    return;
                }
                let Some(next) = sequence.checked_add(1) else {
                    return;
                };
                sequence = next;
            }
            WriterCommand::Close => return,
        }
    }
}

pub(crate) fn stderr_loop(
    mut stderr: ChildStderr,
    lines: SyncSender<String>,
    dropped: std::sync::Arc<AtomicU64>,
    maximum_line_bytes: usize,
) {
    use std::collections::VecDeque;

    let maximum_line_bytes = maximum_line_bytes.max(1);
    let mut current = VecDeque::with_capacity(maximum_line_bytes.min(8 * 1024));
    let mut truncated = false;
    let mut bytes = [0_u8; 4 * 1024];
    loop {
        let count = match stderr.read(&mut bytes) {
            Ok(0) => {
                if !current.is_empty() {
                    send_log_line(&lines, &dropped, &mut current, truncated);
                }
                return;
            }
            Ok(count) => count,
            Err(_) => return,
        };
        for byte in &bytes[..count] {
            if *byte == b'\n' {
                send_log_line(&lines, &dropped, &mut current, truncated);
                truncated = false;
                continue;
            }
            if current.len() == maximum_line_bytes {
                current.pop_front();
                truncated = true;
            }
            current.push_back(*byte);
        }
    }
}

fn send_log_line(
    lines: &SyncSender<String>,
    dropped: &AtomicU64,
    bytes: &mut std::collections::VecDeque<u8>,
    truncated: bool,
) {
    let contiguous = bytes.make_contiguous();
    let line = String::from_utf8_lossy(contiguous).into_owned();
    bytes.clear();
    if truncated {
        dropped.fetch_add(1, Ordering::Relaxed);
    }
    if let Err(error) = lines.try_send(line) {
        match error {
            TrySendError::Full(_) => {
                dropped.fetch_add(1, Ordering::Relaxed);
            }
            TrySendError::Disconnected(_) => {}
        }
    }
}
