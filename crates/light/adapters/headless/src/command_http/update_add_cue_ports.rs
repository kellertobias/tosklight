//! UPDATE with the Add Cue option stores the programmer as a new Cue in the addressed Cuelist.

use light_application::programming_update::RecordUpdateOption;
use light_application::{ActionContext, ProgrammingExecution};

use super::cue_recording_command::CueRecordCommand;
use super::programming_ports::{CommandLineProgrammer, ServerProgrammingPorts};
use super::record_update_option::{parse_option, update_default};

impl ServerProgrammingPorts<'_> {
    pub(super) fn update_add_cue_command(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        command: &str,
    ) -> Option<ProgrammingExecution> {
        let default = update_default(self.state());
        let parsed = match add_cue_record_command(command, default) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => return None,
            Err(error) => return Some(ProgrammingExecution::Rejected { error }),
        };
        let result = self.execute_cue_recording(programmers, context, parsed, command);
        Some(self.recording_execution(
            context,
            command,
            result.map(|(warning, replayed)| (1, warning, replayed)),
        ))
    }
}

/// The Record command an `UPDATE` line stands for when its option is Add Cue and it addresses a
/// Cuelist. A Cue number in the Update address only names the Cuelist; the new Cue is appended.
fn add_cue_record_command(
    command: &str,
    default: RecordUpdateOption,
) -> Result<Option<CueRecordCommand>, String> {
    let (tokens, _) = super::super::tokenize_programmer_command(command)?;
    let Some((first, body)) = tokens.split_first() else {
        return Ok(None);
    };
    if first != "UPDATE" {
        return Ok(None);
    }
    let (option, target) = parse_option(body);
    let named_mode = body
        .first()
        .is_some_and(|token| matches!(token.as_str(), "TRACKED" | "KNOWN" | "ALL"));
    let option = match option {
        Some(option) => option,
        None if named_mode => return Ok(None),
        None => default,
    };
    if option != RecordUpdateOption::AddCue {
        return Ok(None);
    }
    let cuelist = match target.first().map(String::as_str) {
        Some("CUE") => vec!["CUE".to_owned()],
        Some("CUELIST" | "PBK" | "VPBK") => {
            let end = target
                .iter()
                .skip(1)
                .position(|token| token == "CUE")
                .map_or(target.len(), |index| index + 1);
            target[..end].to_vec()
        }
        _ => return Ok(None),
    };
    let record = format!("RECORD ADD CUE {}", cuelist.join(" "));
    super::cue_recording_command::parse(&record)
}

#[cfg(test)]
mod tests {
    use super::super::cue_recording_command::CueRecordCommandTarget;
    use super::*;
    use light_application::ProgrammingCueRecordOperation;

    #[test]
    fn add_cue_update_appends_to_the_addressed_cuelist() {
        let parsed =
            add_cue_record_command("UPDATE ADD CUE CUELIST 4 CUE 2", RecordUpdateOption::Smart)
                .unwrap()
                .unwrap();
        assert_eq!(parsed.target, CueRecordCommandTarget::Cuelist { number: 4 });
        assert_eq!(
            parsed.operation,
            Some(ProgrammingCueRecordOperation::AddCue)
        );
        assert!(parsed.cue_number.is_none());

        let selected = add_cue_record_command("UPDATE CUE 2.1", RecordUpdateOption::AddCue)
            .unwrap()
            .unwrap();
        assert_eq!(selected.target, CueRecordCommandTarget::SelectedCuelist);
        assert!(selected.cue_number.is_none());

        assert!(
            add_cue_record_command("UPDATE PBK 2.6", RecordUpdateOption::AddCue)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn other_options_and_named_modes_stay_updates() {
        for command in [
            "UPDATE CUE 2",
            "UPDATE MERGE PBK 1",
            "UPDATE ALL PBK 1",
            "UPDATE ADD CUE GROUP 3",
            "RECORD PBK 1",
        ] {
            assert!(
                add_cue_record_command(command, RecordUpdateOption::Smart)
                    .unwrap()
                    .is_none(),
                "{command}"
            );
        }
        assert!(
            add_cue_record_command("UPDATE KNOWN PBK 1", RecordUpdateOption::AddCue)
                .unwrap()
                .is_none()
        );
    }
}
