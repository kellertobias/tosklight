//! Same target-scale and full outer-rectangle rule as the Control native benchmark guard.
#[derive(Debug, PartialEq)]
struct Placement {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
fn placement(area: (i32, i32, u32, u32), scale: f64) -> Result<Placement, String> {
    if !scale.is_finite() || scale <= 0.0 || area.2 == 0 || area.3 == 0 {
        return Err("Invalid internal display bounds/scale".into());
    }
    let width = (f64::from(area.2) / scale - 32.0).min(1440.0);
    let height = (f64::from(area.3) / scale - 32.0).min(900.0);
    if width < 640.0 || height < 480.0 {
        return Err("Internal display is too small for validation".into());
    }
    Ok(Placement {
        x: f64::from(area.0) / scale + 16.0,
        y: f64::from(area.1) / scale + 16.0,
        width,
        height,
    })
}
fn contains(area: (i32, i32, u32, u32), rect: (i32, i32, u32, u32)) -> bool {
    rect.2 > 0
        && rect.3 > 0
        && rect.0 >= area.0
        && rect.1 >= area.1
        && i64::from(rect.0) + i64::from(rect.2) <= i64::from(area.0) + i64::from(area.2)
        && i64::from(rect.1) + i64::from(rect.3) <= i64::from(area.1) + i64::from(area.3)
}
#[cfg(target_os = "macos")]
fn builtin_origin() -> Result<(f64, f64), String> {
    use core_graphics::display::CGDisplay;
    let builtin: Vec<_> = CGDisplay::active_displays()
        .map_err(|_| "Cannot enumerate internal displays")?
        .into_iter()
        .map(CGDisplay::new)
        .filter(CGDisplay::is_builtin)
        .collect();
    if builtin.len() != 1 || builtin[0].is_in_mirror_set() {
        return Err("Exactly one active nonmirrored internal display is required".into());
    }
    let bounds = builtin[0].bounds();
    if !bounds.origin.x.is_finite()
        || !bounds.origin.y.is_finite()
        || !bounds.size.width.is_finite()
        || !bounds.size.height.is_finite()
        || bounds.size.width <= 0.0
        || bounds.size.height <= 0.0
    {
        return Err("Internal display bounds are invalid".into());
    }
    Ok((bounds.origin.x, bounds.origin.y))
}
#[cfg(target_os = "macos")]
pub(super) async fn place(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.is_visible().map_err(|e| e.to_string())? {
        return Err("Window must remain hidden until placement is verified".into());
    }
    let origin = builtin_origin()?;
    let monitors = window.available_monitors().map_err(|e| e.to_string())?;
    let matching: Vec<_> = monitors
        .iter()
        .filter(|m| {
            m.position().x == (origin.0 * m.scale_factor()).round() as i32
                && m.position().y == (origin.1 * m.scale_factor()).round() as i32
        })
        .collect();
    if matching.len() != 1 {
        return Err("Cannot uniquely match internal display to native monitor".into());
    }
    let target = matching[0];
    let work = target.work_area();
    let scale = target.scale_factor();
    let area = (
        work.position.x,
        work.position.y,
        work.size.width,
        work.size.height,
    );
    let geometry = placement(area, scale)?;
    window.set_fullscreen(false).map_err(|e| e.to_string())?;
    window.unmaximize().map_err(|e| e.to_string())?;
    window
        .set_min_size(None::<tauri::LogicalSize<f64>>)
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::LogicalPosition::new(
            f64::from(area.0) / scale,
            f64::from(area.1) / scale,
        ))
        .map_err(|e| e.to_string())?;
    window
        .set_size(tauri::LogicalSize::new(geometry.width, geometry.height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::LogicalPosition::new(geometry.x, geometry.y))
        .map_err(|e| e.to_string())?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if window.is_visible().map_err(|e| e.to_string())? {
            return Err("Window became visible before verification".into());
        }
        if builtin_origin()? != origin {
            return Err("Internal display changed during placement".into());
        }
        let current = window.current_monitor().map_err(|e| e.to_string())?;
        let position = window.outer_position().map_err(|e| e.to_string())?;
        let size = window.outer_size().map_err(|e| e.to_string())?;
        if current
            .as_ref()
            .is_some_and(|m| m.position() == target.position() && m.scale_factor() == scale)
            && contains(area, (position.x, position.y, size.width, size.height))
        {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("Internal placement did not settle; window remains hidden".into());
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
#[cfg(not(target_os = "macos"))]
pub(super) async fn place(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Err("Internal validation display is only supported on macOS".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_placement_uses_target_scale_and_rejects_invalid_or_spanning_geometry() {
        let p = placement((3840, 48, 2880, 1752), 2.0).unwrap();
        assert_eq!(
            p,
            Placement {
                x: 1936.0,
                y: 40.0,
                width: 1408.0,
                height: 844.0
            }
        );
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(placement((0, 0, 2880, 1800), scale).is_err());
        }
        assert!(placement((0, 0, 0, 1800), 2.0).is_err());
        assert!(placement((0, 0, 100, 100), 1.0).is_err());
        assert!(contains((3840, 48, 2880, 1752), (3872, 80, 2816, 1688)));
        for rect in [
            (3839, 48, 100, 100),
            (3840, 47, 100, 100),
            (3840, 48, 2881, 100),
            (3840, 48, 100, 1753),
            (3840, 48, 0, 100),
        ] {
            assert!(!contains((3840, 48, 2880, 1752), rect));
        }
    }
}
