//! TL-594: an edit whose displayed source cannot be resolved is held quietly: no mutation,
//! revision or Undo step, a typed re-read reason, and no retained capture, so the next sample
//! adopts its own (fresh) lease. Both lanes.
use super::*;
use crate::{ProgrammingDisplayedLane, ProgrammingDisplayedSource, ProgrammingValuesHold};

/// Lease 1 is "expired": the adapter sets the hold. Any other lease adopts a pose.
struct LeasePorts<'a> {
    inner: &'a ValuesPorts,
    calls: Mutex<Vec<Option<u64>>>,
}

impl ProgrammingPorts for LeasePorts<'_> {
    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        _context: &ActionContext,
        _command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        panic!("displayed-source capture does not execute commands")
    }

    fn values_environment(
        &self,
        context: &ActionContext,
    ) -> Result<ProgrammingValuesEnvironment, crate::ActionError> {
        self.inner.values_environment(context)
    }

    fn prepare_family_edit_context(
        &self,
        _context: &ActionContext,
        _preload: bool,
        intent: &ProgrammingValueIntent,
        environment: &mut ProgrammingValuesEnvironment,
    ) -> Result<(), crate::ActionError> {
        let lease = intent.displayed_source.map(|source| source.lease);
        self.calls.lock().push(lease);
        if lease == Some(1) {
            environment.displayed_source_hold =
                Some(ProgrammingValuesHold::DisplayedSourceUnavailable);
        }
        Ok(())
    }

    fn persist(&self, context: &ActionContext, operation: &'static str) -> Option<String> {
        self.inner.persist(context, operation)
    }

    fn reconcile(&self, _context: &ActionContext, _reason: ProgrammingReconciliation) {}

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        Ok(None)
    }
}

fn apply(
    desk: &GestureDesk,
    request: &str,
    intent: ProgrammingValueIntent,
    ports: &dyn ProgrammingPorts,
) -> (Option<ProgrammingValuesHold>, u64) {
    let registry = &desk.setup.registry;
    let capture = registry.capture_mode_revision();
    if desk.preload {
        let result = desk
            .setup
            .service
            .handle_preload_values(
                ActionEnvelope {
                    context: desk
                        .setup
                        .context
                        .clone()
                        .with_request_id(request)
                        .with_expected_revision(registry.preload_values_revision()),
                    command: ProgrammingPreloadValuesRequest {
                        expected_capture_mode_revision: capture,
                        command: ProgrammingPreloadValuesCommand::ApplyIntent { intent },
                    },
                },
                ports,
            )
            .unwrap();
        (result.hold, result.outcome.revision())
    } else {
        let result = desk
            .setup
            .service
            .handle_values(
                desk.setup.action_with_capture(
                    request,
                    registry.normal_values_revision(),
                    capture,
                    ProgrammingValuesCommand::ApplyIntent { intent },
                ),
                ports,
            )
            .unwrap();
        (result.hold, result.outcome.revision())
    }
}

#[test]
fn an_unresolvable_displayed_source_holds_quietly_and_the_next_sample_recaptures() {
    for preload in [false, true] {
        let desk = GestureDesk::new(preload);
        let ports = LeasePorts {
            inner: &desk.setup.ports,
            calls: Mutex::new(Vec::new()),
        };
        let lane = if preload {
            ProgrammingDisplayedLane::Preload
        } else {
            ProgrammingDisplayedLane::Normal
        };
        let mut intent = desk.intent(false, ScalarEdit::Relative(5.0), "touch");
        intent.displayed_source = Some(ProgrammingDisplayedSource { lane, lease: 1 });
        let depth = desk.depth();
        let (hold, revision) = apply(&desk, "held", intent.clone(), &ports);
        assert_eq!(
            hold,
            Some(ProgrammingValuesHold::DisplayedSourceUnavailable)
        );
        let (_, unchanged) = apply(
            &desk,
            "probe",
            desk.intent(false, ScalarEdit::Relative(0.0), "probe"),
            &desk.setup.ports,
        );
        assert_eq!(unchanged, revision, "a hold creates no revision");
        assert_eq!(desk.depth(), depth, "a hold creates no Undo step");

        intent.displayed_source = Some(ProgrammingDisplayedSource { lane, lease: 2 });
        let (hold, _) = apply(&desk, "fresh", intent, &ports);
        assert_eq!(hold, None);
        assert_eq!(
            *ports.calls.lock(),
            vec![Some(1), Some(2)],
            "the held sample retained no capture; the next one adopts its own lease"
        );
        assert_eq!(desk.value(desk.setup.fixtures[0], false), angles(5.0, 20.0));
    }
}
