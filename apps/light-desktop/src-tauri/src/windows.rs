use serde::Serialize;
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{LogicalPosition, LogicalSize, Manager};

#[tauri::command]
pub(crate) fn current_window_fullscreen(window: tauri::Window) -> Result<bool, String> {
    window.is_fullscreen().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn set_current_window_fullscreen(
    window: tauri::Window,
    fullscreen: bool,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let monitor = if fullscreen {
        window
            .current_monitor()
            .map_err(|error| error.to_string())?
    } else {
        None
    };

    window
        .set_fullscreen(fullscreen)
        .map_err(|error| error.to_string())?;

    // Tauri queues the fullscreen request on Windows. Explicitly apply the
    // current monitor bounds to the native window as well, so the child webview
    // has a full-size parent even if the queued transition leaves old bounds.
    #[cfg(target_os = "windows")]
    if let Some(monitor) = monitor {
        window
            .set_position(*monitor.position())
            .map_err(|error| error.to_string())?;
        window
            .set_size(*monitor.size())
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

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

/// Where a screen window finds the desk it joins; read by the web interface of that window.
const SCREEN_ATTACHMENT_KEY: &str = "light.screen-attachment";

/// A script that hands a screen window the desk server and session of the window that opened it.
///
/// A screen window is a second webview of this application with its own, initially empty session
/// storage. Without this it would look for a server and a session of its own and could end up on
/// a different server, or with no session at all, instead of joining the open desk.
///
/// `keep_existing` is for the script that runs on every page load: a reload must not bring back
/// the attachment the window was created with once the desk has handed over a newer one.
fn attachment_script(attachment: &serde_json::Value, keep_existing: bool) -> Option<String> {
    let valid = attachment
        .get("server_url")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|url| url.starts_with("http://") || url.starts_with("https://"))
        && attachment
            .get("session")
            .and_then(|session| session.get("token"))
            .is_some_and(serde_json::Value::is_string);
    if !valid {
        return None;
    }
    let key = serde_json::to_string(SCREEN_ATTACHMENT_KEY).ok()?;
    let value = serde_json::to_string(&serde_json::to_string(attachment).ok()?).ok()?;
    let guard = if keep_existing {
        "sessionStorage.getItem(k)===null"
    } else {
        "true"
    };
    Some(format!(
        "(()=>{{try{{const k={key};if({guard})sessionStorage.setItem(k,{value})}}catch(_){{}}}})();"
    ))
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
    if let Some(window) = app.get_window(&format!("screen-{screen_id}")) {
        window.close().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn hide_console_screen(app: tauri::AppHandle, screen_id: String) -> Result<(), String> {
    if let Some(window) = app.get_window(&format!("screen-{screen_id}")) {
        window.hide().map_err(|error| error.to_string())?;
    }
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
    attachment: Option<serde_json::Value>,
) -> Result<(), String> {
    let label = format!("screen-{screen_id}");
    if let Some(window) = app.get_window(&label) {
        // The desk reconnected or changed server: hand the open screen the current attachment.
        if let (Some(webview), Some(script)) = (
            app.get_webview(&label),
            attachment
                .as_ref()
                .and_then(|value| attachment_script(value, false)),
        ) {
            webview.eval(script).map_err(|error| error.to_string())?;
        }
        window
            .set_title(&title)
            .map_err(|error| error.to_string())?;
        if window.is_fullscreen().map_err(|error| error.to_string())? {
            window
                .set_fullscreen(false)
                .map_err(|error| error.to_string())?;
        }
        if let Some(value) = bounds {
            if let Some(bounds) = window_bounds(&value) {
                window
                    .set_position(LogicalPosition::new(bounds.x, bounds.y))
                    .map_err(|error| error.to_string())?;
                window
                    .set_size(LogicalSize::new(bounds.width, bounds.height))
                    .map_err(|error| error.to_string())?;
            }
        } else if let Some(monitor) = app
            .available_monitors()
            .map_err(|error| error.to_string())?
            .iter()
            .find(|monitor| {
                display_id
                    .as_ref()
                    .is_some_and(|id| monitor_id(monitor) == *id)
            })
        {
            let position = monitor.position();
            window
                .set_position(LogicalPosition::new(
                    f64::from(position.x) + 20.0,
                    f64::from(position.y) + 20.0,
                ))
                .map_err(|error| error.to_string())?;
        }
        window
            .set_fullscreen(fullscreen)
            .map_err(|error| error.to_string())?;
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
    let mut builder = tauri::window::WindowBuilder::new(&app, &label)
        .title(title)
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .background_color(tauri::window::Color(0x07, 0x09, 0x0c, 0xff))
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
    let window = builder
        .fullscreen(fullscreen)
        .build()
        .map_err(|error| error.to_string())?;
    let size = window.inner_size().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let logical = size.to_logical::<f64>(scale);
    let mut webview_builder = crate::portable::place_webview(
        tauri::webview::WebviewBuilder::new(
            &label,
            tauri::WebviewUrl::App(format!("index.html?screen={screen_id}").into()),
        )
        .transparent(true)
        .background_throttling(BackgroundThrottlingPolicy::Disabled)
        .auto_resize(),
    );
    if let Some(script) = attachment
        .as_ref()
        .and_then(|value| attachment_script(value, true))
    {
        webview_builder = webview_builder.initialization_script(script);
    }
    let webview = window
        .add_child(
            webview_builder,
            LogicalPosition::new(0.0, 0.0),
            LogicalSize::new(logical.width, logical.height),
        )
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    let _ = webview.with_webview(|webview| {
        viz_surface::raise_view_above_siblings(webview.inner());
    });
    #[cfg(not(target_os = "macos"))]
    let _ = webview;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{WindowBounds, attachment_script, window_bounds};
    use serde_json::json;

    fn attachment() -> serde_json::Value {
        json!({
            "server_url": "http://127.0.0.1:5471",
            "session": {"session_id": "s", "client_id": "c", "token": "t</script>\u{2028}", "desk": {"id": "d"}},
            "desk_token": null
        })
    }

    #[test]
    fn screen_attachment_script_stores_the_exact_attachment_as_one_string_literal() {
        let script = attachment_script(&attachment(), false).expect("valid attachment");
        let literal = script
            .split("sessionStorage.setItem(k,")
            .nth(1)
            .and_then(|rest| rest.strip_suffix(")}catch(_){}})();"))
            .expect("one stored literal");
        let stored: String = serde_json::from_str(literal).expect("a JSON string literal");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&stored).unwrap(),
            attachment()
        );
        assert!(script.contains("const k=\"light.screen-attachment\";if(true)"));
    }

    #[test]
    fn screen_attachment_script_on_page_load_keeps_a_newer_handed_over_attachment() {
        let script = attachment_script(&attachment(), true).expect("valid attachment");
        assert!(script.contains("if(sessionStorage.getItem(k)===null)"));
    }

    #[test]
    fn screen_attachment_script_rejects_attachments_without_a_server_or_session() {
        assert_eq!(attachment_script(&json!({}), true), None);
        assert_eq!(
            attachment_script(
                &json!({"server_url": "javascript:alert(1)", "session": {"token": "t"}}),
                true
            ),
            None
        );
        assert_eq!(
            attachment_script(&json!({"server_url": "http://desk", "session": {}}), true),
            None
        );
    }

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
