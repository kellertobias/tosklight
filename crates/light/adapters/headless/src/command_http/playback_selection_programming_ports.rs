use super::{
    playback_address_command::CommandPlaybackTarget,
    playback_selection_command::PlaybackSelectionTarget,
    programming_ports::{CommandLineProgrammer, ServerProgrammingPorts, clear_command_line},
};
use light_application::{
    ActionContext, ActionSource, ExecutionPolicy, PlaybackAction, PlaybackAddress, PlaybackCommand,
    PlaybackSurface, ProgrammingExecution,
};

impl ServerProgrammingPorts<'_> {
    pub(super) fn select_playback_command(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        command: &str,
        _policy: ExecutionPolicy,
    ) -> Option<ProgrammingExecution> {
        let parsed = match super::playback_selection_command::parse(command) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => return None,
            Err(error) => return Some(self.recording_execution(context, command, Err(error))),
        };
        let address = match parsed {
            PlaybackSelectionTarget::Playback(target) => address(target),
            PlaybackSelectionTarget::Cuelist { number } => PlaybackAddress::Pool(number),
        };
        let result = super::super::playback_service::execute(
            self.state(),
            Some(self.session()),
            Some(&self.session().desk),
            context.clone(),
            PlaybackCommand {
                address,
                action: PlaybackAction::Select { pressed: true },
                surface: surface(context.source),
            },
        )
        .map_err(|error| error.message)
        .and_then(|result| {
            if result.replayed {
                return Ok((1, None, true));
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

fn address(target: CommandPlaybackTarget) -> PlaybackAddress {
    match target {
        CommandPlaybackTarget::CurrentPage { slot } => PlaybackAddress::CurrentPage { slot },
        CommandPlaybackTarget::ExplicitPage { page, slot } => {
            PlaybackAddress::ExplicitPage { page, slot }
        }
        CommandPlaybackTarget::Virtual(address) => PlaybackAddress::Virtual(address),
    }
}

const fn surface(source: ActionSource) -> PlaybackSurface {
    match source {
        ActionSource::Osc => PlaybackSurface::Osc,
        ActionSource::Matter => PlaybackSurface::Matter,
        ActionSource::UserInterface | ActionSource::Http => PlaybackSurface::Virtual,
        _ => PlaybackSurface::Physical,
    }
}
