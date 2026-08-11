//! Show-owned command Macro definitions and document-level validation.
//!
//! A Macro is deliberately data, not executable installation code: every executable line is one
//! ordinary command-line command and is interpreted only by the authoritative Programming
//! service at run time.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_MACRO_NUMBER: u16 = 9_999;
pub const MAX_MACRO_NAME_BYTES: usize = 128;
pub const MAX_MACRO_SOURCE_BYTES: usize = 256 * 1024;
pub const MAX_MACRO_LINE_BYTES: usize = 16 * 1024;

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
        for line in self.lines() {
            if line.command.len() > MAX_MACRO_LINE_BYTES {
                return Err(format!(
                    "Macro line {} must not exceed {MAX_MACRO_LINE_BYTES} UTF-8 bytes",
                    line.number
                ));
            }
        }
        if let Some(color) = &self.presentation.color
            && !valid_color(color)
        {
            return Err("Macro color must be a #RRGGBB value".into());
        }
        Ok(())
    }

    pub fn lines(&self) -> impl Iterator<Item = CommandMacroLine<'_>> {
        self.source.lines().enumerate().filter_map(|(index, raw)| {
            let command = raw.trim();
            if command.is_empty() || command.starts_with('#') || command.starts_with("//") {
                return None;
            }
            Some(CommandMacroLine {
                number: index + 1,
                command,
            })
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandMacroLine<'a> {
    pub number: usize,
    pub command: &'a str,
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
