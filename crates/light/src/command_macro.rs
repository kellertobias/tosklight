//! Show-owned command Macro definitions and document-level validation.
//!
//! A Macro is deliberately data, not executable installation code: every executable line is one
//! ordinary command-line command and is interpreted only by the authoritative Programming
//! service at run time.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const MAX_MACRO_NUMBER: u16 = 9_999;
pub const MAX_MACRO_NAME_BYTES: usize = 128;
pub const MAX_MACRO_SOURCE_BYTES: usize = 256 * 1024;
pub const MAX_MACRO_LINE_BYTES: usize = 16 * 1024;
pub const RESTORE_SELECTION_COMMAND: &str = "RESTORE SELECTION";
const MAX_DEFINE_EXPANSION_DEPTH: usize = 16;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MacroPresentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandMacroDefinition {
    pub id: Uuid,
    pub number: u16,
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub presentation: MacroPresentation,
}

impl CommandMacroDefinition {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_nil() {
            return Err("Macro id cannot be nil".into());
        }
        if self.number == 0 || self.number > MAX_MACRO_NUMBER {
            return Err(format!("Macro number must be within 1-{MAX_MACRO_NUMBER}"));
        }
        let name = self.name.trim();
        if name.is_empty() || name.len() > MAX_MACRO_NAME_BYTES {
            return Err(format!(
                "Macro name must contain 1-{MAX_MACRO_NAME_BYTES} UTF-8 bytes"
            ));
        }
        if self.source.len() > MAX_MACRO_SOURCE_BYTES {
            return Err(format!(
                "Macro source must not exceed {MAX_MACRO_SOURCE_BYTES} UTF-8 bytes"
            ));
        }
        for (index, line) in self.source.lines().enumerate() {
            if line.len() > MAX_MACRO_LINE_BYTES {
                return Err(format!(
                    "Macro line {} must not exceed {MAX_MACRO_LINE_BYTES} UTF-8 bytes",
                    index + 1
                ));
            }
        }
        compile_macro_source(&self.source)
            .map_err(|error| format!("Macro line {} is invalid: {}", error.line, error.message))?;
        if let Some(color) = &self.presentation.color
            && !valid_color(color)
        {
            return Err("Macro color must be a #RRGGBB value".into());
        }
        Ok(())
    }

    pub fn lines(&self) -> impl Iterator<Item = CommandMacroLine<'_>> {
        let mut lines = Vec::new();
        for (index, raw) in self.source.lines().enumerate() {
            for segment in raw.split(';') {
                let command = segment.trim();
                if command.starts_with('#') || command.starts_with("//") {
                    break;
                }
                if command.is_empty() {
                    continue;
                }
                lines.push(CommandMacroLine {
                    number: index + 1,
                    command,
                });
            }
        }
        lines.into_iter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandMacroLine<'a> {
    pub number: usize,
    pub command: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandMacroCompiledLine {
    pub number: usize,
    pub command: String,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandMacroDefinitionExpansion {
    pub line: usize,
    pub identifier: String,
    pub expansion: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandMacroCompilation {
    pub lines: Vec<CommandMacroCompiledLine>,
    pub definitions: Vec<CommandMacroDefinitionExpansion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandMacroCompileError {
    pub line: usize,
    pub message: String,
}

/// Compiles Macro-only source directives before ordinary commands reach the authoritative parser.
/// DEFINE entries are document-local, underscore-prefixed substitutions and never execute as
/// commands themselves. Newlines and semicolons are equivalent command separators.
pub fn compile_macro_source(
    source: &str,
) -> Result<CommandMacroCompilation, CommandMacroCompileError> {
    let mut segments = Vec::new();
    for (index, raw) in source.lines().enumerate() {
        for segment in raw.split(';') {
            let command = segment.trim();
            if command.starts_with('#') || command.starts_with("//") {
                break;
            }
            if !command.is_empty() {
                segments.push((index + 1, command));
            }
        }
    }
    let mut definitions = BTreeMap::<String, (usize, String)>::new();
    for (line, command) in &segments {
        if !starts_with_keyword(command, "DEFINE") {
            continue;
        }
        let mut parts = command.splitn(3, char::is_whitespace);
        let _define = parts.next();
        let identifier = parts.next().unwrap_or_default();
        let expansion = parts.next().unwrap_or_default().trim();
        if !valid_define_identifier(identifier) {
            return Err(compile_error(
                *line,
                "DEFINE requires an underscore-prefixed identifier with no spaces",
            ));
        }
        if expansion.is_empty() {
            return Err(compile_error(
                *line,
                "DEFINE requires a command-text expansion",
            ));
        }
        if definitions
            .insert(identifier.to_owned(), (*line, expansion.to_owned()))
            .is_some()
        {
            return Err(compile_error(
                *line,
                format!("DEFINE identifier {identifier} is already defined"),
            ));
        }
    }
    let executable_per_line =
        segments
            .iter()
            .fold(BTreeMap::new(), |mut counts, (line, command)| {
                if !starts_with_keyword(command, "DEFINE") {
                    *counts.entry(*line).or_insert(0usize) += 1;
                }
                counts
            });
    let mut lines = Vec::new();
    for (line, command) in segments {
        if starts_with_keyword(command, "DEFINE") {
            continue;
        }
        let expanded = expand_command(command, line, &definitions, &mut Vec::new(), 0)?;
        if expanded.len() > MAX_MACRO_LINE_BYTES {
            return Err(compile_error(
                line,
                format!("expanded command exceeds {MAX_MACRO_LINE_BYTES} UTF-8 bytes"),
            ));
        }
        let delay_millis =
            parse_macro_delay_millis(&expanded).map_err(|message| compile_error(line, message))?;
        if delay_millis.is_some() && executable_per_line.get(&line).copied() != Some(1) {
            return Err(compile_error(
                line,
                "DELAY must occupy its own executable Macro line",
            ));
        }
        lines.push(CommandMacroCompiledLine {
            number: line,
            command: expanded,
            delay_millis,
        });
    }
    Ok(CommandMacroCompilation {
        lines,
        definitions: definitions
            .into_iter()
            .map(
                |(identifier, (line, expansion))| CommandMacroDefinitionExpansion {
                    line,
                    identifier,
                    expansion,
                },
            )
            .collect(),
    })
}

/// Parses a Macro-only delay expressed as non-negative seconds with up to millisecond precision.
/// Ordinary Programmer/Cue timing remains owned by the command parser.
pub fn parse_macro_delay_millis(command: &str) -> Result<Option<u64>, String> {
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    if !tokens
        .first()
        .is_some_and(|token| token.eq_ignore_ascii_case("DELAY"))
    {
        return Ok(None);
    }
    if tokens.len() != 2 {
        return Err("DELAY requires exactly one duration in seconds".into());
    }
    let value = tokens[1];
    if value.starts_with('-') {
        return Err("DELAY duration must be non-negative".into());
    }
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || (value.contains('.') && fraction.is_empty())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 3
    {
        return Err("DELAY duration must be seconds with at most three decimal places".into());
    }
    let seconds = whole
        .parse::<u64>()
        .map_err(|_| "DELAY duration is too large".to_owned())?;
    let fractional_millis = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u64>().unwrap_or(0) * 100,
        2 => fraction.parse::<u64>().unwrap_or(0) * 10,
        _ => fraction.parse::<u64>().unwrap_or(0),
    };
    seconds
        .checked_mul(1_000)
        .and_then(|millis| millis.checked_add(fractional_millis))
        .map(Some)
        .ok_or_else(|| "DELAY duration is too large".to_owned())
}

fn starts_with_keyword(command: &str, keyword: &str) -> bool {
    command
        .split_whitespace()
        .next()
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(keyword))
}

fn valid_define_identifier(identifier: &str) -> bool {
    identifier.starts_with('_')
        && identifier.len() > 1
        && identifier
            .bytes()
            .skip(1)
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn expand_command(
    command: &str,
    line: usize,
    definitions: &BTreeMap<String, (usize, String)>,
    stack: &mut Vec<String>,
    depth: usize,
) -> Result<String, CommandMacroCompileError> {
    if depth > MAX_DEFINE_EXPANSION_DEPTH {
        return Err(compile_error(line, "DEFINE expansion is too deeply nested"));
    }
    let mut expanded = String::new();
    for token in command.split_whitespace() {
        let Some((_, replacement)) = definitions.get(token) else {
            if token.starts_with('_') {
                return Err(compile_error(
                    line,
                    format!("DEFINE identifier {token} is not defined"),
                ));
            }
            append_expansion_token(&mut expanded, token, line)?;
            continue;
        };
        if stack.iter().any(|identifier| identifier == token) {
            return Err(compile_error(
                line,
                format!("DEFINE expansion cycle includes {token}"),
            ));
        }
        stack.push(token.to_owned());
        let replacement = expand_command(replacement, line, definitions, stack, depth + 1)?;
        stack.pop();
        append_expansion_token(&mut expanded, &replacement, line)?;
    }
    Ok(expanded)
}

fn append_expansion_token(
    expanded: &mut String,
    token: &str,
    line: usize,
) -> Result<(), CommandMacroCompileError> {
    let separator = usize::from(!expanded.is_empty());
    if expanded.len() + separator + token.len() > MAX_MACRO_LINE_BYTES {
        return Err(compile_error(
            line,
            format!("expanded command exceeds {MAX_MACRO_LINE_BYTES} UTF-8 bytes"),
        ));
    }
    if separator == 1 {
        expanded.push(' ');
    }
    expanded.push_str(token);
    Ok(())
}

fn compile_error(line: usize, message: impl Into<String>) -> CommandMacroCompileError {
    CommandMacroCompileError {
        line,
        message: message.into(),
    }
}

fn valid_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(source: &str) -> CommandMacroDefinition {
        CommandMacroDefinition {
            id: Uuid::new_v4(),
            number: 7,
            name: "House open".into(),
            source: source.into(),
            presentation: MacroPresentation {
                color: Some("#7f1d1d".into()),
                icon: None,
            },
        }
    }

    #[test]
    fn executable_lines_preserve_source_numbers_and_order() {
        let macro_definition =
            definition("# prepare\n\nGROUP 1 AT 50\n  // no operation\nRECORD + GROUP 10\n");
        let lines = macro_definition
            .lines()
            .map(|line| (line.number, line.command))
            .collect::<Vec<_>>();
        assert_eq!(lines, vec![(3, "GROUP 1 AT 50"), (5, "RECORD + GROUP 10")]);
    }

    #[test]
    fn semicolons_and_newlines_share_source_line_identity() {
        let macro_definition = definition("FIXTURE 1; AT 50\nFIXTURE 2");
        let lines = macro_definition
            .lines()
            .map(|line| (line.number, line.command))
            .collect::<Vec<_>>();
        assert_eq!(
            lines,
            vec![(1, "FIXTURE 1"), (1, "AT 50"), (2, "FIXTURE 2")]
        );
    }

    #[test]
    fn semicolons_do_not_bypass_the_physical_source_line_limit() {
        let mut macro_definition = definition("FIXTURE 1");
        macro_definition.source = format!("{};{}", "F".repeat(8_192), "F".repeat(8_192));
        assert!(macro_definition.validate().is_err());
    }

    #[test]
    fn define_requires_underscore_and_expands_through_the_document() {
        let compiled =
            compile_macro_source("DEFINE _front FIXTURE 1 THRU 4\n_front AT 50; RESTORE SELECTION")
                .unwrap();
        assert_eq!(
            compiled.lines,
            vec![
                CommandMacroCompiledLine {
                    number: 2,
                    command: "FIXTURE 1 THRU 4 AT 50".into(),
                    delay_millis: None,
                },
                CommandMacroCompiledLine {
                    number: 2,
                    command: RESTORE_SELECTION_COMMAND.into(),
                    delay_millis: None,
                },
            ]
        );
        assert_eq!(compiled.definitions[0].identifier, "_front");
        assert!(compile_macro_source("DEFINE front FIXTURE 1\nfront").is_err());
        assert!(compile_macro_source("_missing").is_err());
    }

    #[test]
    fn define_cycles_are_rejected() {
        let error = compile_macro_source("DEFINE _a _b\nDEFINE _b _a\n_a").unwrap_err();
        assert!(error.message.contains("cycle"));
    }

    #[test]
    fn macro_delay_uses_non_negative_seconds_with_millisecond_precision() {
        assert_eq!(parse_macro_delay_millis("FIXTURE 1").unwrap(), None);
        assert_eq!(parse_macro_delay_millis("DELAY 0").unwrap(), Some(0));
        assert_eq!(parse_macro_delay_millis("delay 1.5").unwrap(), Some(1_500));
        assert_eq!(parse_macro_delay_millis("DELAY 0.025").unwrap(), Some(25));

        for source in [
            "DELAY",
            "DELAY -1",
            "DELAY 1s",
            "DELAY 1.",
            "DELAY .5",
            "DELAY 1.0001",
            "DELAY 1 2",
        ] {
            assert!(
                parse_macro_delay_millis(source).is_err(),
                "{source} must be rejected"
            );
        }
    }

    #[test]
    fn macro_delay_must_occupy_its_own_executable_source_line() {
        let compiled = compile_macro_source("# note\nDELAY 1.25\n// note\nFIXTURE 1").unwrap();
        assert_eq!(compiled.lines[0].delay_millis, Some(1_250));
        assert_eq!(compiled.lines[1].delay_millis, None);

        let error = compile_macro_source("DELAY 1; FIXTURE 1").unwrap_err();
        assert_eq!(error.line, 1);
        assert_eq!(
            error.message,
            "DELAY must occupy its own executable Macro line"
        );
    }

    #[test]
    fn comment_markers_ignore_every_later_semicolon_statement_on_the_line() {
        let compiled =
            compile_macro_source("# note; FIXTURE 1 AT 50\nFIXTURE 2; // stop; AT 90\nAT 20")
                .unwrap();
        assert_eq!(
            compiled
                .lines
                .iter()
                .map(|line| line.command.as_str())
                .collect::<Vec<_>>(),
            ["FIXTURE 2", "AT 20"]
        );
    }

    #[test]
    fn branching_define_expansion_stops_at_the_line_budget() {
        let mut source = String::from("DEFINE _leaf FIXTURE 1\n");
        for level in 1..=8 {
            let previous = if level == 1 {
                "_leaf".to_owned()
            } else {
                format!("_level{}", level - 1)
            };
            source.push_str(&format!(
                "DEFINE _level{level} {previous} {previous} {previous} {previous}\n"
            ));
        }
        source.push_str("_level8");
        let error = compile_macro_source(&source).unwrap_err();
        assert!(error.message.contains("expanded command exceeds"));
    }

    #[test]
    fn definition_rejects_invalid_identity_number_name_and_presentation() {
        let mut macro_definition = definition("GROUP 1");
        macro_definition.id = Uuid::nil();
        assert_eq!(
            macro_definition.validate().unwrap_err(),
            "Macro id cannot be nil"
        );

        macro_definition.id = Uuid::new_v4();
        macro_definition.number = 0;
        assert!(macro_definition.validate().unwrap_err().contains("number"));

        macro_definition.number = 1;
        macro_definition.name = " ".into();
        assert!(macro_definition.validate().unwrap_err().contains("name"));

        macro_definition.name = "Valid".into();
        macro_definition.presentation.color = Some("red".into());
        assert!(macro_definition.validate().unwrap_err().contains("#RRGGBB"));
    }
}
