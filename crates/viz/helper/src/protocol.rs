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
pub const PROTOCOL_MINOR: u16 = 1;

/// How a rendered pane gets from the helper to the desk.
///
/// Both processes are on one machine looking at one GPU, so the picture should never travel
/// through system memory — but "should" depends on the platform offering a surface two processes
/// can share, and on both sides agreeing on a texture format. Rather than assume, each side says
/// what it can do and the desk picks: the shared surface where it is available, the copy where it
/// is not, and neither when the helper cannot draw a pane at all — in which case the desk keeps
/// its own web renderer and nothing is embedded.
///
/// Ordered worst-first so `max()` picks the best transport both sides named.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum FrameTransport {
    /// RGBA8 through the pipe. Portable, and costs a GPU readback and a re-upload every frame.
    Copy,
    /// A surface both processes address on the GPU: `IOSurface` on macOS, a shared DXGI handle on
    /// Windows. The helper draws into it and the desk samples it; nothing is copied.
    Shared,
}

/// What this build can do on the platform it was compiled for.
///
/// Both halves call this — the helper to announce, the desk to choose — so a platform can never end
/// up with one side offering a transport the other was never built to speak.
///
/// `Copy` everywhere, and `Shared` on macOS, where a surface can be handed over as a mach port
/// right through a rendezvous the desk opens.
///
/// Windows is absent from the shared list: it has no shared path anybody here has run, and
/// announcing one would negotiate a desk into a transport that has never delivered a picture. The
/// copy is a working picture there, at the cost of a readback and a re-upload per frame.
///
/// This is what a build *can* do. Whether a particular desk may actually open a rendezvous is a
/// separate question it answers by trying, so a restricted process still negotiates the copy.
pub fn supported_transports() -> Vec<FrameTransport> {
    let mut transports = vec![FrameTransport::Copy];
    if cfg!(target_os = "macos") {
        transports.push(FrameTransport::Shared);
    }
    transports
}

/// The transport both sides can manage, or `None` when they share nothing.
///
/// `None` is a real answer rather than a failure: it is what tells the desk to keep drawing the
/// Stage with its own web renderer instead of embedding a pane it cannot receive.
pub fn agreed_transport(
    desk: &[FrameTransport],
    helper: &[FrameTransport],
) -> Option<FrameTransport> {
    desk.iter()
        .filter(|transport| helper.contains(transport))
        .max()
        .copied()
}

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
    /// Draw the desk's Stage pane instead of a window of the helper's own.
    ///
    /// Sent once, after the desk has read the helper's [`FromHelper::Capabilities`] and picked a
    /// transport both sides named. A helper that never receives this opens its own window, which
    /// is the desk-opened visualizer an operator asks for from the Tools menu.
    ///
    /// Appended after `Shutdown` rather than folded into `Hello`: the greeting is decoded before
    /// either side knows the other's version, so its shape is the one thing that cannot change.
    Embed {
        pane: crate::pane::PaneRect,
        /// Physical pixels per logical point, so the helper sizes its texture for the display the
        /// desk window is actually on rather than assuming one.
        scale: f32,
        transport: FrameTransport,
        /// The desk's own server, as `host:port`, and the user to join it as.
        ///
        /// The pane draws the rig the desk is running, and the rig is built from what that server
        /// serves — the same way the standalone visualizer builds one, with the same code. The
        /// alternative was to encode a scene in the desk and push it down this channel, which would
        /// mean a second implementation of scene building living in the window shell.
        ///
        /// Geometry, transport, input and picture settings still come over this channel: those are
        /// things only the desk knows. The rig is not one of them.
        desk: Option<DeskEndpoint>,
        /// Where to hand the desk a surface, for [`FrameTransport::Shared`].
        ///
        /// A surface cannot be named down this channel: what a second process can resolve is a
        /// port right, and a port name means nothing outside the task holding it. So the desk
        /// opens a channel a right *can* cross, and names it here. `None` for the copy transport,
        /// which needs no introduction because it carries the pixels themselves.
        surface_service: Option<String>,
    },
    /// The operator's picture settings for the pane.
    ///
    /// Sent as they are moved. These belong to the renderer rather than to the desk — the desk is
    /// not drawing this picture — so they cross rather than being applied locally.
    Picture {
        /// Haze the beams are drawn through, `0..=1`. A beam is only visible in something.
        atmosphere: f32,
        /// How brightly everything that is not a light source is lit, `0..=2`.
        ambient: f32,
        /// Draft, Standard, High or Ultra, as the renderer names them.
        quality: RenderQuality,
        /// Operator-safe exposure multiplier.
        exposure: f32,
        /// What every laser is drawn at, `1.0` being the built-in strength. Lasers have no honest
        /// reference — how strong a beam looks depends on the haze, the room and the eye — so it
        /// is the operator's, like the fog.
        laser_brightness: f32,
        /// Fixture numbers and patch addresses beside each fixture.
        show_labels: bool,
    },
    /// Pointer and camera intent picked up by the web layer over the pane.
    ///
    /// A `WKWebView` on top of a native surface wins AppKit hit-testing whatever CSS says, so pane
    /// input cannot fall through to the surface underneath. It is captured in the web layer and
    /// forwarded, already coalesced into a per-frame delta — one message per gesture step, never
    /// one per `pointermove`.
    Input { input: PaneInput },
}

