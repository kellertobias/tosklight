//! The Record/Update option a command names, and the desk default a plain RECORD or UPDATE uses.

use light_application::ProgrammingCueRecordOperation;
use light_application::programming_update::RecordUpdateOption;

use super::super::AppState;

/// Reads a leading `SMART`, `MERGE`, `ADD EXISTING`, or `ADD CUE` option.
pub(crate) fn parse_option(body: &[String]) -> (Option<RecordUpdateOption>, &[String]) {
    let first = body.first().map(String::as_str);
    let second = body.get(1).map(String::as_str);
    match (first, second) {
        (Some("SMART"), _) => (Some(RecordUpdateOption::Smart), &body[1..]),
        (Some("MERGE"), _) => (Some(RecordUpdateOption::Merge), &body[1..]),
        (Some("ADD"), Some("EXISTING")) => (Some(RecordUpdateOption::AddExisting), &body[2..]),
        (Some("ADD"), Some("CUE")) => (Some(RecordUpdateOption::AddCue), &body[2..]),
        _ => (None, body),
    }
}

/// The Cue recording operation that carries out one Record option.
pub(crate) const fn record_operation(option: RecordUpdateOption) -> ProgrammingCueRecordOperation {
    match option {
        RecordUpdateOption::Smart => ProgrammingCueRecordOperation::Overwrite,
        RecordUpdateOption::Merge => ProgrammingCueRecordOperation::Merge,
        RecordUpdateOption::AddExisting => ProgrammingCueRecordOperation::AddMissing,
        RecordUpdateOption::AddCue => ProgrammingCueRecordOperation::AddCue,
    }
}

pub(crate) fn record_default(state: &AppState) -> RecordUpdateOption {
    state
        .installation
        .configuration()
        .update_settings
        .record_default
}

pub(crate) fn update_default(state: &AppState) -> RecordUpdateOption {
    state
        .installation
        .configuration()
        .update_settings
        .update_default
}

/// The option an armed `RECORD [option]` command line names, when it names nothing else.
pub(crate) fn armed_record_option(command: &str) -> Option<Option<RecordUpdateOption>> {
    let tokens = command
        .split_whitespace()
        .map(str::to_ascii_uppercase)
        .collect::<Vec<_>>();
    let (first, body) = tokens.split_first()?;
    if !matches!(first.as_str(), "RECORD" | "REC") {
        return None;
    }
    let (option, rest) = parse_option(body);
    rest.is_empty().then_some(option)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(value: &str) -> Vec<String> {
        value.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn reads_each_option_and_leaves_the_target() {
        let body = tokens("ADD EXISTING PBK 3");
        assert_eq!(
            parse_option(&body),
            (Some(RecordUpdateOption::AddExisting), &body[2..])
        );
        let body = tokens("ADD CUE CUE 4");
        assert_eq!(
            parse_option(&body),
            (Some(RecordUpdateOption::AddCue), &body[2..])
        );
        let body = tokens("MERGE CUE");
        assert_eq!(
            parse_option(&body),
            (Some(RecordUpdateOption::Merge), &body[1..])
        );
        let body = tokens("SMART PBK 1");
        assert_eq!(
            parse_option(&body),
            (Some(RecordUpdateOption::Smart), &body[1..])
        );
        let body = tokens("CUE 4");
        assert_eq!(parse_option(&body), (None, &body[..]));
    }

    #[test]
    fn an_armed_record_line_names_at_most_an_option() {
        assert_eq!(armed_record_option("RECORD"), Some(None));
        assert_eq!(
            armed_record_option("record merge "),
            Some(Some(RecordUpdateOption::Merge))
        );
        assert_eq!(
            armed_record_option("RECORD ADD CUE"),
            Some(Some(RecordUpdateOption::AddCue))
        );
        assert_eq!(armed_record_option("RECORD PBK 3"), None);
        assert_eq!(armed_record_option("UPDATE"), None);
    }
}
