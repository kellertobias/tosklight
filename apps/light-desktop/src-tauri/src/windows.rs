use serde::Serialize;
use tauri::Manager;
use tauri::utils::config::BackgroundThrottlingPolicy;

#[derive(Serialize)]
pub(crate) struct ConsoleDisplay {
    id: String,
    name: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WindowBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn monitor_id(monitor: &tauri::window::Monitor) -> String {
    let position = monitor.position();
    let size = monitor.size();
    format!(
        "{}|{},{}|{}x{}",
        monitor.name().map(String::as_str).unwrap_or("Display"),
        position.x,
        position.y,
        size.width,
        size.height
    )
}

fn window_bounds(value: &serde_json::Value) -> Option<WindowBounds> {
    Some(WindowBounds {
        x: value.get("x")?.as_f64()?,
        y: value.get("y")?.as_f64()?,
        width: value.get("width")?.as_f64()?.max(640.0),
        height: value.get("height")?.as_f64()?.max(480.0),
    })
}

#[tauri::command]
pub(crate) fn list_console_displays(app: tauri::AppHandle) -> Result<Vec<ConsoleDisplay>, String> {
    app.available_monitors()
        .map_err(|error| error.to_string())
        .map(|items| {
            items
                .into_iter()
                .map(|monitor| ConsoleDisplay {
                    id: monitor_id(&monitor),
                    name: monitor.name().cloned().unwrap_or_else(|| "Display".into()),
                })
                .collect()
        })
}

#[tauri::command]
pub(crate) fn close_console_screen(app: tauri::AppHandle, screen_id: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&format!("screen-{screen_id}")) {
        window.close().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn hide_console_screen(app: tauri::AppHandle, screen_id: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&format!("screen-{screen_id}")) {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn open_stage_view_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("stage-view") {
        if !window.is_visible().map_err(|error| error.to_string())? {
            window.show().map_err(|error| error.to_string())?;
        }
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        "stage-view",
        tauri::WebviewUrl::App("index.html?stage-view=1".into()),
    )
    .title("Stage View")
    .inner_size(1000.0, 720.0)
    .resizable(true)
    .background_throttling(BackgroundThrottlingPolicy::Disabled)
    .build()
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn open_console_screen(
    app: tauri::AppHandle,
    screen_id: String,
    title: String,
    display_id: Option<String>,
    bounds: Option<serde_json::Value>,
    fullscreen: bool,
) -> Result<(), String> {
    let label = format!("screen-{screen_id}");
    if let Some(window) = app.get_webview_window(&label) {
        if !window.is_visible().map_err(|error| error.to_string())? {
            window.show().map_err(|error| error.to_string())?;
        }
        return Ok(());
    }
    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let monitor = display_id
        .as_ref()
        .and_then(|id| monitors.iter().find(|monitor| monitor_id(monitor) == *id));
    if display_id.is_some() && monitor.is_none() {
        return Ok(());
    }
    let mut builder = tauri::WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::App(format!("index.html?screen={screen_id}").into()),
    )
    .title(title)
    .inner_size(1200.0, 800.0)
    .resizable(true)
    .background_throttling(BackgroundThrottlingPolicy::Disabled)
    .decorations(false);
    if let Some(value) = bounds {
        if let Some(bounds) = window_bounds(&value) {
            builder = builder
                .position(bounds.x, bounds.y)
                .inner_size(bounds.width, bounds.height);
        }
    } else if let Some(monitor) = monitor {
        let position = monitor.position();
        builder = builder.position(f64::from(position.x) + 20.0, f64::from(position.y) + 20.0);
    }
    builder
        .fullscreen(fullscreen)
        .build()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{WindowBounds, window_bounds};
    use serde_json::json;

    #[test]
    fn console_bounds_keep_position_and_enforce_the_existing_minimum_size() {
        assert_eq!(
            window_bounds(&json!({"x": 12, "y": -4, "width": 320, "height": 200})),
            Some(WindowBounds {
                x: 12.0,
                y: -4.0,
                width: 640.0,
                height: 480.0,
            })
        );
    }

    #[test]
    fn incomplete_console_bounds_are_ignored() {
        assert_eq!(
            window_bounds(&json!({"x": 12, "y": -4, "width": 800})),
            None
        );
    }
}
