use super::{
    playback_address_command::CommandPlaybackTarget,
    playback_selection_command::PlaybackSelectionTarget,
    programming_ports::{CommandLineProgrammer, ServerProgrammingPorts, clear_command_line},
};
use light_application::{
    ActionContext, ExecutionPolicy, PlaybackAction, PlaybackAddress, PlaybackCommand,
    ProgrammingExecution,
};

impl ServerProgrammingPorts<'_> {
    pub(super) fn select_playback_command(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        command: &str,
        _policy: ExecutionPolicy,
    ) -> Option<ProgrammingExecution> {
        match super::speed_group_binding_command::parse(command) {
            Ok(Some(_)) => return None,
            Ok(None) => {}
            Err(error) => return Some(self.recording_execution(context, command, Err(error))),
        }
        let parsed = match super::playback_selection_command::parse(command) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => return None,
            Err(error) => return Some(self.recording_execution(context, command, Err(error))),
        };
        let address = match parsed {
            PlaybackSelectionTarget::Playback(target) => address(target),
            PlaybackSelectionTarget::Cuelist { number } => {
                let resolved = (|| {
                    let show = self
                        .state()
                        .active_show
                        .current()
                        .clone()
                        .ok_or("no show is open".to_owned())?;
                    let document = crate::runtime::ActiveShowRepository::open(&show.path)
                        .and_then(|store| store.portable_document())
                        .map_err(|error| error.to_string())?;
                    light_application::cuelist_pool_catalog(&document)
                        .map_err(|error| error.message)?
                        .resolve(number)
                        .map(PlaybackAddress::CueList)
                        .ok_or_else(|| format!("Cuelist {number} does not exist"))
                })();
                match resolved {
                    Ok(address) => address,
                    Err(error) => {
                        return Some(self.recording_execution(context, command, Err(error)));
                    }
                }
            }
        };
        let result = super::super::playback_service::execute(
            self.state(),
            Some(self.session()),
            Some(&self.session().desk),
            context.clone(),
            PlaybackCommand {
                address,
                action: PlaybackAction::Select { pressed: true },
                control_source: match super::cue_navigation_action::control_source(
                    self.state(),
                    context,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        return Some(self.recording_execution(context, command, Err(error)));
                    }
                },
            },
        )
        .map_err(|error| error.message)
        .and_then(|_result| {
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

fn address(target: CommandPlaybackTarget) -> PlaybackAddress {
    super::cue_navigation_action::resolve_address(target)
}