/// How much the renderer is asked to do per frame.
///
/// Named here rather than taken from `viz-scene` so the channel does not depend on the renderer's
/// own types: this is a wire contract between two builds that must be able to differ.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RenderQuality {
    Draft,
    Standard,
    High,
    Ultra,
}

/// Where the desk serves the show the pane is to draw.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DeskEndpoint {
    pub host: String,
    pub port: u16,
    /// The user to join as, so the desk keeps one view per renderer rather than one for all.
    pub user: String,
    /// Which renderer this is, for a desk driving more than one.
    pub target: String,
}

/// What the operator did over the pane, as intent rather than as events.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum PaneInput {
    /// Rotate about the point being looked at, in logical points of drag.
    Orbit { dx: f32, dy: f32 },
    /// Slide the view: the camera and the point it looks at move together, so the picture
    /// translates without turning.
    Pan { dx: f32, dy: f32 },
    /// Move the camera itself across its own right and up axes, leaving what it looks at where it
    /// was — so the view turns as the camera walks.
    Truck { dx: f32, dy: f32 },
    /// Toward or away from the point being looked at, in wheel notches; positive moves in.
    Zoom { amount: f32 },
    /// Put the camera somewhere exactly, for a control surface that addresses it by number rather
    /// than by dragging — an encoder, above all.
    Place {
        x: Option<f32>,
        y: Option<f32>,
        z: Option<f32>,
        /// Degrees, absolute.
        pan: Option<f32>,
        tilt: Option<f32>,
        /// Distance from the camera to what it looks at, in metres.
        distance: Option<f32>,
    },
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
    /// One rendered frame of the Stage pane, as straight RGBA.
    ///
    /// **The fallback transport, not the intended one.** Both processes run on the same machine
    /// and share a GPU, so the picture should not travel through system memory at all: the desk
    /// should be handed a shared surface — `IOSurface` on macOS — and sample the helper's texture
    /// directly.
    ///
    /// The cost of this path is not the pipe. Copying a pane between two processes on one machine
    /// is around a hundredth of memory bandwidth, which is nothing. It is the round trip: reading
    /// a frame back off the GPU stalls the render pipeline, and the desk then uploads it straight
    /// back to the GPU for the webview to draw. Two transfers and a stall, every frame, to move an
    /// image between two processes that were already looking at the same device.
    ///
    /// It is kept because it is portable and needs no platform-specific texture import, so it
    /// covers whatever a shared surface cannot — and because a correct slow path is worth having
    /// while the fast one is written.
    ///
    /// The pane rectangle is sent rather than the whole window partly for this reason, and
    /// [`crate::framing::MAX_FRAME`] bounds what can cross.
    Frame {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    /// What this helper can do, sent once immediately after `Ready`.
    ///
    /// Separate from `Ready` because `Ready` is read by a desk that has not yet checked the
    /// version, so its shape has to stay fixed. A desk one minor behind never asks for this and
    /// never waits for it.
    Capabilities { transports: Vec<FrameTransport> },
    /// A surface the desk can sample, for [`FrameTransport::Shared`].
    ///
    /// Sent whenever the surface is created or replaced — which is every pane resize, since a
    /// shared surface is a fixed size. The desk drops the previous one when this arrives.
    Surface {
        handle: SharedSurfaceHandle,
        width: u32,
        height: u32,
    },
}

