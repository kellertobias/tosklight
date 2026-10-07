//! TL-556: Preload first-edit adoption reads only the injected Pending source. The source here
//! is the real retained Position Pending rig (synthetic authored profile, real engine, paired
//! evaluator and accepted pair); production installs nothing until TL-548.
use super::*;
use crate::runtime::output_scheduler::{
    PendingAttemptTicket, PendingNativeReadout, RealPositionPendingRig,
};
use crate::runtime::position_readout::{
    CapturedPositionReadouts, PendingPositionReadoutSource, capture_pending_position_readouts,
};
use light_application::{ProgrammingValueIntent, ProgrammingValueOperation};
use light_core::programming::{
    ComponentEdit, FamilyEditContext, JointAngles, PositionIntent, ProgrammingComponent,
    ProgrammingOwner, ScalarEdit, edit_family,
};
use light_core::{ProgrammerId, SessionId};
use light_wire::v2::output_control::OutputFrameIdentity;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const TICKET: u64 = 3;

/// Test double for the TL-548 source. The rig's paired evaluator is thread-confined (`!Send`),
/// so the double holds the readouts that `capture_pending_position_readouts` built from the
/// rig's accepted pair and the exact capture, and serves them only to the desk Programmer.
struct RigSource {
    captured: CapturedPositionReadouts,
    desk_programmer: ProgrammerId,
    calls: AtomicUsize,
    bound: Mutex<Vec<OutputFrameIdentity>>,
}

impl PendingPositionReadoutSource for RigSource {
    fn capture(
        &self,
        programmer: ProgrammerId,
        owners: &[FixtureId],
    ) -> Option<CapturedPositionReadouts> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if programmer != self.desk_programmer {
            return None;
        }
        let mut captured = self.captured.clone();
        captured.owners = owners
            .iter()
            .map(|owner| {
                captured
                    .owners
                    .iter()
                    .find(|entry| entry.owner == *owner)
                    .cloned()
            })
            .collect::<Option<_>>()?;
        self.bound.lock().unwrap().push(captured.identity.clone());
        Some(captured)
    }
}

struct Desk {
    state: AppState,
    directory: std::path::PathBuf,
    session: SessionId,
    owner: FixtureId,
    rig: RealPositionPendingRig,
    source: Arc<RigSource>,
}

impl Drop for Desk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// A desk whose active show is the rig episode's show, with an accepted Target pair.
fn desk() -> Desk {
    let (state, directory) = crate::runtime::tests::test_state();
    let mut rig = RealPositionPendingRig::new(false);
    rig.fix_target([2., 6., 1.]);
    assert_eq!(rig.attempt().successful_attempts, 1);
    let session = SessionId::new();
    let desk_programmer = state.programming.start(session).id;
    state
        .active_show
        .replace_current(Some(light_show::ShowEntry {
            is_base_show: false,
            id: rig.identity().show_id,
            name: "Pending adoption".into(),
            path: directory
                .join("shows/pending.toskshow")
                .display()
                .to_string(),
            revision: 0,
            updated_at: String::new(),
            created_at: None,
            last_loaded_at: None,
            revision_copy: None,
        }));
    let owner = rig.target();
    let accepted = rig.accepted().unwrap();
    let captured = capture_pending_position_readouts(
        rig.engine(),
        rig.identity(),
        PendingAttemptTicket::new(TICKET),
        accepted,
        &accepted.capture,
        &[owner],
    )
    .unwrap();
    let source = Arc::new(RigSource {
        captured,
        desk_programmer,
        calls: AtomicUsize::new(0),
        bound: Mutex::default(),
    });
    Desk {
        state,
        directory,
        session,
        owner,
        rig,
        source,
    }
}

fn intent(owner: FixtureId) -> ProgrammingValueIntent {
    ProgrammingValueIntent {
        fixture_ids: vec![owner],
        group_id: None,
        attribute: ProgrammingOwner::Position.key(),
        operation: ProgrammingValueOperation::ComponentEdits(vec![ComponentEdit::Scalar {
            component: ProgrammingComponent::Pan,
            operation: ScalarEdit::Relative(5.),
        }]),
        undo_group: Some("pending-position-turn".into()),
        timing: Default::default(),
        displayed_source: None,
        color_adoption: Default::default(),
    }
}

fn prepare(desk: &Desk, session: SessionId, preload: bool) -> ProgrammingValuesEnvironment {
    let mut environment = values_environment(&desk.state);
    let stale = JointAngles {
        pan_degrees: -999.,
        tilt_degrees: -999.,
    };
    environment
        .family_contexts
        .entry(desk.owner)
        .or_default()
        .solved_angles = Some(stale);
    prepare_family_edit_context(
        &desk.state,
        session,
        preload,
        &intent(desk.owner),
        &mut environment,
    );
    environment
}

