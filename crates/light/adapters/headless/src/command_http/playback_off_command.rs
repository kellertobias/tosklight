//! OFF uses canonical PBK addressing and the existing Playback Off authority.
use super::{
    playback_address_command::{self, CommandPlaybackTarget},
    programming_ports::{CommandLineProgrammer, ServerProgrammingPorts, clear_command_line},
};
use light_application::{ActionContext, PlaybackAction, PlaybackCommand, ProgrammingExecution};

pub(super) fn parse(command: &str) -> Result<Option<CommandPlaybackTarget>, String> {
    let (tokens, timing) = super::super::tokenize_programmer_command(command)?;
    if tokens.first().is_none_or(|token| token != "OFF") {
        return Ok(None);
    }
    if timing.fade || timing.fade_millis.is_some() || timing.delay_millis.is_some() {
        return Err("Playback Off timing is unsupported".into());
    }
    let (target, rest) = playback_address_command::parse(&tokens[1..])?;
    if !rest.is_empty() {
        return Err("unexpected tokens after playback address".into());
    }
    Ok(Some(target))
}
// Called only after the OSC subscriber, session, desk-lock and shifted-key guards.
pub(crate) fn open_running_output_on_repeated_off(
    state: &super::super::AppState,
    session: &super::super::Session,
    path: &str,
    request_id: Option<&str>,
) -> bool {
    use light_programmer::command_line::{
        CommandGesture, CommandGestureIntent, CommandGestureKind, CommandImmediateAction,
        CommandKey, command_gesture_intent,
    };
    let Some(current) = state.programming.command_line_state(session.id) else {
        return false;
    };
    if current.visible_text().trim() != "OFF" {
        return false;
    }
    let intent = command_gesture_intent(
        &current,
        CommandKey::Off,
        CommandGesture {
            kind: CommandGestureKind::Double,
            shifted: false,
        },
    );
    if !matches!(
        intent,
        CommandGestureIntent::Immediate(CommandImmediateAction::RunningOutput)
    ) {
        return false;
    }
    super::super::emit(
        state,
        "desk_action",
        serde_json::json!({
            "path":path,"desk_id":session.desk.id,"session_id":session.id,
            "request_id":request_id,"action":"running-output","source":"osc",
        }),
    );
    true
}
impl ServerProgrammingPorts<'_> {
    pub(super) fn off_playback_command(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        command: &str,
    ) -> Option<ProgrammingExecution> {
        let parsed = match parse(command) {
            Ok(Some(target)) => target,
            Ok(None) => return None,
            Err(error) => return Some(self.recording_execution(context, command, Err(error))),
        };
        let result = (|| {
            let control_source =
                super::cue_navigation_action::control_source(self.state(), context)?;
            super::super::playback_service::execute(
                self.state(),
                Some(self.session()),
                Some(&self.session().desk),
                context.clone(),
                PlaybackCommand {
                    address: super::cue_navigation_action::resolve_address(parsed),
                    action: PlaybackAction::Off { pressed: true },
                    control_source,
                },
            )
            .map_err(|error| error.message)?;
            clear_command_line(programmers, self.session())?;
            Ok((
                1,
                self.accepted_recording_command(context, command, 1),
                false,
            ))
        })();
        Some(self.recording_execution(context, command, result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tl786_off_uses_only_canonical_current_or_explicit_page_grammar() {
        assert_eq!(
            parse("OFF PBK 16").unwrap(),
            Some(CommandPlaybackTarget::CurrentPage { position: 16 })
        );
        assert_eq!(
            parse("OFF PBK 3.16").unwrap(),
            Some(CommandPlaybackTarget::ExplicitPage {
                page: 3,
                position: 16
            })
        );
        assert_eq!(
            parse("off pbk 99.1000").unwrap(),
            Some(CommandPlaybackTarget::ExplicitPage {
                page: 99,
                position: 1000
            })
        );
        for value in [
            "OFF",
            "OFF PBK",
            "OFF PBK 0",
            "OFF PBK 0.1",
            "OFF PBK 100.1",
            "OFF PBK 1.1001",
            "OFF PBK 3.16 EXTRA",
            "OFF PBK 3.16 TIME 5",
            "OFF PBK 3.16 DELAY 0",
            "OFF VPBK 16",
            "OFF VPLBK 16",
        ] {
            assert!(parse(value).is_err(), "{value}");
        }
        for suffix in ["TIME 5", "FADE 0", "DELAY 0"] {
            assert_eq!(
                parse(&format!("OFF PBK 3.16 {suffix}")).unwrap_err(),
                "Playback Off timing is unsupported"
            );
        }
        assert!(parse("PBK 3.16").unwrap().is_none());
        assert!(parse("FIXTURE 1 AT 50").unwrap().is_none());
        assert!(parse("DYNAMIC 1 OFF").unwrap().is_none());
    }
}
