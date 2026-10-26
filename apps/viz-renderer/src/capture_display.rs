//! Explicit local capture placement; ordinary renderer launches keep their existing behavior.
use winit::{
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

pub(crate) fn create(
    event_loop: &ActiveEventLoop,
    attributes: WindowAttributes,
) -> Result<Window, String> {
    let requested = std::env::var("TOSKLIGHT_VIZ_CAPTURE_DISPLAY").ok();
    if requested.is_none() {
        return event_loop
            .create_window(attributes)
            .map_err(|error| error.to_string());
    }
    if requested.as_deref() != Some("internal") {
        return Err("TOSKLIGHT_VIZ_CAPTURE_DISPLAY only supports internal".into());
    }
    #[cfg(target_os = "macos")]
    {
        use core_graphics::display::CGDisplay;
        use winit::{
            dpi::{LogicalPosition, LogicalSize},
            platform::macos::MonitorHandleExtMacOS,
        };
        let displays: Vec<_> = CGDisplay::active_displays()
            .map_err(|_| "Cannot enumerate capture displays")?
            .into_iter()
            .map(CGDisplay::new)
            .filter(CGDisplay::is_builtin)
            .collect();
        if displays.len() != 1 {
            return Err("Exactly one internal capture display is required".into());
        }
        let display = displays[0];
        if display.is_in_mirror_set() {
            return Err("Internal capture display is mirrored; capture remains hidden".into());
        }
        let bounds = display.bounds();
        let width = 1440.0_f64.min(bounds.size.width - 80.0);
        let height = 900.0_f64.min(bounds.size.height - 100.0);
        if width < 640.0 || height < 480.0 {
            return Err("Internal display cannot fit the capture".into());
        }
        let window = event_loop
            .create_window(
                attributes
                    .with_visible(false)
                    .with_position(LogicalPosition::new(
                        bounds.origin.x + 40.0,
                        bounds.origin.y + 50.0,
                    ))
                    .with_inner_size(LogicalSize::new(width, height)),
            )
            .map_err(|error| error.to_string())?;
        let monitor = window
            .current_monitor()
            .ok_or("Capture monitor is unavailable")?;
        if monitor.native_id() != display.id {
            return Err(
                "Capture placement did not reach the internal display; window remains hidden"
                    .into(),
            );
        }
        let position = window.outer_position().map_err(|error| error.to_string())?;
        let size = window.outer_size();
        let scale = window.scale_factor();
        let (x, y) = (f64::from(position.x) / scale, f64::from(position.y) / scale);
        let (w, h) = (
            f64::from(size.width) / scale,
            f64::from(size.height) / scale,
        );
        if !scale.is_finite()
            || scale <= 0.0
            || x < bounds.origin.x
            || y < bounds.origin.y
            || x + w > bounds.origin.x + bounds.size.width
            || y + h > bounds.origin.y + bounds.size.height
        {
            return Err("Capture window is not wholly contained on the internal display; capture remains hidden".into());
        }
        eprintln!(
            "[local-capture-display] internal={} size={}x{} scale={}",
            display.id,
            width,
            height,
            window.scale_factor()
        );
        window.set_visible(true);
        Ok(window)
    }
    #[cfg(not(target_os = "macos"))]
    Err("Internal capture placement is supported only on macOS".into())
}
