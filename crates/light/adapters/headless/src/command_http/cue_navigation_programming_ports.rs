//! Routes the CUE navigation family through the Programming interaction boundary.
//!
//! Programming keeps the shared command-line transition, reset, and history; the runtime transition
//! itself is the typed Playback action executed by `cue_navigation_action`. A rejected action
//! therefore leaves both the command line and the runtime untouched.

use super::cue_navigation_action;
use super::programming_ports::{ServerProgrammingPorts, clear_command_line};
use light_application::{ActionContext, ExecutionPolicy, ProgrammingExecution};
use light_programmer::ProgrammerRegistry;

impl ServerProgrammingPorts<'_> {
    pub(super) fn navigate_cue_command(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        policy: ExecutionPolicy,
    ) -> Option<ProgrammingExecution> {
        let parsed = match super::cue_navigation_command::parse(command) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => return None,
            Err(error) => {
                self.rejected_recording_command(context, command, &error);
                return Some(ProgrammingExecution::Rejected { error });
            }
        };
        let result = cue_navigation_action::execute(self.state(), self.session(), context, parsed)
            .and_then(|transition| {
                // Replaying the same request ID repeats neither the runtime transition nor the
                // command-line reset, the history entry, or the compatibility notification.
                if transition.replayed {
                    return Ok((1, None, true));
                }
                if matches!(policy, ExecutionPolicy::Compatibility) && transition.applied {
                    cue_navigation_action::emit_compatibility_change(
                        self.state(),
                        self.session(),
                        transition.playback,
                        parsed,
                    );
                }
                clear_command_line(programmers, self.session())?;
                Ok((
                    1,
                    self.accepted_recording_command(context, command, 1),
                    false,
                ))
            });
        Some(self.recording_execution(context, command, result))
    }
}
