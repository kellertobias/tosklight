//! Read-only native presentation identity, for explicitly enabled private measurements.

/// Describe the exact HAL presentation layer and its parent without acquiring a drawable or
/// mutating any native state. Addresses are process-local diagnostic identities, never handles.
/// Returns `None` outside macOS/Metal or away from the main thread. Call only behind a private
/// measurement gate; class names and pointer identities are not operator-facing diagnostics.
#[cfg(target_os = "macos")]
pub fn surface_layer_identity(surface: &wgpu::Surface<'static>) -> Option<String> {
    objc2::MainThreadMarker::new()?;
    // SAFETY: only read the live Metal layer. The HAL guard and layer lock retain the receiver;
    // retaining its superlayer keeps that receiver alive too. No drawable/device work is involved.
    let hal = unsafe { surface.as_hal::<wgpu::hal::api::Metal>() }?;
    let layer = hal.render_layer().lock();
    let parent = layer.superlayer();
    let (parent_address, parent_class) = parent.as_ref().map_or_else(
        || ("null".to_owned(), "none".to_owned()),
        |parent| {
            (
                format!("{:p}", &**parent),
                parent.class().name().to_string_lossy().into_owned(),
            )
        },
    );
    // SAFETY: CALayer inherits the delegate selector. Inspect its unretained pointer only; do
    // not dereference it, message it, or retain it beyond this main-thread diagnostic call.
    let delegate: *mut objc2::runtime::AnyObject = unsafe { objc2::msg_send![&*layer, delegate] };
    Some(format!(
        "layer={:p} class={} superlayer={} superClass={} delegate={:p} opaque={} hidden={}",
        &*layer,
        layer.class().name().to_string_lossy(),
        parent_address,
        parent_class,
        delegate,
        layer.isOpaque(),
        layer.isHidden(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn surface_layer_identity(_surface: &wgpu::Surface<'static>) -> Option<String> {
    None
}
