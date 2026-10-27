//! Explicit private native validation. No show/defaults, endpoint or placement is inferred.
mod display;
#[cfg(test)]
mod tests;
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
use tauri::Manager;
const DATA: &str = "TOSKLIGHT_VIZ_CAPTURE_DATA_DIR";
const SHOW: &str = "TOSKLIGHT_VIZ_CAPTURE_SHOW";
const DISPLAY: &str = "TOSKLIGHT_VIZ_CAPTURE_DISPLAY";
const PREFERENCES: &str = "TOSKLIGHT_VIZ_PREFERENCES";
#[derive(Clone, Debug)]
pub(crate) struct Launch {
    pub data: PathBuf,
    pub show: PathBuf,
    pub preferences: PathBuf,
}
static CONFIG: OnceLock<Result<Option<Launch>, String>> = OnceLock::new();
pub(crate) fn configuration() -> Result<Option<&'static Launch>, String> {
    match CONFIG.get_or_init(|| {
        let read = |key| {
            std::env::var(key).map(Some).or_else(|e| match e {
                std::env::VarError::NotPresent => Ok(None),
                _ => Err(format!("{key} must be valid text")),
            })
        };
        let data = read(DATA)?;
        let show = read(SHOW)?;
        let display = read(DISPLAY)?;
        if data.is_none() && show.is_none() && display.is_none() {
            return Ok(None);
        }
        parse(
            data.as_deref(),
            show.as_deref(),
            display.as_deref(),
            read(PREFERENCES)?.as_deref(),
        )
    }) {
        Ok(value) => Ok(value.as_ref()),
        Err(error) => Err(error.clone()),
    }
}
fn existing_absolute(value: &str, label: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute"));
    }
    path.canonicalize()
        .map_err(|e| format!("Cannot resolve {label}: {e}"))
}
fn parse(
    data: Option<&str>,
    show: Option<&str>,
    display: Option<&str>,
    preferences: Option<&str>,
) -> Result<Option<Launch>, String> {
    // An ordinary renderer preference override alone does not opt the editor into capture mode.
    if data.is_none() && show.is_none() && display.is_none() {
        return Ok(None);
    }
    let (Some(data), Some(show), Some("internal"), Some(preferences)) =
        (data, show, display, preferences)
    else {
        return Err(format!(
            "Private validation requires {DATA}, {SHOW}, {DISPLAY}=internal and absolute {PREFERENCES}; window remains hidden"
        ));
    };
    let data = existing_absolute(data, DATA)?;
    if !data.is_dir() || !data.components().any(|p| p.as_os_str() == ".artifacts") {
        return Err(
            "Private data must be an existing owned directory beneath canonical .artifacts".into(),
        );
    }
    let show = existing_absolute(show, SHOW)?;
    let preferences = existing_absolute(preferences, PREFERENCES)?;
    for (path, label) in [(&show, SHOW), (&preferences, PREFERENCES)] {
        if !path.is_file() || !path.starts_with(&data) {
            return Err(format!(
                "{label} must be an existing file inside private {DATA}"
            ));
        }
    }
    if show == preferences {
        return Err("Show and renderer preferences must be different files".into());
    }
    Ok(Some(Launch {
        data,
        show,
        preferences,
    }))
}
/// Runs before Tauri creates even its initially hidden webview.
pub(crate) fn configure(config: &mut tauri::utils::config::Config) -> Result<(), String> {
    if let Some(launch) = configuration()? {
        write_readiness(launch, false)?;
        for window in &mut config.app.windows {
            window.visible = false;
            window.focus = false;
            window.incognito = true;
            // WKWebView uses incognito/nonpersistent storage; supported other runtimes use this path.
            window.data_directory = Some(launch.data.join("webview"));
        }
    }
    Ok(())
}
pub(crate) fn open_private(
    session: &crate::session::Session,
    launch: &Launch,
) -> Result<(), String> {
    // Never consult global recent files or persisted sync bindings in this validation process.
    session.set_recent_store(crate::recent::RecentShow::at(
        launch.data.join("recent-show"),
    ));
    let summary = session.open(&launch.show)?;
    if Path::new(&summary.path)
        .canonicalize()
        .map_err(|e| e.to_string())?
        != launch.show
    {
        return Err(
            "Private validation opened an unexpected document; window remains hidden".into(),
        );
    }
    Ok(())
}
pub(crate) fn show_private(app: &tauri::App, launch: &Launch) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Private main window is missing")?;
    if window.is_visible().map_err(|e| e.to_string())? {
        return Err("Private window was shown before internal placement".into());
    }
    let app = app.handle().clone();
    let launch = launch.clone();
    tauri::async_runtime::spawn(async move {
        let result = display::place(&window)
            .await
            .and_then(|()| window.show().map_err(|e| e.to_string()));
        if let Err(error) = result {
            let _ = window.hide();
            eprintln!("private Architect validation failed: {error}");
            app.exit(1);
            return;
        }
        if let Err(error) = write_readiness(&launch, true) {
            let _ = window.hide();
            eprintln!("private Architect receipt failed: {error}");
            app.exit(1);
        }
    });
    Ok(())
}

fn readiness(launch: &Launch, shown: bool) -> serde_json::Value {
    serde_json::json!({"pid":std::process::id(),"show":launch.show,"preferences":launch.preferences,
        "display":"internal","windowPlacedAndShown":shown,"frontendReadinessClaimed":false})
}
fn write_readiness(launch: &Launch, shown: bool) -> Result<(), String> {
    let temporary = launch
        .data
        .join(format!("capture-window-{}.json.tmp", std::process::id()));
    std::fs::write(&temporary, readiness(launch, shown).to_string()).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, launch.data.join("capture-window.json")).map_err(|e| e.to_string())
}
