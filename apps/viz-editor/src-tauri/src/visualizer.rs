use parking_lot::Mutex;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};

const VISUALIZER_PATH_ENV: &str = "TOSKLIGHT_VIZ_RENDERER";

/// The editor-owned scene endpoint and the one renderer launched for it.
///
/// The listener is bound before this state is constructed, so a renderer can never observe the
/// old pick-a-port/release-it race. Keeping the child handle also makes repeated Open Viz clicks
/// idempotent instead of leaving renderer processes behind.
pub struct VisualizerLauncher {
    address: SocketAddr,
    child: Mutex<Option<Child>>,
}

pub type RendererSettings = viz_scene::RendererSettings;

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
    let binary = visualizer_binary()?;
    let mut current = launcher.child.lock();
    if let Some(child) = current.as_mut() {
        if reuse_running_child(child.try_wait())? {
            return Ok(());
        }
        *current = None;
    }
    let arguments = visualizer_arguments(launcher.address);
    let child = visualizer_command(&binary, &arguments)
        .spawn()
        .map_err(|error| format!("could not open {}: {error}", binary.display()))?;
    *current = Some(child);
    Ok(())
}

#[tauri::command]
pub fn visualizer_is_running(
    launcher: tauri::State<'_, VisualizerLauncher>,
) -> Result<bool, String> {
    let mut current = launcher.child.lock();
    let Some(child) = current.as_mut() else {
        return Ok(false);
    };
    if reuse_running_child(child.try_wait())? {
        return Ok(true);
    }
    *current = None;
    Ok(false)
}

#[tauri::command]
pub fn renderer_settings(
    session: tauri::State<'_, crate::session::Session>,
) -> Result<RendererSettings, String> {
    if let Some(update) = session.scene_source().renderer_settings() {
        return Ok(update.settings);
    }
    let settings = restored_renderer_settings()?;
    let _ = session
        .scene_source()
        .set_renderer_settings("editor-startup", settings.clone())?;
    Ok(settings)
}

pub fn restored_renderer_settings() -> Result<RendererSettings, String> {
    let path = preferences_path()?;
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(RendererSettings::from_file(&text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(RendererSettings::default())
        }
        Err(error) => Err(format!(
            "could not read Visualizer settings from {}: {error}",
            path.display()
        )),
    }
}

#[tauri::command]
pub fn save_renderer_settings(
    session: tauri::State<'_, crate::session::Session>,
    settings: RendererSettings,
) -> Result<RendererSettings, String> {
    settings.validate()?;
    let path = preferences_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "could not create the Visualizer settings folder {}: {error}",
                parent.display()
            )
        })?;
    }
    std::fs::write(&path, settings.to_file()).map_err(|error| {
        format!(
            "could not save Visualizer settings to {}: {error}",
            path.display()
        )
    })?;
    session
        .scene_source()
        .set_renderer_settings("editor", settings.clone())?;
    Ok(settings)
}

fn visualizer_command(binary: &std::path::Path, arguments: &[String]) -> Command {
    let mut command = Command::new(binary);
    command
        .args(arguments)
        // This renderer is another window of the editor-owned PreViz application. On macOS the
        // child uses an accessory activation policy so it cannot add a second Dock/App Switcher
        // identity beside the editor.
        .env("TOSKLIGHT_VIZ_LAUNCHED_BY", "editor")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn reuse_running_child(status: std::io::Result<Option<ExitStatus>>) -> Result<bool, String> {
    status
        .map(|status| status.is_none())
        .map_err(|error| format!("could not inspect the visualizer process: {error}"))
}

fn visualizer_arguments(address: SocketAddr) -> Vec<String> {
    let host = match address.ip() {
        IpAddr::V4(ip) if ip == Ipv4Addr::UNSPECIFIED => Ipv4Addr::LOCALHOST.to_string(),
        ip => ip.to_string(),
    };
    let mut arguments = vec![
        "--planning-server".to_owned(),
        host,
        "--port".to_owned(),
        address.port().to_string(),
    ];
    if let Ok(path) = preferences_path() {
        arguments.push("--preferences".into());
        arguments.push(path.display().to_string());
    }
    arguments
}

fn preferences_path() -> Result<PathBuf, String> {
    if let Some(path) =
        std::env::var_os("TOSKLIGHT_VIZ_PREFERENCES").filter(|value| !value.is_empty())
    {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| {
            "could not locate the operator's home folder for Visualizer settings".to_owned()
        })?;
    if cfg!(target_os = "macos") {
        return Ok(home.join("Library/Application Support/ToskLight/Visualizer/preferences.conf"));
    }
    if cfg!(windows) {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|data| data.join("ToskLight/Visualizer/preferences.conf"))
            .ok_or_else(|| "could not locate APPDATA for Visualizer settings".to_owned());
    }
    Ok(std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
        .join("tosklight/visualizer/preferences.conf"))
}