/// The expected After pose, requested intent and native identity of the rig's accepted pair.
fn expected(desk: &Desk) -> (JointAngles, AttributeValue, OutputFrameIdentity) {
    let rig = &desk.rig;
    let accepted = rig.accepted().unwrap();
    let sidecar = accepted
        .value
        .after
        .sidecars
        .iter()
        .find(|row| row.owner == ProgrammingOwner::Position)
        .unwrap();
    let native = PendingNativeReadout::prepare(
        rig.identity(),
        PendingAttemptTicket::new(TICKET),
        accepted,
        &accepted.capture,
    )
    .unwrap();
    let readout = rig
        .engine()
        .position_commanded_readouts_from_physical(
            accepted.value.after.frame_token.generation(),
            &accepted.value.rendered.projection.physical,
            &[desk.owner],
        )
        .unwrap();
    (
        readout[0].common_angles().unwrap(),
        sidecar.value.clone(),
        native.lane().frame.clone().unwrap(),
    )
}

#[tokio::test]
async fn preload_first_edit_adopts_the_accepted_after_pose_once_with_the_pending_identity() {
    let desk = desk();
    desk.state
        .output
        .pending_position_readouts()
        .install(desk.source.clone());
    let (pose, requested, identity) = expected(&desk);
    let address = (desk.owner, ProgrammingOwner::Position.key());

    let environment = prepare(&desk, desk.session, true);
    assert_eq!(desk.source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(*desk.source.bound.lock().unwrap(), vec![identity]);
    let context = &environment.family_contexts[&desk.owner];
    assert!(context.position_adoption_attempted);
    assert_eq!(context.solved_angles, Some(pose));
    assert_eq!(environment.current_values[&address], requested);
    assert!(
        matches!(&requested, AttributeValue::Position(intent) if **intent
        == PositionIntent::target(light_core::programming::TargetReference::Origin, [2., 6., 1.]))
    );

    // The turn applies once on top of the adopted accepted pose: Target becomes that Angle
    // pair plus exactly one +5 Pan, never re-solved or applied twice.
    let ProgrammingValueOperation::ComponentEdits(edits) = intent(desk.owner).operation else {
        unreachable!()
    };
    let edited = edit_family(
        &environment.current_values[&address],
        &edits,
        &FamilyEditContext {
            solved_angles: context.solved_angles,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        edited,
        AttributeValue::Position(Arc::new(PositionIntent::angles(
            pose.pan_degrees + 5.,
            pose.tilt_degrees
        )))
    );

    // Navigation (no Angle edit) never consults the Pending source.
    let mut navigation = intent(desk.owner);
    navigation.operation = ProgrammingValueOperation::ComponentEdits(vec![]);
    let mut environment = values_environment(&desk.state);
    prepare_family_edit_context(
        &desk.state,
        desk.session,
        true,
        &navigation,
        &mut environment,
    );
    assert_eq!(desk.source.calls.load(Ordering::SeqCst), 1);
    assert!(!environment.current_values.contains_key(&address));
}

#[tokio::test]
async fn preload_without_a_source_stays_quiet_and_never_reads_live() {
    let desk = desk();
    let address = (desk.owner, ProgrammingOwner::Position.key());
    let baseline = values_environment(&desk.state)
        .current_values
        .get(&address)
        .cloned();
    let quiet = prepare(&desk, desk.session, true);
    let context = &quiet.family_contexts[&desk.owner];
    assert!(context.position_adoption_attempted);
    assert_eq!(
        context.solved_angles, None,
        "no Pending source: no adoption"
    );
    assert_eq!(quiet.current_values.get(&address).cloned(), baseline);
    assert_eq!(desk.source.calls.load(Ordering::SeqCst), 0);

    // An installed source that has nothing for this desk's Programmer is equally quiet.
    let unrelated = Arc::new(RigSource {
        captured: desk.source.captured.clone(),
        desk_programmer: ProgrammerId::new(),
        calls: AtomicUsize::new(0),
        bound: Mutex::default(),
    });
    desk.state
        .output
        .pending_position_readouts()
        .install(unrelated.clone());
    let foreign = prepare(&desk, desk.session, true);
    assert_eq!(foreign.family_contexts[&desk.owner].solved_angles, None);
    assert_eq!(unrelated.calls.load(Ordering::SeqCst), 1);
    assert!(unrelated.bound.lock().unwrap().is_empty());
    desk.state
        .output
        .pending_position_readouts()
        .install(desk.source.clone());

    // Live first edits never consult the Pending source.
    let live = prepare(&desk, desk.session, false);
    assert_eq!(live.family_contexts[&desk.owner].solved_angles, None);
    assert_eq!(desk.source.calls.load(Ordering::SeqCst), 0);

    desk.state.output.pending_position_readouts().clear();
    assert_eq!(
        prepare(&desk, desk.session, true).family_contexts[&desk.owner].solved_angles,
        None
    );
}

#[tokio::test]
async fn preload_readouts_of_another_show_are_not_adopted() {
    let desk = desk();
    desk.state
        .output
        .pending_position_readouts()
        .install(desk.source.clone());
    desk.state.active_show.replace_current(None);
    let environment = prepare(&desk, desk.session, true);
    assert_eq!(desk.source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(environment.family_contexts[&desk.owner].solved_angles, None);
}
