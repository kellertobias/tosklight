use super::programming_ports::{ServerProgrammingPorts, clear_command_line, recording_context};
use light_application::{
    ActionContext, ExecutionPolicy, ProgrammingCueDeletionExpectation,
    ProgrammingCueDeletionRequest, ProgrammingCueDeletionState, ProgrammingExecution,
};
use light_programmer::ProgrammerRegistry;

impl ServerProgrammingPorts<'_> {
    pub(super) fn delete_cue_command(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        policy: ExecutionPolicy,
    ) -> Option<ProgrammingExecution> {
        if !super::cue_deletion_command::is_cue_deletion(command) {
            return None;
        }
        let result = self.execute_cue_deletion(programmers, context, command, policy);
        Some(self.recording_execution(context, command, result))
    }

    fn execute_cue_deletion(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        raw_command: &str,
        policy: ExecutionPolicy,
    ) -> Result<(usize, Option<String>, bool), String> {
        let parsed = super::cue_deletion_command::parse(raw_command)?
            .ok_or("Cue deletion command was not recognized")?;
        let show_id = self.active_show_id()?;
        let context = recording_context(context, "cue-delete");
        let ports = super::ServerProgrammingCueDeletionPorts::new(
            self.state().clone(),
            self.session().clone(),
            true,
        );
        let result = self
            .state()
            .programming
            .delete_cue_within_interaction(
                &context,
                &ProgrammingCueDeletionRequest {
                    show_id,
                    address: parsed.address,
                    cue_number: parsed.cue_number,
                    expectation: ProgrammingCueDeletionExpectation::Current,
                },
                &self.state().active_show_service,
                &ports,
            )
            .map_err(|error| error.message)?;
        publish_compatibility(self, &result, policy);
        clear_command_line(programmers, self.session())?;
        let warning = self.accepted_recording_command(&context, raw_command, 1);
        Ok((1, warning, false))
    }
}

fn publish_compatibility(
    ports: &ServerProgrammingPorts<'_>,
    outcome: &light_application::ProgrammingCueDeletionOutcome,
    policy: ExecutionPolicy,
) {
    if policy != ExecutionPolicy::Compatibility {
        return;
    }
    let ProgrammingCueDeletionState::Changed { .. } = outcome.state else {
        return;
    };
    let projection = &outcome.cue_list;
    super::super::emit(
        ports.state(),
        "show_object_changed",
        serde_json::json!({
            "show_id":outcome.show_id,
            "kind":"cue_list",
            "id":projection.object_id,
            "revision":projection.object_revision,
        }),
    );
}
