//! The desk's own output, for a renderer drawing the Stage inside the desk's window.
//!
//! Universes are decoded through exactly the path a real packet takes, and the desk's word on
//! where its 3D Points are is laid over the result, so a point with no address still moves what
//! is hung on it.
//!
//! Before any of that, a stamped snapshot is admitted by its frame identity: within one
//! connection epoch an older Live frame never replaces a newer picture, a snapshot whose outer
//! and native Live identities disagree is not one picture at all, and the very frame already
//! applied is not decoded again. A followed Preload lane stamped by the desk's accepted Pending
//! publication is admitted the same way by its own episode and ticket
//! ([`preload::PreloadFrameAdmission`]). This is frame identity admission only;
//! requested/achieved quality consumers are not part of it.

use super::{DeskProvider, ProviderEvent, SceneValues};
use crate::desk_output_frame::{desk_output_signature, stamp_desk_output_frame};
use light_wire::v2::output_control::{OutputDmxSnapshot, OutputFrameIdentity};

impl DeskProvider {
    /// Forget every proof about the desk's output frames, and the snapshot waiting to be applied.
    ///
    /// Frame identities are process-local to the server that stamped them: a reconnect may reach
    /// a restarted server whose counters began again, and a new scene may be a different show.
    /// Neither is ordered against what came before, so acceptance starts over instead. The next
    /// snapshot is presented even when it says what the last presented one said, because the
    /// values it is decoded into were just rebuilt.
    ///
    /// Every connection message bounds an epoch of the worker's connection: no output read on
    /// one side of it is ordered against one read on the other. A new scene is a new source:
    /// whatever was proven about the last one's frames, and any snapshot still waiting from it,
    /// belongs to something that is no longer displayed.
    pub(super) fn reset_desk_output_acceptance(&mut self) {
        self.desk_output = None;
        self.presented_desk_output = None;
        self.desk_output_admission.reset();
    }

    /// Hold an arriving desk-output snapshot for the next poll.
    pub(super) fn queue_desk_output(&mut self, value: OutputDmxSnapshot) {
        // One pending snapshot, never a queue: a stamped one is not displaced by an
        // older or contradictory arrival in the same drain.
        if !self
            .desk_output
            .as_ref()
            .is_some_and(|pending| keeps_pending(pending, &value))
        {
            self.desk_output = Some(value);
        }
    }

