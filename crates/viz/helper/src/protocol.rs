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
// Externally tagged, which is serde's default: a compact binary format identifies a variant by
// index rather than by a name inside the message, and cannot carry an internal tag at all.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    /// Where in the window the helper may draw, in the logical points the web layout works in.
    ///
    /// A helper filling its own window never receives this. One drawing the desk's Stage pane
    /// receives it whenever the layout moves, and scissors itself to it — the surrounding chrome
    /// belongs to the webview, and a renderer that overshot would paint over the sheet.
    Pane { pane: crate::pane::PaneRect },
    /// Close the window and exit. The desk waits briefly, then kills.
    Shutdown,
}

/// What the helper sends back.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
///
/// A compact binary form rather than JSON. This is a private pipe between two processes of the
/// same build, so a self-describing text format buys nothing — and it cannot carry what the
/// renderer actually uses: an empty scene's bounds are infinities, which JSON writes as null and
/// then refuses to read back as a number.
pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, String> {
    postcard::to_allocvec(message).map_err(|error| error.to_string())
}

/// Decode a frame. A frame that will not decode is the other side being wrong, not this side, so
/// it is reported rather than panicked on.
pub fn decode<T: for<'a> Deserialize<'a>>(payload: &[u8]) -> Result<T, String> {
    postcard::from_bytes(payload).map_err(|error| error.to_string())
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
            ToHelper::Pane {
                pane: crate::pane::PaneRect {
                    x: 224.0,
                    y: 40.0,
                    width: 960.0,
                    height: 540.0,
                },
            },
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

#[cfg(test)]
mod value_plane {
    use super::*;

    /// The message the desk sends most often, carrying what the rig is actually doing.
    ///
    /// Values are the high-frequency traffic on this channel — a scene arrives when the rig
    /// changes, values arrive whenever the desk renders — so a round trip through the wire form
    /// is worth pinning rather than assuming.
    #[test]
    fn live_values_survive_the_channel() {
        let mut values = viz_scene::SceneValues::default();
        values.resize(2);
        values.emitters[0].intensity = 0.75;
        values.emitters[0].colour = [1.0, 0.5, 0.25];
        values.emitters[1].pan = 0.5;
        values.emitters[1].tilt = 0.25;
        values.atmosphere.density = 0.4;
        values.frame = 17;

        let carried = ToHelper::Values {
            payload: encode(&values).expect("values encode"),
        };
        let decoded: ToHelper = decode(&encode(&carried).expect("encodes")).expect("decodes");
        let ToHelper::Values { payload } = decoded else {
            panic!("the message changed shape crossing the channel");
        };
        let arrived: viz_scene::SceneValues = decode(&payload).expect("values decode");

        assert_eq!(arrived.emitters.len(), 2);
        assert_eq!(arrived.emitters[0].intensity, 0.75);
        assert_eq!(arrived.emitters[0].colour, [1.0, 0.5, 0.25]);
        assert_eq!(arrived.emitters[1].pan, 0.5);
        assert_eq!(arrived.emitters[1].tilt, 0.25);
        assert_eq!(arrived.atmosphere.density, 0.4);
        assert_eq!(arrived.frame, 17);
    }

    /// A laser's scan path is part of what the helper draws, and it is the one part of the value
    /// plane that is not a fixed-size number — so it gets its own check.
    #[test]
    fn a_laser_scan_path_survives_the_channel() {
        let mut values = viz_scene::SceneValues::default();
        values.resize(1);
        values.laser_scans[0].points_per_second = 30_000.0;
        values.laser_scans[0].slots = vec![1, 2, 3];

        let payload = encode(&values).expect("encodes");
        let arrived: viz_scene::SceneValues = decode(&payload).expect("decodes");
        assert_eq!(arrived.laser_scans[0].points_per_second, 30_000.0);
        assert_eq!(arrived.laser_scans[0].slots, vec![1, 2, 3]);
    }
}

#[cfg(test)]
mod scene_plane {
    use super::*;

    /// The rig itself crossing the channel.
    ///
    /// Sent when the show changes rather than every frame, but it is the message everything else
    /// is addressed against: values name emitters by position in this scene, so a scene that
    /// arrived wrong lights the wrong fixtures rather than failing visibly.
    #[test]
    fn a_scene_with_real_bounds_survives_the_channel() {
        let mut scene = viz_scene::Scene {
            revision: 9,
            show_name: "Demo Show".to_owned(),
            source_identity: "the desk".to_owned(),
            ..viz_scene::Scene::default()
        };
        // A rig with something in it has finite bounds. See below for why that matters.
        scene.bounds = viz_scene::Aabb {
            min: viz_scene::glam::Vec3::ZERO,
            max: viz_scene::glam::Vec3::splat(4.0),
        };

        let carried = ToHelper::Scene {
            payload: encode(&scene).expect("the scene encodes"),
        };
        let decoded: ToHelper = decode(&encode(&carried).expect("encodes")).expect("decodes");
        let ToHelper::Scene { payload } = decoded else {
            panic!("the message changed shape crossing the channel");
        };
        let arrived: viz_scene::Scene = decode(&payload).expect("decodes");

        assert_eq!(arrived.revision, 9);
        assert_eq!(arrived.show_name, "Demo Show");
        assert_eq!(arrived.source_identity, "the desk");
        assert!(arrived.fixtures.is_empty(), "an empty rig stays empty");
    }

    /// An empty scene is the case that decided the channel's format.
    ///
    /// An empty `Aabb` is infinities — that is how "nothing included yet" is represented, and it is
    /// correct. JSON has no infinity: it wrote them as null and then refused to read them back as
    /// numbers, so a desk that opened the visualizer before loading a show sent a scene the helper
    /// could not decode. A binary format carries the bits as they are.
    #[test]
    fn an_empty_scene_crosses_the_channel_infinities_and_all() {
        let scene = viz_scene::Scene::default();
        let arrived: viz_scene::Scene =
            decode(&encode(&scene).expect("encodes")).expect("an empty scene decodes");
        assert!(
            arrived.bounds.min.x.is_infinite(),
            "the empty bounds arrived as they were set"
        );
        assert!(arrived.fixtures.is_empty());
    }
}
