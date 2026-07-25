use light_application::{ActionContext, ExecutionPolicy, ProgrammingExecution, SpeedGroupOutcome};
use light_programmer::ProgrammerRegistry;

use super::{
    programming_ports::{ServerProgrammingPorts, clear_command_line, recording_context},
    speed_group_action,
};

impl ServerProgrammingPorts<'_> {
    pub(super) fn speed_group_command(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        policy: ExecutionPolicy,
    ) -> Option<ProgrammingExecution> {
        let parsed = match super::speed_group_command::parse(command) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => return None,
            Err(error) => {
                self.rejected_recording_command(context, command, &error);
                return Some(ProgrammingExecution::Rejected { error });
            }
        };
        let context = recording_context(context, "speed-group");
        let addressed = super::speed_group_command::addressed(parsed).len();
        let result = speed_group_action::execute(self.state(), self.session(), &context, parsed)
            .and_then(|result| {
                if result.replayed {
                    return Ok((addressed, result.warning, true));
                }
                if matches!(policy, ExecutionPolicy::Compatibility)
                    && result.outcome == SpeedGroupOutcome::Applied
                {
                    speed_group_action::emit_compatibility_change(
                        self.state(),
                        command,
                        parsed,
                        &result,
                    )?;
                }
                clear_command_line(programmers, self.session())?;
                self.record_speed_group_history(
                    &context,
                    command,
                    addressed,
                    result.warning.as_deref(),
                );
                Ok((addressed, result.warning, false))
            });
        Some(self.recording_execution(&context, command, result))
    }

    fn record_speed_group_history(
        &self,
        context: &ActionContext,
        command: &str,
        applied: usize,
        warning: Option<&str>,
    ) {
        let feedback = warning.map_or_else(
            || format!("Applied to {applied} target(s)"),
            |warning| format!("Applied to {applied} target(s); {warning}"),
        );
        super::super::record_command_history(
            self.state(),
            self.session(),
            command,
            "accepted",
            &feedback,
            self.source(),
            context.request_id.as_deref(),
        );
    }
}