    /// Fold the desk's own output into the values, and present it when it changed the picture.
    pub(super) fn apply_desk_output(&mut self, events: &mut Vec<ProviderEvent>) {
        // The desk's own output, for a renderer drawing inside the desk's window. Decoded through
        // exactly the path a real packet takes, so nothing downstream can tell the difference —
        // the numbers are the same numbers, read from the desk instead of heard from the wire.
        // Applied on every read rather than when a revision moves. The desk's output revision
        // counts structural changes, not frames: it sits still while every level in the show is
        // moving, so gating on it showed the rig as it was at the moment the pane opened and never
        // again.
        if let (Some(output), Some(decoder), Some(scene)) =
            (self.desk_output.take(), &mut self.decoder, &self.scene)
        {
            // Admit the frame by its identity before anything is decoded or presented. A
            // rejected frame is dropped passively: the picture already presented stays.
            let admission = self.desk_output_admission.classify(&output);
            if !self.desk_output_admission.admits(admission) {
                return;
            }
            // A followed Preload lane is one picture with its own Pending identity: a replayed
            // or incoherent Pending frame drops the snapshot passively, like an older Live one.
            let preload = self
                .following_preload
                .then(|| self.desk_output_admission.preload.classify(&output));
            if let Some(verdict) = preload
                && !self.desk_output_admission.preload.admits(verdict)
            {
                return;
            }
            // Reject the entire source before decoding any universe or point. A current server
            // may temporarily have no matching source while the patch/show is being replaced.
            if output.native_protocol > 0
                && !output.native.as_ref().is_some_and(|lane| {
                    lane.show_id == scene.show_id
                        && lane.revision >= scene.source_show_revision
                        && decoder.accepts_native_snapshot(
                            lane.instances
                                .iter()
                                .map(|i| (i.instance_id, i.native_identity.as_str())),
                        )
                })
            {
                return;
            }
            // What this read would put on screen. Nothing below changes any of its inputs, so it
            // is known before decoding: the very frame already applied, saying what the presented
            // picture already says, is not decoded or presented again.
            let signature = desk_output_signature(
                &output,
                (self.following_preload && output.native_protocol == 0)
                    .then_some(&self.preload_projection),
                self.following_preload,
                scene.revision,
            );
            if admission == Admission::Same && self.presented_desk_output == Some(signature) {
                self.desk_output_admission.held += 1;
                if let Some(verdict) = preload {
                    self.desk_output_admission.preload.applied(verdict, &output);
                }
                return;
            }
            let time_seconds = self.epoch.elapsed().as_secs_f32();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_micros() as u64)
                .unwrap_or_default();
            let frames = universe_frames(&output, now);
            if !frames.is_empty() {
                decoder.apply(scene, &frames, &mut self.values, time_seconds);
            }
            let valid_lane = |lane: &light_wire::v2::output_control::OutputNativeLane| {
                lane.show_id == scene.show_id && lane.revision >= scene.source_show_revision
            };
            for (lane, preload) in [
                (output.native.as_ref(), false),
                (
                    self.following_preload
                        .then_some(output.preload.as_ref())
                        .flatten(),
                    true,
                ),
            ] {
                if let Some(lane) = lane.filter(|lane| valid_lane(lane)) {
                    decoder.apply_native(
                        scene,
                        lane.instances
                            .iter()
                            .map(|i| viz_project::NativeInstanceValues {
                                fixture_id: i.fixture_id,
                                instance_id: i.instance_id,
                                native_identity: &i.native_identity,
                                owned_channels: i.owned_channels.as_deref(),
                                raw: &i.raw,
                            }),
                        preload,
                        &mut self.values,
                        time_seconds,
                    );
                }
            }
            // The desk's own word on where its 3D Points are. A point patched to a universe was
            // also just decoded from its slots; the resolved pose the desk states is the same
            // number without the quantisation, and a point with no address has only this.
            let points = if self.following_preload {
                output
                    .preload
                    .as_ref()
                    .filter(|lane| valid_lane(lane))
                    .map(|lane| lane.points.as_slice())
            } else {
                None
            }
            .unwrap_or(&output.points);
            apply_point_poses(scene, points, &mut self.values);
            // The preload sits on top of the live picture rather than replacing it: a fixture
            // nobody preloaded goes on showing what it is doing. This also applies when the desk
            // has no patched universes; unpatched fixtures are still part of the show.
            if self.following_preload && output.native_protocol == 0 {
                let overlay: Vec<crate::preload_overlay::PreloadValue> = self
                    .preload_projection
                    .fixture_values
                    .iter()
                    .filter_map(|entry| match entry.value {
                        crate::wire::PreloadAttributeValue::Normalized(value) => {
                            Some(crate::preload_overlay::PreloadValue {
                                fixture_id: entry.fixture_id,
                                attribute: entry.attribute.clone(),
                                value,
                            })
                        }
                        crate::wire::PreloadAttributeValue::Other => None,
                    })
                    .collect();
                crate::preload_overlay::apply(scene, &overlay, &mut self.values);
            }
            // An empty output snapshot is still an authoritative source frame. A show may retain
            // a complete unpatched rig, and the Stage must keep presenting it instead of treating
            // the absence of network universes as the absence of the desk. But a snapshot that
            // says what the last one said is not a new frame: the pane holds its picture.
            self.desk_output_admission.applied(admission, &output);
            if let Some(verdict) = preload {
                self.desk_output_admission.preload.applied(verdict, &output);
            }
            if self.presented_desk_output != Some(signature) {
                self.presented_desk_output = Some(signature);
                stamp_desk_output_frame(&mut self.values, &mut self.value_frame, now);
                events.push(ProviderEvent::Values(Box::new(self.values.clone())));
            }
        }
    }
}

