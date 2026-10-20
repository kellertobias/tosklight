//! A macOS child webview follows the native content size, never fullscreen transition ratios.
use tauri::{PhysicalPosition, PhysicalSize, Rect};

fn content_bounds(size: PhysicalSize<u32>) -> Option<Rect> {
    (size.width > 0 && size.height > 0).then_some(Rect {
        position: PhysicalPosition::new(0, 0).into(),
        size: size.into(),
    })
}

#[cfg(target_os = "macos")]
pub(super) fn fit(window: &tauri::Window, webview: &tauri::Webview) -> tauri::Result<()> {
    if let Some(bounds) = content_bounds(window.inner_size()?) {
        webview.set_bounds(bounds)?;
    }
    Ok(())
}

/// Tauri's proportional autoresize caches child/window ratios during add_child. Native
/// fullscreen changes the content height asynchronously, so those two measurements can
/// disagree and permanently oversize the child. Wry's bottom-origin conversion then places
/// its top above the visible content view. Track exact current bounds after each transition.
#[cfg(target_os = "macos")]
pub(super) fn install(window: &tauri::Window, webview: &tauri::Webview) -> tauri::Result<()> {
    webview.set_auto_resize(false)?;
    fit(window, webview)?;
    let owner = window.clone();
    let child = webview.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. }
        ) {
            if let Err(error) = fit(&owner, &child) {
                eprintln!("Could not fit screen webview to native content bounds: {error}");
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_content_height_is_never_scaled_by_a_pretransition_measurement() {
        // Retina window, fullscreen transition, then the usable content below system chrome.
        // A ratio seeded from 1964/1860 would push the child's top 104 physical pixels out.
        for size in [
            PhysicalSize::new(2880, 1800),
            PhysicalSize::new(3024, 1964),
            PhysicalSize::new(3024, 1860),
            PhysicalSize::new(2880, 1800),
        ] {
            let bounds = content_bounds(size).unwrap();
            assert_eq!(
                bounds.position.to_physical::<i32>(2.0),
                PhysicalPosition::new(0, 0)
            );
            assert_eq!(bounds.size.to_physical::<u32>(2.0), size);
            assert_eq!(
                bounds.size.to_logical::<f64>(2.0).height,
                f64::from(size.height) / 2.0
            );
        }
    }

    #[test]
    fn moving_between_retina_and_external_displays_preserves_native_content_pixels() {
        for (scale, size) in [
            (2.0, PhysicalSize::new(3024, 1860)),
            (1.0, PhysicalSize::new(1920, 1080)),
        ] {
            let bounds = content_bounds(size).unwrap();
            assert_eq!(bounds.size.to_physical::<u32>(scale), size);
            assert_eq!(
                bounds.position.to_logical::<f64>(scale),
                tauri::LogicalPosition::new(0.0, 0.0)
            );
        }
    }

    #[test]
    fn minimized_zero_extent_does_not_resize_the_child_to_an_invalid_viewport() {
        assert!(content_bounds(PhysicalSize::new(0, 900)).is_none());
        assert!(content_bounds(PhysicalSize::new(1440, 0)).is_none());
    }
}