/// A GPU surface named in a way the other process can open.
///
/// Deliberately not a pointer. Both sides are separate processes, so what crosses is whatever the
/// platform lets one process name to another, and the variant says which platform's rules apply so
/// a mismatched pair refuses rather than dereferences a number from the wrong world.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SharedSurfaceHandle {
    /// A marker that a surface is waiting, not a name that resolves.
    ///
    /// `IOSurfaceLookup` by ID resolves only a surface created as global, which modern macOS
    /// ignores — measured on Darwin 25.5, where the desk looking up a surface the renderer had
    /// just created found nothing. The right itself travels out of band, as a mach port in a mach
    /// message over the rendezvous the desk named in [`ToHelper::Embed`]; this message only says
    /// that one has been sent and how big it is.
    IoSurfaceId(u32),
    /// A shared handle to a D3D11 texture, as `IDXGIResource1::CreateSharedHandle` returns.
    DxgiSharedHandle(u64),
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
            ToHelper::Embed {
                pane: crate::pane::PaneRect {
                    x: 0.0,
                    y: 0.0,
                    width: 640.0,
                    height: 360.0,
                },
                scale: 2.0,
                transport: FrameTransport::Shared,
                desk: Some(DeskEndpoint {
                    host: "127.0.0.1".to_owned(),
                    port: 5000,
                    user: "Operator".to_owned(),
                    target: "stage-pane".to_owned(),
                }),
                surface_service: Some("de.tokenet.tosklight.stage-pane.test".to_owned()),
            },
            ToHelper::Input {
                input: PaneInput::Orbit { dx: -3.5, dy: 0.25 },
            },
            ToHelper::Picture {
                atmosphere: 0.2,
                ambient: 0.75,
                quality: RenderQuality::High,
                exposure: 1.0,
                laser_brightness: 1.0,
                show_labels: false,
            },
        ];
        for message in messages {
            let encoded = encode(&message).expect("encodes");
            let decoded: ToHelper = decode(&encoded).expect("decodes");
            assert_eq!(decoded, message);
        }
    }

    /// Everything the helper answers with, including the two messages a shared surface needs.
    #[test]
    fn every_answer_survives_the_round_trip() {
        let answers = [
            FromHelper::Capabilities {
                transports: vec![FrameTransport::Copy, FrameTransport::Shared],
            },
            FromHelper::Surface {
                handle: SharedSurfaceHandle::IoSurfaceId(4_919),
                width: 1_920,
                height: 1_080,
            },
            FromHelper::Surface {
                handle: SharedSurfaceHandle::DxgiSharedHandle(0xdead_beef),
                width: 640,
                height: 360,
            },
        ];
        for answer in answers {
            let decoded: FromHelper = decode(&encode(&answer).expect("encodes")).expect("decodes");
            assert_eq!(decoded, answer);
        }
    }

    /// The transport is chosen, never assumed. Worst-first ordering is what makes `max` the pick.
    #[test]
    fn the_best_shared_transport_is_chosen_and_no_overlap_means_none() {
        assert_eq!(
            agreed_transport(
                &[FrameTransport::Copy, FrameTransport::Shared],
                &[FrameTransport::Copy, FrameTransport::Shared],
            ),
            Some(FrameTransport::Shared)
        );
        assert_eq!(
            agreed_transport(&[FrameTransport::Copy], &[FrameTransport::Shared]),
            None,
            "naming different transports is the same as naming none"
        );
        assert!(FrameTransport::Shared > FrameTransport::Copy);
    }

    /// A frame is the largest thing that crosses this channel, so its bound is worth stating.
    #[test]
    fn a_rendered_frame_crosses_the_channel() {
        let frame = FromHelper::Frame {
            width: 4,
            height: 2,
            rgba: vec![0xab; 4 * 2 * 4],
        };
        let decoded: FromHelper = decode(&encode(&frame).expect("encodes")).expect("decodes");
        assert_eq!(decoded, frame);
    }

    /// The pane is sent rather than the whole window precisely because of this: a frame is four
    /// bytes a pixel, and the framing refuses one that could not be a real message.
    #[test]
    fn a_frame_larger_than_the_channel_allows_is_refused() {
        let pixels = crate::framing::MAX_FRAME / 4 + 1_024;
        let frame = FromHelper::Frame {
            width: pixels as u32,
            height: 1,
            rgba: vec![0; pixels * 4],
        };
        let encoded = encode(&frame).expect("encodes");
        assert!(
            crate::framing::write_frame(&mut Vec::new(), &encoded).is_err(),
            "a frame this size is refused rather than sent"
        );
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