/// The snapshot's universes as decoder frames, each padded or clipped to 512 slots.
fn universe_frames(output: &OutputDmxSnapshot, now: u64) -> Vec<viz_dmx::UniverseFrame> {
    output
        .universes
        .iter()
        .map(|universe| {
            let mut slots = [0_u8; 512];
            let length = universe.slots.len().min(512);
            slots[..length].copy_from_slice(&universe.slots[..length]);
            viz_dmx::UniverseFrame {
                logical_universe: universe.universe,
                slots,
                received_micros: now,
                stale: false,
            }
        })
        .collect()
}

/// What the provider has proven about the desk's stamped Live output frames, within one
/// connection epoch.
///
/// A stamped Live frame carries the server's publication identity: a process-local `sequence`
/// the server's capacity-one publication hub hands out in publication order, the runtime
/// `generation` it was resolved under, and the instant it was sampled. Inside one connection to
/// one server, a lower `sequence` is an older picture. Across a reconnect — which may be a
/// restarted server whose counters began again — the numbers mean nothing to each other, so the
/// provider forgets this state at every connection and scene boundary rather than comparing.
///
/// Only the Live identity is read here. The Preload lane is an independent evaluation with its
/// own Pending stamp, admitted separately by [`preload::PreloadFrameAdmission`]; it is never
/// compared with Live and never given Live's.
///
/// Rejection is passive: the frame is dropped, the presented picture stays as it is, nothing is
/// queued and nobody is told. The desk serves its output again on the next read.
#[derive(Debug, Default)]
pub(super) struct DeskOutputAdmission {
    /// The newest stamped Live identity applied in this epoch.
    newest: Option<OutputFrameIdentity>,
    /// Snapshots decoded into the values, stamped or not.
    pub(super) decoded: u64,
    /// Stamped duplicates of the applied frame that would not have changed the picture.
    pub(super) held: u64,
    /// Older stamped Live frames from this epoch.
    pub(super) stale: u64,
    /// Snapshots whose identities contradict each other or the applied frame.
    pub(super) incoherent: u64,
    /// The followed Preload lane's own Pending admission.
    pub(super) preload: preload::PreloadFrameAdmission,
}

/// Where one snapshot's Live identity stands against what this epoch has already applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Admission {
    /// No identity at all: an older server, or one with nothing published yet. Taken as it
    /// always was, latest-wins, and proves nothing.
    Unstamped,
    /// Newer than anything applied in this epoch, or the first stamp of the epoch.
    Newer,
    /// The very frame already applied.
    Same,
    /// Older than the frame already applied.
    Older,
    /// Outer and native identities disagree, or one sequence names two different frames.
    Incoherent,
}

/// The identity a snapshot claims for its Live picture.
///
/// The outer stamp covers the universes and Points; the native Live lane carries its own. Where
/// both are supplied they must name the same frame, or the snapshot is not one picture. Where
/// only one is supplied it speaks for the whole Live picture. The Preload lane is not consulted.
pub(super) fn live_identity(
    output: &OutputDmxSnapshot,
) -> Result<Option<&OutputFrameIdentity>, ()> {
    match (
        output.frame.as_ref(),
        output.native.as_ref().and_then(|lane| lane.frame.as_ref()),
    ) {
        (Some(outer), Some(native)) if outer != native => Err(()),
        (Some(identity), _) | (None, Some(identity)) => Ok(Some(identity)),
        (None, None) => Ok(None),
    }
}

