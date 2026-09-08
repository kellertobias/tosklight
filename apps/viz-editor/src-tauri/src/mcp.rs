//! Operator-facing MCP client configuration for this exact Architect installation.

use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::Manager;

const SERVER_NAME: &str = "tosklight-patch-mcp.mjs";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfiguration {
    application_path: String,
    server_path: String,
    codex_command: String,
    json_configuration: String,
}

/// Return copy-ready configuration for the MCP bridge shipped with this running application.
#[tauri::command]
pub fn mcp_configuration(app: tauri::AppHandle) -> Result<McpConfiguration, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate this Architect application: {error}"))?;
    let application = application_path(&executable);
    let server = configured_server_path()
        .or_else(|| {
            app.path()
                .resource_dir()
                .ok()
                .map(|resources| resources.join(SERVER_NAME))
                .filter(|path| path.is_file())
        })
        .or_else(|| {
            executable
                .parent()
                .map(|directory| directory.join(SERVER_NAME))
                .filter(|path| path.is_file())
        })
        .ok_or_else(|| {
            format!(
                "this Architect installation does not contain its MCP bridge beside {}",
                executable.display()
            )
        })?;
    let server_text = server.display().to_string();
    let command_path = shell_quoted(&server_text);
    let json_configuration = serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "tosklight_architect": {
                "command": "node",
                "args": [server_text],
                "env": { "TOSKLIGHT_TARGET": "architect" }
            }
        }
    }))
    .map_err(|error| format!("could not format MCP configuration: {error}"))?;
    Ok(McpConfiguration {
        application_path: application.display().to_string(),
        server_path: server.display().to_string(),
        codex_command: format!(
            "codex mcp add tosklight_architect --env TOSKLIGHT_TARGET=architect -- node {command_path}"
        ),
        json_configuration,
    })
}

fn configured_server_path() -> Option<PathBuf> {
    std::env::var_os("TOSKLIGHT_PATCH_MCP")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

fn application_path(executable: &Path) -> PathBuf {
    executable
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .unwrap_or(executable)
        .to_path_buf()
}

fn shell_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_bundle_path_is_reported_as_the_application() {
        let executable =
            Path::new("/Applications/ToskLight Architect.app/Contents/MacOS/ToskLight Architect");
        assert_eq!(
            application_path(executable),
            Path::new("/Applications/ToskLight Architect.app")
        );
    }

    #[test]
    fn copied_commands_quote_real_paths_with_spaces() {
        assert_eq!(
            shell_quoted("/Applications/ToskLight Architect.app/Contents/Resources/server.mjs"),
            "'/Applications/ToskLight Architect.app/Contents/Resources/server.mjs'"
        );
    }
}
