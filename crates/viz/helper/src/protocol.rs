//! What the desk and its renderer helper say to each other.
//!
//! Two processes shipped in one bundle, so they are always the same build — but only in theory. A
//! partially replaced application, a stale helper left by an interrupted update, or a developer
//! running one from a checkout and the other from a bundle all put mismatched halves on the same
//! pipe. So the first thing either side does is say which protocol it speaks, and a helper the
//! desk cannot talk to is refused rather than driven blindly.
//!
//! The rule is the one the API rules state: a major difference is refused, and one minor version
//! behind is accepted, because that is the case an interrupted update actually produces.

use serde::{Deserialize, Serialize};

/// The protocol this build speaks.
pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 0;

/// What the desk sends the helper.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToHelper {
    /// Always first. States the protocol and what to open.
    Hello {
        protocol_major: u16,
        protocol_minor: u16,
        /// The window title, so the helper does not have to know the product's name.
        title: String,
    },
    /// A complete scene, replacing whatever is displayed.
    Scene { payload: Vec<u8> },
    /// Live values for the scene already sent. Dropped if no scene has arrived: values addressed
    /// to a scene the helper does not have would be values for the wrong rig.
    Values { payload: Vec<u8> },
    /// The view — mode, camera, quality.
    View { payload: Vec<u8> },
    /// Close the window and exit. The desk waits briefly, then kills.
    Shutdown,
}

/// What the helper sends back.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FromHelper {
    /// The helper is up and speaks this protocol. Sent once, in answer to `Hello`.
    Ready {
        protocol_major: u16,
        protocol_minor: u16,
        /// What is drawing, for the desk's diagnostics: adapter and backend.
        renderer: String,
    },
    /// Something went wrong that the operator has to see. Not fatal on its own — the helper says
    /// so and keeps going where it can.
    Error { detail: String },
    /// The helper is stopping of its own accord.
    Stopping { detail: String },
}

/// Why a helper was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum Incompatible {
    /// A different major version. There is no compatibility across one, by definition.
    Major { theirs: u16, ours: u16 },
    /// Newer than this build. Its messages may mean things this side does not know.
    Ahead {
        theirs: (u16, u16),
        ours: (u16, u16),
    },
    /// More than one minor behind.
    TooOld {
        theirs: (u16, u16),
        ours: (u16, u16),
    },
}

impl std::fmt::Display for Incompatible {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let advice = "the visualizer helper does not match this build of ToskLight; reinstall so \
                      both halves come from the same version";
        match self {
            Self::Major { theirs, ours } => {
                write!(
                    formatter,
                    "helper protocol {theirs}.x, this desk speaks {ours}.x — {advice}"
                )
            }
            Self::Ahead { theirs, ours } | Self::TooOld { theirs, ours } => write!(
                formatter,
                "helper protocol {}.{}, this desk speaks {}.{} — {advice}",
                theirs.0, theirs.1, ours.0, ours.1
            ),
        }
    }
}

/// Whether this build can talk to a helper announcing `major.minor`.
///
/// One minor behind is accepted because that is what an interrupted update leaves behind, and a
/// helper one minor old still understands every message this side sends. Anything else is refused:
/// driving a renderer that may be misreading its instructions is worse than not opening it.
pub fn accepts(major: u16, minor: u16) -> Result<(), Incompatible> {
    accepts_between((PROTOCOL_MAJOR, PROTOCOL_MINOR), (major, minor))
}

/// The rule itself, with both sides named.
///
/// Stated separately from the constants so it can be checked at versions this build does not
/// happen to be at — at minor zero there is no "more than one behind" to reach, and a rule only
/// exercised at today's numbers is a rule nobody has tested.
pub fn accepts_between(ours: (u16, u16), theirs: (u16, u16)) -> Result<(), Incompatible> {
    if theirs.0 != ours.0 {
        return Err(Incompatible::Major {
            theirs: theirs.0,
            ours: ours.0,
        });
    }
    if theirs.1 > ours.1 {
        return Err(Incompatible::Ahead { theirs, ours });
    }
    if ours.1 - theirs.1 > 1 {
        return Err(Incompatible::TooOld { theirs, ours });
    }
    Ok(())
}

/// Encode a message for [`crate::framing::write_frame`].
pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(message).map_err(|error| error.to_string())
}

/// Decode a frame. A frame that will not decode is the other side being wrong, not this side, so
/// it is reported rather than panicked on.
pub fn decode<T: for<'a> Deserialize<'a>>(payload: &[u8]) -> Result<T, String> {
    serde_json::from_slice(payload).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_survives_the_round_trip() {
        let messages = [
            ToHelper::Hello {
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                title: "ToskLight Visualizer".to_owned(),
            },
            ToHelper::Scene {
                payload: vec![1, 2, 3],
            },
            ToHelper::Values {
                payload: Vec::new(),
            },
            ToHelper::View { payload: vec![9] },
            ToHelper::Shutdown,
        ];
        for message in messages {
            let encoded = encode(&message).expect("encodes");
            let decoded: ToHelper = decode(&encoded).expect("decodes");
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn the_helper_answers_are_carried_too() {
        let ready = FromHelper::Ready {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            renderer: "Apple M3 Pro (Metal)".to_owned(),
        };
        let decoded: FromHelper = decode(&encode(&ready).expect("encodes")).expect("decodes");
        assert_eq!(decoded, ready);
    }

    #[test]
    fn a_matching_helper_is_accepted() {
        assert_eq!(accepts(PROTOCOL_MAJOR, PROTOCOL_MINOR), Ok(()));
    }

    /// The case an interrupted update actually leaves behind.
    #[test]
    fn one_minor_behind_is_accepted() {
        assert_eq!(accepts_between((1, 4), (1, 3)), Ok(()));
    }

    #[test]
    fn another_major_is_refused() {
        assert!(matches!(
            accepts(PROTOCOL_MAJOR + 1, 0),
            Err(Incompatible::Major { .. })
        ));
    }

    /// A newer helper may mean things by its messages that this side does not know, so it is
    /// refused rather than driven on the assumption that newer is compatible.
    #[test]
    fn a_helper_ahead_of_this_build_is_refused() {
        assert!(matches!(
            accepts_between((1, 4), (1, 5)),
            Err(Incompatible::Ahead { .. })
        ));
    }

    #[test]
    fn a_helper_far_behind_is_refused() {
        assert!(matches!(
            accepts_between((1, 4), (1, 2)),
            Err(Incompatible::TooOld { .. })
        ));
    }

    /// The operator gets an instruction, not a version number on its own.
    #[test]
    fn a_refusal_says_what_to_do() {
        let refusal = accepts(PROTOCOL_MAJOR + 1, 0)
            .expect_err("refused")
            .to_string();
        assert!(refusal.contains("reinstall"), "{refusal}");
    }

    /// A frame that will not decode is the far side being wrong; this side reports it.
    #[test]
    fn a_frame_that_is_not_a_message_is_reported() {
        let error = decode::<ToHelper>(b"not json").expect_err("refused");
        assert!(!error.is_empty());
    }
}
