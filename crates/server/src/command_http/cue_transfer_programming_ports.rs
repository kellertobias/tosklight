use super::programming_ports::{ServerProgrammingPorts, clear_command_line};
use light_application::{
    ActionContext, ProgrammingCueTransferMode, ProgrammingCueTransferRequest, ProgrammingExecution,
};
use light_programmer::{ProgrammerRegistry, ProgrammingChoiceOptionId};

impl ServerProgrammingPorts<'_> {
    pub(super) fn transfer_cue_command(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
    ) -> Option<ProgrammingExecution> {
        if !super::cue_transfer_command::is_cue_transfer(command) {
            return None;
        }
        let show_id = match self.active_show_id() {
            Ok(show_id) => show_id,
            Err(error) => return self.rejected_transfer(context, command, error),
        };
        let parsed = match super::cue_transfer_command::parse(command, show_id) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => return None,
            Err(error) => return self.rejected_transfer(context, command, error),
        };
        let result = match parsed.mode {
            None => self.prepare_transfer_choice(context, parsed.request),
            Some(mode) => {
                self.execute_transfer(programmers, context, command, parsed.request, mode)
            }
        };
        Some(result)
    }

    fn prepare_transfer_choice(
        &self,
        context: &ActionContext,
        request: light_application::ProgrammingCueTransferChoiceRequest,
    ) -> ProgrammingExecution {
        let ports = super::ServerProgrammingCueTransferPorts::new(
            self.state().clone(),
            self.session().clone(),
            true,
        );
        match self
            .state()
            .programming
            .prepare_cue_transfer_choice_within_interaction(
                context,
                request,
                &self.state().active_show_service,
                &ports,
            ) {
            Ok(pending_choice) => ProgrammingExecution::ChoiceRequired { pending_choice },
            Err(error) => ProgrammingExecution::Rejected {
                error: error.message,
            },
        }
    }

    fn execute_transfer(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        raw_command: &str,
        request: light_application::ProgrammingCueTransferChoiceRequest,
        mode: ProgrammingCueTransferMode,
    ) -> ProgrammingExecution {
        let result = self.apply_transfer(context, &request, mode);
        let result = result.and_then(|()| {
            clear_command_line(programmers, self.session())?;
            Ok(self.accepted_recording_command(context, raw_command, 1))
        });
        self.recording_execution(context, raw_command, result.map(|warning| (1, warning)))
    }

    fn apply_transfer(
        &self,
        context: &ActionContext,
        request: &light_application::ProgrammingCueTransferChoiceRequest,
        mode: ProgrammingCueTransferMode,
    ) -> Result<(), String> {
        let ports = super::ServerProgrammingCueTransferPorts::new(
            self.state().clone(),
            self.session().clone(),
            true,
        );
        if let Some((typed, exact_context)) = self.pending_transfer(context, request, mode)? {
            self.state()
                .programming
                .cue_transfer_within_interaction(
                    &exact_context,
                    &typed,
                    &self.state().active_show_service,
                    &ports,
                )
                .map_err(|error| error.message)?;
        } else {
            self.state()
                .programming
                .current_cue_transfer_within_interaction(
                    context,
                    request,
                    mode,
                    &self.state().active_show_service,
                    &ports,
                )
                .map_err(|error| error.message)?;
        }
        Ok(())
    }

    fn pending_transfer(
        &self,
        context: &ActionContext,
        request: &light_application::ProgrammingCueTransferChoiceRequest,
        mode: ProgrammingCueTransferMode,
    ) -> Result<Option<(ProgrammingCueTransferRequest, ActionContext)>, String> {
        let command = self
            .state()
            .programmers
            .command_line_state(self.session().id)
            .ok_or("programmer command line does not exist")?;
        let Some(choice) = command.pending_choice else {
            return Ok(None);
        };
        let option = choice
            .options
            .iter()
            .find(|option| option.id == option_id(mode))
            .ok_or("Cue transfer choice does not provide the selected mode")?;
        let expected = match mode {
            ProgrammingCueTransferMode::Plain => &request.plain_command,
            ProgrammingCueTransferMode::Status => &request.status_command,
        };
        if normalize(&option.command) != normalize(expected) {
            return Err("explicit Cue transfer does not match the pending choice".into());
        }
        let request = ProgrammingCueTransferRequest {
            show_id: light_core::ShowId(choice.show_id),
            choice_id: choice.choice_id,
            mode,
            expected_command_line_revision: command.revision,
        };
        Ok(Some((
            request,
            context.clone().with_expected_revision(choice.show_revision),
        )))
    }

    fn rejected_transfer(
        &self,
        context: &ActionContext,
        command: &str,
        error: String,
    ) -> Option<ProgrammingExecution> {
        self.rejected_recording_command(context, command, &error);
        Some(ProgrammingExecution::Rejected { error })
    }
}

const fn option_id(mode: ProgrammingCueTransferMode) -> ProgrammingChoiceOptionId {
    match mode {
        ProgrammingCueTransferMode::Plain => ProgrammingChoiceOptionId::Plain,
        ProgrammingCueTransferMode::Status => ProgrammingChoiceOptionId::Status,
    }
}

fn normalize(command: &str) -> String {
    command
        .split_whitespace()
        .map(str::to_ascii_uppercase)
        .collect::<Vec<_>>()
        .join(" ")
}