fn visualizer_binary() -> Result<PathBuf, String> {
    if let Some(named) = std::env::var_os(VISUALIZER_PATH_ENV).filter(|value| !value.is_empty()) {
        return existing(PathBuf::from(named));
    }
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate the rig editor binary: {error}"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| "the rig editor binary has no directory".to_owned())?;
    // Only the renderer's own file name. The bundle's executable is the editor and carries the
    // product name, so searching for that name here would find this binary and launch it again.
    // The older bundles named their executable after the renderer, which is why those names stay
    // discoverable for an installation that has not been replaced yet.
    let names: &[&str] = if cfg!(windows) {
        &["viz-renderer.exe", "ToskLight PreViz.exe"]
    } else {
        &["viz-renderer", "ToskLight PreViz"]
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

fn existing(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "{VISUALIZER_PATH_ENV} names {}, which is not a file",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RendererSettings, existing, reuse_running_child, visualizer_arguments, visualizer_command,
    };
    use std::net::{Ipv4Addr, SocketAddr};
    use std::path::Path;

    #[test]
    fn configured_visualizer_must_be_a_file() {
        let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("missing-viz-renderer");
        assert!(existing(missing).unwrap_err().contains("not a file"));
    }

    #[test]
    fn editor_launch_names_its_live_planning_endpoint_and_never_the_show_file() {
        let arguments = visualizer_arguments(SocketAddr::from((Ipv4Addr::LOCALHOST, 5311)));
        assert_eq!(
            &arguments[..4],
            ["--planning-server", "127.0.0.1", "--port", "5311"]
        );
        assert!(!arguments.iter().any(|argument| argument == "--show"));
        assert!(arguments.iter().any(|argument| argument == "--preferences"));
    }

    #[test]
    fn repeated_open_reuses_the_renderer_that_is_still_running() {
        assert!(reuse_running_child(Ok(None)).unwrap());
    }

    #[test]
    fn editor_marks_its_renderer_as_the_accessory_window_of_previz() {
        let command = visualizer_command(Path::new("viz-renderer"), &[]);
        assert!(command.get_envs().any(|(name, value)| {
            name == "TOSKLIGHT_VIZ_LAUNCHED_BY" && value.is_some_and(|value| value == "editor")
        }));
    }

    #[test]
    fn child_inspection_failure_is_reported_as_an_editor_owned_launch_error() {
        let error =
            reuse_running_child(Err(std::io::Error::other("process unavailable"))).unwrap_err();
        assert_eq!(
            error,
            "could not inspect the visualizer process: process unavailable"
        );
    }

    #[test]
    fn editor_settings_round_trip_the_renderer_preference_contract() {
        let source = "source lighting_desk\nhost desk.local\nport 5001\nuser Tobias\nquality ultra\nfog 0.08\npersistence 0.12\npersistence_falloff 4\nambient 0.09\nexposure 1.2\nlaser_brightness 1.5\nlamp_fog_cloudiness 0.2\nlamp_fog_turbulence 0.3\nlaser_fog_cloudiness 0.4\nlaser_fog_turbulence 0.5\ncrowd_amount 0.75\ntheme dark_on_light\nbackground 0.1,0.2,0.3\nlabels false\nshow_selection true\nfloor_grid false\nblender /Applications/Blender.app\ninput 2 sacn\n";
        let settings = RendererSettings::from_file(source);
        settings.validate().unwrap();
        assert_eq!(RendererSettings::from_file(&settings.to_file()), settings);
    }

    #[test]
    fn editor_settings_reject_out_of_range_renderer_values() {
        let settings = RendererSettings {
            exposure: 9.0,
            ..RendererSettings::default()
        };
        assert_eq!(
            settings.validate().unwrap_err(),
            "Exposure must be from 0.05 to 4"
        );
    }
}
