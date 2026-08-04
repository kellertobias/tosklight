use std::sync::Arc;

use super::cue_link_command::{CueLinkAddress, CueLinkCommand};
use super::programming_ports::{
    CommandLineProgrammer, ServerProgrammingPorts, clear_command_line, recording_context,
};
use light_application::{
    ActionContext, ActionEnvelope, PlaybackTopologyAction, PlaybackTopologyCommand,
    ProgrammingExecution, lossless_json,
};
use light_playback::{CueList, CueTrigger, PlaybackTarget};

impl ServerProgrammingPorts<'_> {
    pub(super) fn link_cue_command(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        command: &str,
    ) -> Option<ProgrammingExecution> {
        let parsed = match super::cue_link_command::parse(command) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => return None,
            Err(error) => return Some(ProgrammingExecution::Rejected { error }),
        };
        let result = self.execute_cue_link(programmers, context, command, parsed);
        Some(self.recording_execution(context, command, result.map(|warning| (1, warning, false))))
    }

    fn execute_cue_link(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        raw_command: &str,
        parsed: CueLinkCommand,
    ) -> Result<Option<String>, String> {
        let (entry, store) = super::super::command_presets::active_show_store(self.state())?;
        let snapshot = self.state().output.snapshot();
        let playback_number = match parsed.address {
            CueLinkAddress::Selected => self
                .state()
                .installation
                .selected_playback(context.desk_id, entry.id)
                .map_err(|error| error.to_string())?
                .ok_or("no playback is selected")?,
            CueLinkAddress::Pool { playback_number } => playback_number,
            CueLinkAddress::PageSlot { page, slot } => snapshot
                .playback_pages
                .iter()
                .find(|candidate| candidate.number == page)
                .and_then(|candidate| candidate.slots.get(&slot))
                .copied()
                .ok_or("page playback is unassigned")?,
        };
        let definition = snapshot
            .playbacks
            .iter()
            .find(|definition| definition.number == playback_number)
            .ok_or("playback does not exist")?;
        let PlaybackTarget::CueList { cue_list_id } = definition.target else {
            return Err("playback is not assigned to a Cuelist".into());
        };
        let object = store
            .objects("cue_list")
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|object| {
                serde_json::from_value::<CueList>(object.body.clone())
                    .is_ok_and(|cue_list| cue_list.id == cue_list_id)
            })
            .ok_or("Cuelist object does not exist")?;
        let before: CueList = serde_json::from_value(object.body.clone())
            .map_err(|error| format!("stored Cuelist is invalid: {error}"))?;
        let mut cue_list = before.clone();
        let destination_id = cue_list
            .cues
            .iter()
            .find(|cue| cue.number == parsed.destination_number)
            .map(|cue| cue.id)
            .ok_or("Link destination Cue does not exist")?;
        let source = cue_list
            .cues
            .iter_mut()
            .find(|cue| cue.number == parsed.source_number)
            .ok_or("Link source Cue does not exist")?;
        source.trigger = CueTrigger::Link {
            cue_id: destination_id,
            delay_millis: parsed.delay_millis,
        };
        cue_list.validate()?;
        let raw_body = lossless_json::merge_typed(&object.body, &before, &cue_list)
            .map_err(|error| error.to_string())?;
        let show_revision = store
            .portable_revision()
            .map_err(|error| error.to_string())?
            .value();
        let context = recording_context(context, "cue-link").with_expected_revision(show_revision);
        let ports = super::super::ServerPlaybackTopologyPorts::new(
            self.state().clone(),
            self.session().clone(),
            entry.id,
        );
        self.state()
            .playback
            .handle_topology(
                ActionEnvelope {
                    context: context.clone(),
                    command: PlaybackTopologyCommand {
                        show_id: entry.id,
                        action: PlaybackTopologyAction::SaveCueList {
                            cue_list_id,
                            expected_revision: object.revision,
                            expected_object_id: Some(object.id),
                            cue_list,
                            raw_body: Arc::new(raw_body),
                        },
                    },
                },
                &ports,
            )
            .map_err(|error| error.message)?;
        clear_command_line(programmers, self.session())?;
        Ok(self.accepted_recording_command(&context, raw_command, 1))
    }
}
