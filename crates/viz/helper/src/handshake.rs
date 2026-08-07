//! The first exchange on the helper channel.
//!
//! Both halves perform it before anything is drawn: the desk says what it speaks and what to open,
//! the helper answers with what it speaks and what is drawing. Either side refusing ends the
//! channel there, which is the point — a mismatched helper must not reach the frame loop, because
//! by then it would be a window showing something nobody can vouch for.

use crate::framing::{FramingError, read_frame, write_frame};
use crate::protocol::{
    FromHelper, Incompatible, PROTOCOL_MAJOR, PROTOCOL_MINOR, ToHelper, accepts, decode, encode,
};
use std::io::{Read, Write};

#[derive(Debug)]
pub enum HandshakeError {
    /// The channel failed before the exchange finished.
    Channel(FramingError),
    /// The other side spoke, but not a protocol this build can work with.
    Incompatible(Incompatible),
    /// A frame arrived that is not the message the exchange expects at this point.
    Unexpected(String),
}

impl std::fmt::Display for HandshakeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Channel(error) => write!(formatter, "{error}"),
            Self::Incompatible(reason) => write!(formatter, "{reason}"),
            Self::Unexpected(detail) => write!(
                formatter,
                "the visualizer helper did not answer as expected ({detail}); it may not be the \
                 helper this build ships"
            ),
        }
    }
}

impl std::error::Error for HandshakeError {}

impl From<FramingError> for HandshakeError {
    fn from(error: FramingError) -> Self {
        Self::Channel(error)
    }
}

/// What the desk learns about the helper it just started.
#[derive(Clone, Debug, PartialEq)]
pub struct HelperIdentity {
    pub protocol: (u16, u16),
    /// Adapter and backend, for the desk's diagnostics.
    pub renderer: String,
}

/// The desk's half: announce, then wait to be accepted.
pub fn greet_helper(
    to_helper: &mut impl Write,
    from_helper: &mut impl Read,
    title: &str,
) -> Result<HelperIdentity, HandshakeError> {
    let hello = ToHelper::Hello {
        protocol_major: PROTOCOL_MAJOR,
        protocol_minor: PROTOCOL_MINOR,
        title: title.to_owned(),
    };
    write_frame(
        to_helper,
        &encode(&hello).map_err(HandshakeError::Unexpected)?,
    )?;

    let frame = read_frame(from_helper)?;
    match decode::<FromHelper>(&frame).map_err(HandshakeError::Unexpected)? {
        FromHelper::Ready {
            protocol_major,
            protocol_minor,
            renderer,
        } => {
            accepts(protocol_major, protocol_minor).map_err(HandshakeError::Incompatible)?;
            Ok(HelperIdentity {
                protocol: (protocol_major, protocol_minor),
                renderer,
            })
        }
        // A helper that fails before it is ready says so rather than dying silently, and the desk
        // reports that reason instead of "the helper stopped".
        FromHelper::Error { detail } | FromHelper::Stopping { detail } => {
            Err(HandshakeError::Unexpected(detail))
        }
        // A frame before the greeting is a helper drawing for somebody else, or a stream this desk
        // has joined partway through. Either way it is not the answer this exchange is waiting for.
        FromHelper::Frame { .. } => Err(HandshakeError::Unexpected(
            "a rendered frame arrived before the helper said it was ready".to_owned(),
        )),
    }
}

