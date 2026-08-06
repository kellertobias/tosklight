//! The private channel between the desk and its renderer helper.
//!
//! Framing only: how a message is delimited and how big it is allowed to be. What the messages
//! mean belongs to the layer above.
//!
//! The plan requires this channel to be bounded, and the reason is the isolation it exists to
//! provide. A helper is untrusted in the specific sense that it may be mid-failure — a driver
//! taking it down can leave a half-written frame or a length field of garbage on the pipe. If the
//! desk answers a four-gigabyte length by allocating four gigabytes, the helper's fault has become
//! the desk's, which is exactly what running it as a separate process was meant to prevent.
//!
//! So a length is checked before anything is reserved for it, and an impossible one ends the
//! channel rather than the desk.

use std::io::{Read, Write};

/// The largest frame either side will send or accept.
///
/// A scene snapshot for a large rig is the biggest thing that crosses here, and it is well inside
/// this. The number exists to be exceeded only by corruption, not by a real message.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub enum FramingError {
    /// The other side closed cleanly between frames.
    Closed,
    /// A length field no real message could have. The channel is not recoverable: the stream
    /// position is unknown, so the only safe thing is to end it.
    Oversized(usize),
    Io(std::io::Error),
}

impl std::fmt::Display for FramingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => formatter.write_str("the helper channel closed"),
            Self::Oversized(length) => write!(
                formatter,
                "the helper channel announced a {length}-byte frame, past the {MAX_FRAME}-byte \
                 limit; the stream is corrupt and cannot be resynchronised"
            ),
            Self::Io(error) => write!(formatter, "the helper channel: {error}"),
        }
    }
}

impl std::error::Error for FramingError {}

impl From<std::io::Error> for FramingError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Write one frame: a four-byte big-endian length, then the payload.
pub fn write_frame(writer: &mut impl Write, payload: &[u8]) -> Result<(), FramingError> {
    if payload.len() > MAX_FRAME {
        return Err(FramingError::Oversized(payload.len()));
    }
    writer.write_all(&(payload.len() as u32).to_be_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

/// Read one frame, refusing an impossible length before allocating for it.
pub fn read_frame(reader: &mut impl Read) -> Result<Vec<u8>, FramingError> {
    let mut header = [0_u8; 4];
    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(FramingError::Closed);
        }
        Err(error) => return Err(error.into()),
    }
    let length = u32::from_be_bytes(header) as usize;
    // Checked before the allocation, not after: the whole point is that a corrupt length cannot
    // make the desk reserve memory on the helper's behalf.
    if length > MAX_FRAME {
        return Err(FramingError::Oversized(length));
    }
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_survives_the_round_trip() {
        let mut channel = Vec::new();
        write_frame(&mut channel, b"a scene snapshot").expect("writes");
        let read = read_frame(&mut channel.as_slice()).expect("reads");
        assert_eq!(read, b"a scene snapshot");
    }

    #[test]
    fn frames_keep_their_boundaries() {
        let mut channel = Vec::new();
        write_frame(&mut channel, b"first").expect("writes");
        write_frame(&mut channel, b"second").expect("writes");
        let mut reader = channel.as_slice();
        assert_eq!(read_frame(&mut reader).expect("first"), b"first");
        assert_eq!(read_frame(&mut reader).expect("second"), b"second");
    }

    #[test]
    fn an_empty_frame_is_a_frame() {
        let mut channel = Vec::new();
        write_frame(&mut channel, b"").expect("writes");
        assert!(
            read_frame(&mut channel.as_slice())
                .expect("reads")
                .is_empty()
        );
    }

    #[test]
    fn a_clean_close_between_frames_is_not_an_error_condition() {
        let empty: &[u8] = &[];
        assert!(matches!(
            read_frame(&mut { empty }),
            Err(FramingError::Closed)
        ));
    }

    /// The isolation this exists for: a corrupt length must not make the desk allocate for it.
    #[test]
    fn an_impossible_length_is_refused_before_anything_is_reserved() {
        let mut channel = Vec::new();
        channel.extend_from_slice(&u32::MAX.to_be_bytes());
        // Deliberately no payload: a reader that trusted the length would wait for four gigabytes
        // that are never coming, or reserve them.
        match read_frame(&mut channel.as_slice()) {
            Err(FramingError::Oversized(length)) => {
                assert_eq!(length, u32::MAX as usize);
            }
            other => panic!("expected the length to be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_frame_past_the_limit_is_refused_rather_than_sent() {
        let mut channel = Vec::new();
        let oversized = vec![0_u8; MAX_FRAME + 1];
        assert!(matches!(
            write_frame(&mut channel, &oversized),
            Err(FramingError::Oversized(_))
        ));
        assert!(channel.is_empty(), "nothing was written");
    }

    /// A frame cut short is an error, not a short read: half a scene is not a scene.
    #[test]
    fn a_truncated_payload_is_an_error() {
        let mut channel = Vec::new();
        channel.extend_from_slice(&16_u32.to_be_bytes());
        channel.extend_from_slice(b"only four");
        assert!(matches!(
            read_frame(&mut channel.as_slice()),
            Err(FramingError::Io(_))
        ));
    }

    #[test]
    fn the_refusal_says_what_to_do_about_it() {
        let message = FramingError::Oversized(u32::MAX as usize).to_string();
        assert!(message.contains("corrupt"), "{message}");
    }
}