impl DeskOutputAdmission {
    pub(super) fn classify(&self, output: &OutputDmxSnapshot) -> Admission {
        let identity = match live_identity(output) {
            Err(()) => return Admission::Incoherent,
            Ok(None) => return Admission::Unstamped,
            Ok(Some(identity)) => identity,
        };
        let Some(newest) = &self.newest else {
            return Admission::Newer;
        };
        match identity.sequence.cmp(&newest.sequence) {
            std::cmp::Ordering::Less => Admission::Older,
            std::cmp::Ordering::Greater => Admission::Newer,
            std::cmp::Ordering::Equal if identity == newest => Admission::Same,
            std::cmp::Ordering::Equal => Admission::Incoherent,
        }
    }

    /// Count and drop an older or incoherent stamped frame; every other frame proceeds.
    fn admits(&mut self, admission: Admission) -> bool {
        match admission {
            Admission::Older => self.stale += 1,
            Admission::Incoherent => self.incoherent += 1,
            Admission::Unstamped | Admission::Newer | Admission::Same => return true,
        }
        false
    }

    /// Record a frame as applied. Only a newer stamp moves the epoch forward.
    fn applied(&mut self, admission: Admission, output: &OutputDmxSnapshot) {
        self.decoded += 1;
        if admission == Admission::Newer
            && let Ok(Some(identity)) = live_identity(output)
        {
            self.newest = Some(identity.clone());
        }
    }

    /// Forget every proof. Called at each connection and scene boundary, never mid-epoch.
    pub(super) fn reset(&mut self) {
        self.newest = None;
        self.preload.reset();
    }
}

/// Whether a snapshot still waiting to be applied should be kept over one that just arrived.
///
/// The provider holds one pending snapshot, never a queue. Within one drain of the worker's
/// messages a stamped pending frame is not displaced by an older or contradictory one; anything
/// else arriving replaces it exactly as before.
pub(super) fn keeps_pending(pending: &OutputDmxSnapshot, arrived: &OutputDmxSnapshot) -> bool {
    let Ok(Some(pending)) = live_identity(pending) else {
        return false;
    };
    match live_identity(arrived) {
        Err(()) => true,
        Ok(None) => false,
        Ok(Some(arrived)) => {
            arrived.sequence < pending.sequence
                || (arrived.sequence == pending.sequence && arrived != pending)
        }
    }
}

/// Put the desk's 3D Point poses onto the values, in renderer axes.
///
/// A point's origin is where its own fixture instance stands in the scene, which is the same
/// resolution every placement takes, so the point and its slaves agree on where "here" is. The
/// desk's `(x, y, z)` — across, upstage, up — becomes the renderer's `(x, z, -y)`, and its
/// rotation is changed by basis conjugation, exactly as [`crate::transform`] converts a placement. A pose for a point
/// the scene does not hold is dropped: nothing can be slaved to a fixture that is not there.
pub(super) fn apply_point_poses(
    scene: &viz_scene::Scene,
    points: &[crate::wire::OutputPointPose],
    values: &mut SceneValues,
) {
    for point in points {
        let Some(instance) = scene
            .fixtures
            .iter()
            .find(|instance| instance.fixture_id == point.fixture_id)
        else {
            continue;
        };
        let [x, y, z] = point.offset_metres;
        let [rx, ry, rz] = point.rotation_degrees;
        let pose = viz_scene::PointPose {
            fixture_id: point.fixture_id,
            origin_metres: instance.position.to_array(),
            offset_metres: crate::transform::to_world(x, y, z).to_array(),
            rotation_degrees: crate::transform::rotation_to_world(rx, ry, rz).to_array(),
        };
        match values
            .position_points
            .iter_mut()
            .find(|held| held.fixture_id == point.fixture_id)
        {
            Some(held) => *held = pose,
            None => values.position_points.push(pose),
        }
    }
}

#[path = "desk_output_preload.rs"]
mod preload;

#[cfg(test)]
#[path = "desk_output_admission_tests.rs"]
mod admission_tests;
