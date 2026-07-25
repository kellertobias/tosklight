use light_application::{
    ActionContext, ActionEnvelope, ActionError, ProgrammingCueRecordingPorts, ProgrammingExecution,
    ProgrammingPorts,
};
use light_programmer::ProgrammerRegistry;

use super::programming_ports::{ServerProgrammingPorts, clear_command_line, recording_context};

impl ServerProgrammingPorts<'_> {
    pub(super) fn record_cue_command(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
    ) -> Option<ProgrammingExecution> {
        let parsed = match super::cue_recording_command::parse(command) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => return None,
            Err(error) => {
                self.rejected_recording_command(context, command, &error);
                return Some(ProgrammingExecution::Rejected { error });
            }
        };
        let result = self.execute_cue_recording(programmers, context, parsed, command);
        Some(self.recording_execution(
            context,
            command,
            result.map(|(warning, replayed)| (1, warning, replayed)),
        ))
    }

    fn execute_cue_recording(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        parsed: super::cue_recording_command::CueRecordCommand,
        raw_command: &str,
    ) -> Result<(Option<String>, bool), String> {
        let context = recording_context(context, "cue-record");
        let command = light_application::ProgrammingCueRecordRequest {
            show_id: self.active_show_id()?,
            target: parsed.target,
            operation: parsed.operation,
            cue_number: parsed.cue_number,
            timing: parsed.timing,
            cue_only: false,
            name: None,
            capture_policy: light_application::ProgrammingCueCapturePolicy::CurrentCapture,
            activation_policy: light_application::ProgrammingCueActivationPolicy::Hold,
            expected_show_revision:
                light_application::ProgrammingCueShowRevisionExpectation::Current,
        };
        let result = self
            .state()
            .programming
            .record_cue_within_interaction(
                ActionEnvelope {
                    context: context.clone(),
                    command,
                },
                self,
            )
            .map_err(|error| error.message)?;
        if result.replayed {
            return Ok((None, true));
        }
        clear_command_line(programmers, self.session())?;
        Ok((
            self.accepted_recording_command(&context, raw_command, 1),
            false,
        ))
    }

    pub(super) fn record_armed_cue(
        &self,
        target: light_application::ProgrammingCueRecordTarget,
    ) -> Result<light_application::ProgrammingCueRecordResult, String> {
        let context = ActionContext::operator(
            self.session().desk.id,
            self.session().user.id.0,
            self.session().id.0,
            light_application::ActionSource::Osc,
        )
        .with_request_id(format!("osc-cue-record-{}", uuid::Uuid::new_v4()));
        let action = ActionEnvelope {
            context: context.clone(),
            command: self.armed_cue_command(target)?,
        };
        self.state()
            .programming
            .run_external_interaction(&context, self, || {
                self.record_armed_cue_within(action, &context)
            })
            .map_err(|error| error.message)?
            .output
    }

    fn armed_cue_command(
        &self,
        target: light_application::ProgrammingCueRecordTarget,
    ) -> Result<light_application::ProgrammingCueRecordRequest, String> {
        Ok(light_application::ProgrammingCueRecordRequest {
            show_id: self.active_show_id()?,
            target,
            operation: light_application::ProgrammingCueRecordOperation::Overwrite,
            cue_number: None,
            timing: light_application::ProgrammingCueRecordTiming::default(),
            cue_only: false,
            name: None,
            capture_policy: light_application::ProgrammingCueCapturePolicy::CurrentCapture,
            activation_policy: light_application::ProgrammingCueActivationPolicy::GoToIfNormal,
            expected_show_revision:
                light_application::ProgrammingCueShowRevisionExpectation::Current,
        })
    }

    fn record_armed_cue_within(
        &self,
        action: ActionEnvelope<light_application::ProgrammingCueRecordRequest>,
        context: &ActionContext,
    ) -> Result<light_application::ProgrammingCueRecordResult, String> {
        let result = self
            .state()
            .programming
            .record_cue_within_interaction(action, self)
            .map_err(|error| {
                self.rejected_recording_command(context, "RECORD", &error.message);
                error.message
            })?;
        if !result.replayed {
            clear_command_line(&self.state().programmers, self.session())?;
            self.accepted_recording_command(context, "RECORD", 1);
        }
        Ok(result)
    }
}

impl ProgrammingCueRecordingPorts for ServerProgrammingPorts<'_> {
    fn authorize_cue_recording(&self, context: &ActionContext) -> Result<(), ActionError> {
        <Self as ProgrammingPorts>::authorize(self, context)
    }

    fn cue_recording_environment(
        &self,
        context: &ActionContext,
        request: &light_application::ProgrammingCueRecordRequest,
    ) -> Result<light_application::ProgrammingCueRecordingEnvironment, ActionError> {
        super::cue_recording_environment::environment(self.state(), context, request)
    }

    fn commit_cue(
        &self,
        context: &ActionContext,
        commit: &light_application::ProgrammingCueCommit,
    ) -> Result<light_application::ProgrammingCueCommitResult, ActionError> {
        super::cue_recording_ports::commit(self.state(), context, commit)
    }

    fn activate_recorded_cue(
        &self,
        context: &ActionContext,
        playback_number: u16,
        cue_number: light_application::CueNumber,
    ) -> Option<light_application::ProgrammingCueActivationCompletion> {
        let command = light_application::PlaybackCommand {
            address: light_application::PlaybackAddress::Pool(playback_number),
            action: light_application::PlaybackAction::GoTo(cue_number),
            surface: playback_surface(context.source),
        };
        let result = super::super::playback_service::execute(
            self.state(),
            Some(self.session()),
            Some(&self.session().desk),
            context.clone(),
            command,
        );
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                report_activation_failure(self, context, playback_number, cue_number, &error);
                return None;
            }
        };
        Some(light_application::ProgrammingCueActivationCompletion {
            projection: result.projection,
            event_sequence: result.event_sequence,
        })
    }
}

fn report_activation_failure(
    ports: &ServerProgrammingPorts<'_>,
    context: &ActionContext,
    playback_number: u16,
    cue_number: light_application::CueNumber,
    error: &super::super::ApiError,
) {
    super::super::emit(
        ports.state(),
        "cue_record_activation_failed",
        serde_json::json!({
            "request_id": context.request_id,
            "correlation_id": context.correlation_id,
            "desk_id": context.desk_id,
            "session_id": context.session_id,
            "playback_number": playback_number,
            "cue_number": cue_number.value(),
            "error": error.message,
        }),
    );
}

const fn playback_surface(
    source: light_application::ActionSource,
) -> light_application::PlaybackSurface {
    match source {
        light_application::ActionSource::Osc => light_application::PlaybackSurface::Osc,
        light_application::ActionSource::Matter => light_application::PlaybackSurface::Matter,
        _ => light_application::PlaybackSurface::Virtual,
    }
}
