//! Reading the desk's instructions after the handshake.
//!
//! The helper's side of the loop, kept apart from the window and the renderer so it can be driven
//! by a buffer in a test rather than only by a desk that started it. What arrives is decoded and
//! handed on; drawing it is the caller's business.

use crate::framing::{FramingError, read_frame};
use crate::protocol::{ToHelper, decode};
use std::io::Read;

/// What the channel produced.
#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    /// Something to act on.
    Message(ToHelper),
    /// The desk asked the helper to stop, or the channel ended. Either way the helper exits —
    /// there is nothing for it to be without a desk.
    Finished(String),
}

/// Read the next instruction.
///
/// A frame that will not decode does not end the channel: the desk may have sent something this
/// helper is too old to understand, and skipping it is better than a window disappearing over a
/// message that did not matter. A channel that has broken is a different thing and does end it.
pub fn next_instruction(from_desk: &mut impl Read) -> Instruction {
    loop {
        let frame = match read_frame(from_desk) {
            Ok(frame) => frame,
            Err(FramingError::Closed) => {
                // The desk went away. The helper's window belongs to it, so it goes too.
                return Instruction::Finished("the desk closed the channel".to_owned());
            }
            Err(error) => return Instruction::Finished(error.to_string()),
        };
        match decode::<ToHelper>(&frame) {
            Ok(ToHelper::Shutdown) => {
                return Instruction::Finished("the desk asked the visualizer to close".to_owned());
            }
            Ok(message) => return Instruction::Message(message),
            // Unknown or malformed: skipped deliberately. A helper one version behind meeting a
            // message it has never heard of should keep drawing, not vanish.
            Err(_) => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framing::write_frame;
    use crate::protocol::encode;

    fn channel(messages: &[ToHelper]) -> Vec<u8> {
        let mut buffer = Vec::new();
        for message in messages {
            write_frame(&mut buffer, &encode(message).expect("encodes")).expect("writes");
        }
        buffer
    }

    #[test]
    fn messages_arrive_in_order() {
        let buffer = channel(&[
            ToHelper::Scene { payload: vec![1] },
            ToHelper::Values { payload: vec![2] },
        ]);
        let mut reader = buffer.as_slice();
        assert_eq!(
            next_instruction(&mut reader),
            Instruction::Message(ToHelper::Scene { payload: vec![1] })
        );
        assert_eq!(
            next_instruction(&mut reader),
            Instruction::Message(ToHelper::Values { payload: vec![2] })
        );
    }

    #[test]
    fn a_shutdown_finishes_the_loop() {
        let buffer = channel(&[ToHelper::Shutdown]);
        match next_instruction(&mut buffer.as_slice()) {
            Instruction::Finished(reason) => assert!(reason.contains("close"), "{reason}"),
            other => panic!("expected the loop to finish, got {other:?}"),
        }
    }

    /// The desk going away takes the helper's window with it: a visualizer nobody is driving is a
    /// window nothing can close.
    #[test]
    fn a_closed_channel_finishes_the_loop() {
        let empty: &[u8] = &[];
        match next_instruction(&mut { empty }) {
            Instruction::Finished(reason) => assert!(reason.contains("desk"), "{reason}"),
            other => panic!("expected the loop to finish, got {other:?}"),
        }
    }

    /// A helper one version behind must keep drawing when it meets a message it does not know,
    /// rather than disappearing over something that may not have concerned it.
    #[test]
    fn a_message_this_helper_does_not_understand_is_skipped() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, br#"{"type":"something_newer"}"#).expect("writes");
        write_frame(
            &mut buffer,
            &encode(&ToHelper::Values { payload: vec![7] }).expect("encodes"),
        )
        .expect("writes");

        assert_eq!(
            next_instruction(&mut buffer.as_slice()),
            Instruction::Message(ToHelper::Values { payload: vec![7] }),
            "the unknown message was skipped and the next one arrived"
        );
    }

    /// Corruption is not an unknown message: the stream position is lost, so the channel ends.
    #[test]
    fn a_corrupt_length_finishes_the_loop() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&u32::MAX.to_be_bytes());
        match next_instruction(&mut buffer.as_slice()) {
            Instruction::Finished(reason) => assert!(reason.contains("corrupt"), "{reason}"),
            other => panic!("expected the loop to finish, got {other:?}"),
        }
    }
}
