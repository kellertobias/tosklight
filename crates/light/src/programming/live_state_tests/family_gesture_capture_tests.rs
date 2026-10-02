use super::*;

struct CapturePorts<'a> {
    inner: &'a ValuesPorts,
    calls: Mutex<Vec<bool>>,
    fail: Mutex<bool>,
    absent: bool,
}

impl<'a> CapturePorts<'a> {
    fn new(inner: &'a ValuesPorts) -> Self {
        Self {
            inner,
            calls: Mutex::new(Vec::new()),
            fail: Mutex::new(false),
            absent: false,
        }
    }
}

impl ProgrammingPorts for CapturePorts<'_> {
    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        _context: &ActionContext,
        _command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        panic!("family capture does not execute commands")
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
        preload: bool,
        _intent: &ProgrammingValueIntent,
        environment: &mut ProgrammingValuesEnvironment,
    ) -> Result<(), crate::ActionError> {
        self.calls.lock().push(preload);
        if *self.fail.lock() {
            return Err(crate::ActionError::new(
                ActionErrorKind::Unavailable,
                "test capture unavailable",
            ));
        }
        if self.absent {
            environment.family_contexts.clear();
        } else {
            let sequence = self.calls.lock().len() as f32;
            for context in environment.family_contexts.values_mut() {
                context.solved_angles = Some(JointAngles {
                    pan_degrees: sequence * 100.0,
                    tilt_degrees: sequence * 10.0,
                });
            }
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

#[test]
fn lazy_capture_is_once_per_touch_in_both_lanes_including_neutral_and_replayed_samples() {
    for preload in [false, true] {
        let desk = GestureDesk::new(preload);
        let ports = CapturePorts::new(&desk.setup.ports);
        let neutral = desk.intent(false, ScalarEdit::Relative(0.0), "touch");
        desk.apply_with_ports("neutral", neutral.clone(), &ports)
            .unwrap();
        desk.apply_with_ports("neutral", neutral, &ports).unwrap();
        desk.apply_with_ports(
            "move",
            desk.intent(false, ScalarEdit::Relative(1.0), "touch"),
            &ports,
        )
        .unwrap();
        assert_eq!(*ports.calls.lock(), vec![preload]);
        assert_eq!(
            desk.value(desk.setup.fixtures[0], false),
            angles(101.0, 10.0)
        );
        desk.apply_with_ports(
            "new-touch",
            desk.intent(false, ScalarEdit::Relative(1.0), "next"),
            &ports,
        )
        .unwrap();
        assert_eq!(*ports.calls.lock(), vec![preload, preload]);
        assert_eq!(
            desk.value(desk.setup.fixtures[0], false),
            angles(102.0, 10.0)
        );
    }
}

#[test]
fn lazy_capture_skips_non_edits_and_invalid_members_and_captures_each_one_shot() {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        desk.setup
            .ports
            .environment
            .group_memberships
            .insert("empty".into(), 0);
        desk.setup
            .ports
            .environment
            .group_members
            .insert("empty".into(), vec![]);
        let ports = CapturePorts::new(&desk.setup.ports);
        let mut empty = desk.intent(false, ScalarEdit::Relative(0.0), "empty");
        empty.fixture_ids.clear();
        desk.apply_with_ports("empty", empty, &ports).unwrap();
        let mut no_edits = desk.intent(false, ScalarEdit::Relative(0.0), "no-edits");
        no_edits.operation = ProgrammingValueOperation::ComponentEdits(vec![]);
        desk.apply_with_ports("no-edits", no_edits, &ports).unwrap();
        let mut empty_group = desk.intent(true, ScalarEdit::Relative(0.0), "empty-group");
        empty_group.group_id = Some("empty".into());
        desk.apply_with_ports("empty-group", empty_group, &ports)
            .unwrap();
        let mut invalid = desk.intent(false, ScalarEdit::Relative(1.0), "invalid");
        invalid.fixture_ids = vec![FixtureId::new()];
        assert!(desk.apply_with_ports("invalid", invalid, &ports).is_err());
        let mut absolute = desk.intent(false, ScalarEdit::Relative(1.0), "absolute");
        absolute.operation = ProgrammingValueOperation::AbsoluteSet(angles(1.0, 2.0));
        desk.apply_with_ports("absolute", absolute, &ports).unwrap();
        assert!(ports.calls.lock().is_empty());
        for request in ["one-shot-a", "one-shot-b"] {
            let mut intent = desk.intent(false, ScalarEdit::Relative(1.0), "unused");
            intent.undo_group = None;
            desk.apply_with_ports(request, intent, &ports).unwrap();
        }
        assert_eq!(*ports.calls.lock(), vec![preload, preload]);
    }
}

#[test]
fn failed_capture_is_retryable_and_successful_absence_is_retained() {
    for preload in [false, true] {
        let desk = GestureDesk::new(preload);
        let mut ports = CapturePorts::new(&desk.setup.ports);
        *ports.fail.lock() = true;
        let intent = desk.intent(false, ScalarEdit::Relative(0.0), "touch");
        let depth = desk.depth();
        assert!(
            desk.apply_with_ports("retry", intent.clone(), &ports)
                .is_err()
        );
        assert_eq!(desk.depth(), depth);
        *ports.fail.lock() = false;
        ports.absent = true;
        desk.apply_with_ports("retry", intent.clone(), &ports)
            .unwrap();
        ports.absent = false;
        desk.apply_with_ports("still-neutral", intent, &ports)
            .unwrap();
        assert_eq!(*ports.calls.lock(), vec![preload, preload]);
        assert_eq!(desk.depth(), depth);
    }
}
