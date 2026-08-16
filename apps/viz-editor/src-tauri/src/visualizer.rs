//! Launch the renderer as another window of this editor-owned PreViz session.

use parking_lot::Mutex;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub struct VisualizerLauncher {
    address: SocketAddr,
    child: Mutex<Option<Child>>,
}

impl VisualizerLauncher {
    pub fn new(address: SocketAddr) -> Self {
        Self {
            address,
            child: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn open_visualizer(launcher: tauri::State<'_, VisualizerLauncher>) -> Result<(), String> {
    let mut child = launcher.child.lock();
    if let Some(current) = child.as_mut() {
        if current
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_none()
        {
            return Ok(());
        }
        *child = None;
    }
    let binary = visualizer_binary()?;
    let host = match launcher.address.ip() {
        IpAddr::V4(ip) if ip == Ipv4Addr::UNSPECIFIED => Ipv4Addr::LOCALHOST.to_string(),
        ip => ip.to_string(),
    };
    *child = Some(
        Command::new(&binary)
            .args([
                "--planning-server",
                &host,
                "--port",
                &launcher.address.port().to_string(),
            ])
            .env("TOSKLIGHT_VIZ_LAUNCHED_BY", "editor")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not open {}: {error}", binary.display()))?,
    );
    Ok(())
}

fn visualizer_binary() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("TOSKLIGHT_VIZ_RENDERER").filter(|value| !value.is_empty())
    {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path.clone()).ok_or_else(|| {
            format!(
                "TOSKLIGHT_VIZ_RENDERER names {}, which is not a file",
                path.display()
            )
        });
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = executable
        .parent()
        .ok_or_else(|| "the rig editor binary has no directory".to_owned())?;
    let names: &[&str] = if cfg!(windows) {
        &["ToskLight PreViz.exe", "viz-renderer.exe"]
    } else {
        &["ToskLight PreViz", "viz-renderer"]
    };
    names
        .iter()
        .map(|name| directory.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            format!(
                "the visualizer output is not installed beside {}",
                executable.display()
            )
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn launched_renderer_reads_the_editor_planning_source() {
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], 5311));
        assert_eq!(address.ip().to_string(), "127.0.0.1");
        assert_eq!(address.port(), 5311);
    }
}