/// The helper's half: wait to be greeted, check it, and answer.
///
/// Returns the window title the desk asked for, so the helper never has to know the product's
/// name or which of the two windows it is.
pub fn answer_desk(
    from_desk: &mut impl Read,
    to_desk: &mut impl Write,
    renderer: &str,
) -> Result<String, HandshakeError> {
    let frame = read_frame(from_desk)?;
    let title = match decode::<ToHelper>(&frame).map_err(HandshakeError::Unexpected)? {
        ToHelper::Hello {
            protocol_major,
            protocol_minor,
            title,
        } => {
            accepts(protocol_major, protocol_minor).map_err(HandshakeError::Incompatible)?;
            title
        }
        other => {
            return Err(HandshakeError::Unexpected(format!(
                "expected the opening message, got {other:?}"
            )));
        }
    };
    let ready = FromHelper::Ready {
        protocol_major: PROTOCOL_MAJOR,
        protocol_minor: PROTOCOL_MINOR,
        renderer: renderer.to_owned(),
    };
    write_frame(
        to_desk,
        &encode(&ready).map_err(HandshakeError::Unexpected)?,
    )?;
    Ok(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both halves against each other, which is the only way to know they agree.
    #[test]
    fn the_two_halves_complete_the_exchange() {
        let mut desk_to_helper = Vec::new();
        let mut helper_to_desk = Vec::new();

        // The desk speaks first, so its frame is written before the helper reads.
        let hello = encode(&ToHelper::Hello {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            title: "ToskLight Visualizer".to_owned(),
        })
        .expect("encodes");
        write_frame(&mut desk_to_helper, &hello).expect("writes");

        let title = answer_desk(
            &mut desk_to_helper.as_slice(),
            &mut helper_to_desk,
            "Apple M3 Pro (Metal)",
        )
        .expect("the helper answers");
        assert_eq!(title, "ToskLight Visualizer");

        // And the desk accepts what came back.
        let mut sent = Vec::new();
        let identity = greet_helper(
            &mut sent,
            &mut helper_to_desk.as_slice(),
            "ToskLight Visualizer",
        )
        .expect("the desk accepts");
        assert_eq!(identity.protocol, (PROTOCOL_MAJOR, PROTOCOL_MINOR));
        assert_eq!(identity.renderer, "Apple M3 Pro (Metal)");
    }

    #[test]
    fn a_helper_of_another_major_is_refused_by_the_desk() {
        let ready = encode(&FromHelper::Ready {
            protocol_major: PROTOCOL_MAJOR + 1,
            protocol_minor: 0,
            renderer: "whatever".to_owned(),
        })
        .expect("encodes");
        let mut answer = Vec::new();
        write_frame(&mut answer, &ready).expect("writes");

        let error =
            greet_helper(&mut Vec::new(), &mut answer.as_slice(), "title").expect_err("refused");
        assert!(matches!(error, HandshakeError::Incompatible(_)));
        assert!(error.to_string().contains("reinstall"), "{error}");
    }

    /// A helper that fails while starting says why, and the desk reports that rather than a bare
    /// "it stopped".
    #[test]
    fn a_helper_that_fails_before_it_is_ready_is_reported_with_its_reason() {
        let failure = encode(&FromHelper::Error {
            detail: "no GPU adapter".to_owned(),
        })
        .expect("encodes");
        let mut answer = Vec::new();
        write_frame(&mut answer, &failure).expect("writes");

        let error =
            greet_helper(&mut Vec::new(), &mut answer.as_slice(), "title").expect_err("refused");
        assert!(error.to_string().contains("no GPU adapter"), "{error}");
    }

    /// The helper refuses anything but the opening message: a desk that started mid-conversation
    /// is a desk whose state it cannot trust.
    #[test]
    fn the_helper_refuses_a_conversation_that_did_not_start_at_the_beginning() {
        let scene = encode(&ToHelper::Scene {
            payload: vec![1, 2, 3],
        })
        .expect("encodes");
        let mut channel = Vec::new();
        write_frame(&mut channel, &scene).expect("writes");

        let error =
            answer_desk(&mut channel.as_slice(), &mut Vec::new(), "renderer").expect_err("refused");
        assert!(matches!(error, HandshakeError::Unexpected(_)));
    }

    /// A helper that dies before saying anything is a closed channel, not a protocol fault, and
    /// the message should not blame the protocol for it.
    #[test]
    fn a_helper_that_never_speaks_is_a_closed_channel() {
        let empty: &[u8] = &[];
        let error = greet_helper(&mut Vec::new(), &mut { empty }, "title").expect_err("refused");
        assert!(matches!(
            error,
            HandshakeError::Channel(FramingError::Closed)
        ));
    }
}
