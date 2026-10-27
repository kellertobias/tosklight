//! Keep benchmark windows on the internal display before making them visible.
const DISPLAY_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_DISPLAY";

pub(super) async fn place(window: &tauri::WebviewWindow, fixed_pixels: bool) -> Result<(), String> {
    let requested = std::env::var(DISPLAY_ENV).ok();
    if requested
        .as_deref()
        .is_some_and(|value| value != "internal")
    {
        return Err(format!("{DISPLAY_ENV} only supports internal"));
    }
    #[cfg(target_os = "macos")]
    return place_internal(window, fixed_pixels).await;
    #[cfg(not(target_os = "macos"))]
    {
        if requested.is_some() {
            return Err("Internal display detection is supported only on macOS".into());
        }
        if fixed_pixels {
            window
                .set_min_size(None::<tauri::PhysicalSize<u32>>)
                .map_err(|error| error.to_string())?;
            window
                .set_size(tauri::PhysicalSize::new(1920, 1080))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
struct Placement {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Derive all geometry from the TARGET display's scale, including its global work-area origin.
fn target_placement(
    area: (i32, i32, u32, u32),
    target_scale: f64,
    current: (u32, u32),
    current_scale: f64,
    fixed: bool,
) -> Result<Placement, String> {
    if !target_scale.is_finite()
        || target_scale <= 0.0
        || !current_scale.is_finite()
        || current_scale <= 0.0
    {
        return Err("Invalid display scale; benchmark remains hidden".into());
    }
    let area_width = f64::from(area.2) / target_scale;
    let area_height = f64::from(area.3) / target_scale;
    let (width, height) = if fixed {
        (1920.0 / target_scale, 1080.0 / target_scale)
    } else {
        (
            (f64::from(current.0) / current_scale).min(area_width),
            (f64::from(current.1) / current_scale).min(area_height),
        )
    };
    if width <= 0.0 || height <= 0.0 || width > area_width || height > area_height {
        return Err("Internal display cannot fit the baseline; benchmark remains hidden".into());
    }
    Ok(Placement {
        x: f64::from(area.0) / target_scale + (area_width - width) / 2.0,
        y: f64::from(area.1) / target_scale + (area_height - height) / 2.0,
        width,
        height,
    })
}

#[cfg(target_os = "macos")]
async fn place_internal(window: &tauri::WebviewWindow, fixed_pixels: bool) -> Result<(), String> {
    if window.is_visible().map_err(|error| error.to_string())? {
        return Err(
            "Benchmark must remain hidden until internal display placement is verified".into(),
        );
    }
    let origin = builtin_origin()?;
    // Tauri's macOS monitor origins are CGDisplayBounds * that monitor's scale.
    let monitors = window
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let matching: Vec<_> = monitors
        .iter()
        .filter(|monitor| {
            let scale = monitor.scale_factor();
            monitor.position().x == (origin.0 * scale).round() as i32
                && monitor.position().y == (origin.1 * scale).round() as i32
        })
        .collect();
    if matching.len() != 1 {
        return Err("Cannot uniquely match the internal display; benchmark remains hidden".into());
    }
    let monitor = matching[0];
    let area = monitor.work_area();
    let current = window.inner_size().map_err(|error| error.to_string())?;
    let current_scale = window.scale_factor().map_err(|error| error.to_string())?;
    let target_scale = monitor.scale_factor();
    let target = target_placement(
        (
            area.position.x,
            area.position.y,
            area.size.width,
            area.size.height,
        ),
        target_scale,
        (current.width, current.height),
        current_scale,
        fixed_pixels,
    )?;
    window
        .set_fullscreen(false)
        .map_err(|error| error.to_string())?;
    window.unmaximize().map_err(|error| error.to_string())?;
    window
        .set_min_size(None::<tauri::LogicalSize<f64>>)
        .map_err(|error| error.to_string())?;
    // Physical coordinates are converted using the window's CURRENT screen in tao. Logical
    // coordinates avoid that conversion while moving between screens with different scales.
    // Move first, then request the size derived from the destination's physical-pixel budget.
    window
        .set_position(tauri::LogicalPosition::new(
            f64::from(area.position.x) / target_scale,
            f64::from(area.position.y) / target_scale,
        ))
        .map_err(|error| error.to_string())?;
    window
        .set_size(tauri::LogicalSize::new(target.width, target.height))
        .map_err(|error| error.to_string())?;
    window
        .set_position(tauri::LogicalPosition::new(target.x, target.y))
        .map_err(|error| error.to_string())?;
    // tao queues these AppKit changes asynchronously even when called on the main thread.
    // Yield rather than blocking that thread, and never show an unverified or spanning window.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if verified_internal(window, monitor, fixed_pixels)? {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(
                "Internal display placement did not settle; benchmark remains hidden".into(),
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[cfg(target_os = "macos")]
fn verified_internal(
    window: &tauri::WebviewWindow,
    target: &tauri::window::Monitor,
    fixed: bool,
) -> Result<bool, String> {
    if window.is_visible().map_err(|error| error.to_string())? {
        return Err("Benchmark became visible before internal placement was verified".into());
    }
    let Some(current) = window
        .current_monitor()
        .map_err(|error| error.to_string())?
    else {
        return Ok(false);
    };
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    if current.position() != target.position()
        || current.size() != target.size()
        || current.name() != target.name()
        || (current.scale_factor() - target.scale_factor()).abs() > 0.000_001
        || (scale - target.scale_factor()).abs() > 0.000_001
    {
        return Ok(false);
    }
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let size = window.outer_size().map_err(|error| error.to_string())?;
    let area = target.work_area();
    let contained = contains_rect(
        (
            area.position.x,
            area.position.y,
            area.size.width,
            area.size.height,
        ),
        (position.x, position.y, size.width, size.height),
    );
    let inner = window.inner_size().map_err(|error| error.to_string())?;
    Ok(contained && (!fixed || (inner.width == 1920 && inner.height == 1080)))
}

fn contains_rect(area: (i32, i32, u32, u32), rect: (i32, i32, u32, u32)) -> bool {
    rect.0 >= area.0
        && rect.1 >= area.1
        && rect.2 > 0
        && rect.3 > 0
        && i64::from(rect.0) + i64::from(rect.2) <= i64::from(area.0) + i64::from(area.2)
        && i64::from(rect.1) + i64::from(rect.3) <= i64::from(area.1) + i64::from(area.3)
}

#[cfg(target_os = "macos")]
fn builtin_origin() -> Result<(f64, f64), String> {
    use core_graphics::display::CGDisplay;
    let displays = CGDisplay::active_displays()
        .map_err(|_| "Unable to enumerate active displays; benchmark remains hidden".to_string())?;
    let builtin: Vec<_> = displays
        .into_iter()
        .map(CGDisplay::new)
        .filter(CGDisplay::is_builtin)
        .collect();
    if builtin.len() != 1 {
        return Err(
            "Exactly one active internal display is required; benchmark remains hidden".into(),
        );
    }
    if builtin[0].is_in_mirror_set() {
        return Err("Internal display is mirrored; capture remains hidden".into());
    }
    let bounds = builtin[0].bounds();
    if !bounds.origin.x.is_finite() || !bounds.origin.y.is_finite() {
        return Err("Internal display has invalid bounds; benchmark remains hidden".into());
    }
    Ok((bounds.origin.x, bounds.origin.y))
}

#[cfg(test)]
mod tests {
    use super::{contains_rect, target_placement};

    #[test]
    fn mixed_dpi_fixed_budget_uses_destination_scale_and_global_origin() {
        // Internal2x is to the right of an external1x screen. Monitor positions are physical.
        let fixed = target_placement((3840, 48, 3456, 2112), 2.0, (1440, 900), 1.0, true).unwrap();
        assert_eq!((fixed.width, fixed.height), (960.0, 540.0));
        assert_eq!((fixed.x, fixed.y), (2304.0, 282.0));
        assert_eq!((fixed.width * 2.0, fixed.height * 2.0), (1920.0, 1080.0));
        // Starting on another Retina screen changes none of the fixed destination geometry.
        assert_eq!(
            fixed,
            target_placement((3840, 48, 3456, 2112), 2.0, (2880, 1800), 2.0, true).unwrap()
        );
    }

    #[test]
    fn lifecycle_preserves_logical_size_and_handles_negative_origins() {
        let target =
            target_placement((-3840, 0, 3456, 2160), 2.0, (1440, 900), 1.0, false).unwrap();
        assert_eq!((target.width, target.height), (1440.0, 900.0));
        assert_eq!((target.x, target.y), (-1776.0, 90.0));
    }

    #[test]
    fn invalid_scale_and_insufficient_internal_pixels_fail_closed() {
        assert!(target_placement((0, 0, 1800, 1080), 2.0, (1440, 900), 1.0, true).is_err());
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(target_placement((0, 0, 3456, 2160), scale, (1440, 900), 1.0, true).is_err());
        }
    }

    #[test]
    fn complete_outer_rect_must_be_inside_internal_work_area() {
        let internal = (-3840, 48, 3456, 2112);
        assert!(contains_rect(internal, (-3000, 200, 1920, 1080)));
        assert!(!contains_rect(internal, (-3900, 200, 1920, 1080)));
        assert!(!contains_rect(internal, (-1000, 200, 1920, 1080)));
        assert!(!contains_rect(internal, (-3000, 1800, 1920, 1080)));
    }
}
