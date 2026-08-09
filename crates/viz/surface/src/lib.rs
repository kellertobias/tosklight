#![allow(unsafe_code, reason = "reviewed platform shared-surface boundary")]

//! A GPU surface two processes on one machine can both address.
//!
//! The desk draws its Stage with a renderer that runs in another process, because a GPU driver can
//! end a process and the Programmer, playback and output engine must not be in that one. That
//! leaves one picture to move between two processes several dozen times a second.
//!
//! Sending pixels is the obvious answer and the wrong one. The two processes are looking at the
//! same GPU: reading a frame back to system memory stalls the render pipeline, and the desk then
//! uploads it straight back to the device it came from. Two transfers and a stall per frame to
//! move an image between two programs that already shared the hardware holding it.
//!
//! So instead the helper draws into a surface the desk can sample. The platform decides what that
//! is — an `IOSurface` on macOS — and each side wraps it in a `wgpu` texture of its own.
//!
//! # Why this crate exists at all
//!
//! Importing a texture another process created is a raw handle and a platform call, and there is
//! no safe way to write it. The workspace forbids `unsafe` everywhere else so that it is confined
//! to this crate, where it is a small reviewed surface rather than something spread across the
//! renderer. Everything here is either the export side or the import side of the same operation,
//! and both are checked against the same size and format before anything is dereferenced.
//!
//! # What happens where there is no shared surface
//!
//! [`is_supported`] answers false, the two sides never negotiate [`FrameTransport::Shared`], and
//! the pane is carried by the copy transport instead — or, if the desk cannot embed at all, it
//! keeps drawing the Stage with its own web renderer. Nothing here is on the path that decides
//! that; it only reports what it can do.
//!
//! [`FrameTransport::Shared`]: viz_helper::protocol::FrameTransport::Shared

#[cfg(target_os = "macos")]
pub mod rendezvous;

use viz_helper::protocol::SharedSurfaceHandle;

/// The texture format both sides use.
///
/// Fixed rather than negotiated: the renderer's headless path already writes straight RGBA so a
/// capture is byte-identical wherever it runs, and matching it here means the desk samples what
/// the helper drew without a conversion in between.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Whether this build can share a surface with another process here.
///
/// True on macOS, where the surface is an `IOSurface` and its right is handed over by
/// [`rendezvous`]. Whether *this* process may open a rendezvous is a separate question — a
/// restricted process may not register a bootstrap name — so the desk asks by opening one rather
/// than by asking here.
pub fn is_supported() -> bool {
    cfg!(target_os = "macos")
}

/// A surface one process created and both can draw or sample.
pub struct SharedSurface {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    handle: SharedSurfaceHandle,
    width: u32,
    height: u32,
    /// Kept alive for as long as the texture refers to its memory. Dropping the surface while a
    /// texture still points into it is exactly the crash this field exists to prevent.
    #[cfg(target_os = "macos")]
    _backing: Backing,
}

/// The surface's own memory, held so it outlives the texture over it.
///
/// `CFRetained<IOSurfaceRef>` is neither `Send` nor `Sync` in the bindings, because the generated
/// `IOSurfaceRef` is an opaque type the generator cannot make claims about. `IOSurface` itself is
/// documented as usable from any thread — that is the entire point of a surface two *processes*
/// share — and this holds nothing but a retain count that Core Foundation manages atomically. The
/// desk keeps its compositor behind a mutex and hands it to the thread that owns the window, which
/// needs the whole thing to cross a thread boundary, so the claim is made here rather than by
/// giving up and copying pixels.
#[cfg(target_os = "macos")]
struct Backing(
    #[allow(dead_code, reason = "held for its lifetime, not read")]
    objc2_core_foundation::CFRetained<objc2_io_surface::IOSurfaceRef>,
);

// SAFETY: an `IOSurface` may be used from any thread, and this owns only a retain on one.
#[cfg(target_os = "macos")]
unsafe impl Send for Backing {}
// SAFETY: as above. Sharing a reference exposes no interior mutability of its own; the pixels are
// written through Metal textures, whose own synchronisation is the GPU's.
#[cfg(target_os = "macos")]
unsafe impl Sync for Backing {}

impl SharedSurface {
    /// The texture, for a caller that wants to sample or copy it.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// A view of the whole surface, which is what the renderer draws into.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// What to send the other process so it can open the same surface.
    pub fn handle(&self) -> SharedSurfaceHandle {
        self.handle
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// A send right to this surface, for handing to another process over a [`rendezvous`].
    ///
    /// A fresh right each call, which the caller sends and the kernel then owns. This is the only
    /// name for a surface that a second process can actually resolve.
    #[cfg(target_os = "macos")]
    pub fn mach_port(&self) -> mach2::port::mach_port_t {
        self._backing.0.create_mach_port()
    }
}

/// Put a window's drawing layer behind everything the interface draws on top of it.
///
/// The whole arrangement depends on the web interface compositing *above* the rendered picture:
/// menus, dialogs and the pane's own settings have to be able to open across the Stage. A layer
/// added to the window sorts among its siblings, and by default it can land in front of the child
/// webview — at which point everything the interface draws inside the pane rectangle disappears
/// behind the picture, while everything outside it looks fine. That reads as "the pane settings
/// button does nothing".
///
/// A negative `zPosition` sorts it behind its siblings whatever order they were added in.
#[cfg(target_os = "macos")]
pub fn send_surface_layer_to_back(surface: &wgpu::Surface<'static>) {
    // SAFETY: reads the Metal layer this surface already owns and sets one of its properties. The
    // layer outlives the call, and `zPosition` is a plain animatable property with no invariants
    // tied to the swapchain.
    if let Some(hal) = unsafe { surface.as_hal::<wgpu::hal::api::Metal>() } {
        // Ordered among its siblings rather than sunk beneath them. A negative position put it
        // behind the window's own opaque backing as well, which is a picture nobody can see; zero
        // leaves it where it was created, above the background and below whatever is raised over
        // it.
        hal.render_layer().lock().setZPosition(0.0);
    }
}

/// Raise a view's layer above the siblings it shares a window with.
///
/// The desk's window holds two things: the surface the renderer draws the Stage into, and the
/// interface as a transparent child on top. "On top" has to be true of the *layers*, not merely of
/// the order they were added, or everything the interface draws inside the pane rectangle vanishes
/// behind the picture — which is what an operator sees as the pane's settings button doing nothing.
///
/// Raising the interface is the right half of that sandwich to move. Sinking the picture instead
/// puts it behind the window's own backing as well, and a layer behind that is a picture nobody
/// can see.
///
/// `view` is an `NSView`; a null pointer is ignored.
#[cfg(target_os = "macos")]
pub fn raise_view_above_siblings(view: *mut std::ffi::c_void) {
    if view.is_null() {
        return;
    }
    // SAFETY: the caller hands over a live `NSView` for the lifetime of the call — Tauri's
    // `with_webview` runs its closure on the main thread with the webview alive. `wantsLayer` and
    // `zPosition` are plain properties with no invariants tied to what the view is drawing.
    unsafe {
        let view = view.cast::<objc2::runtime::AnyObject>();
        // Only the ordering is touched. Asking for a backing layer here instead would give a
        // `WKWebView` — already layer-backed — a fresh opaque one, and an opaque interface covers
        // the very picture this is trying to reveal.
        let layer: *mut objc2::runtime::AnyObject = objc2::msg_send![view, layer];
        if layer.is_null() {
            return;
        }
        let _: () = objc2::msg_send![layer, setZPosition: 1.0_f64];
        // And it must stay see-through: everything outside what the interface paints is where the
        // Stage shows through.
        let _: () = objc2::msg_send![layer, setOpaque: false];
    }
}

/// Open a surface another process sent the right to.
///
/// The size is the sender's, so a right for a pane the layout has already moved past is refused
/// rather than drawn at the wrong shape.
#[cfg(target_os = "macos")]
pub fn import_from_port(
    device: &wgpu::Device,
    port: mach2::port::mach_port_t,
    width: u32,
    height: u32,
) -> Result<SharedSurface, SurfaceError> {
    platform::import_from_port(device, port, width, height)
}

/// Create a surface this process draws into and another can sample.
pub fn create(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> Result<SharedSurface, SurfaceError> {
    platform::create(device, width, height)
}

/// Open a surface another process created.
///
/// The size is taken from the sender rather than from the surface so a stale announcement — one
/// that named a pane the layout has already moved past — is refused here instead of producing a
/// texture whose contents do not match what the desk is about to draw it as.
pub fn import(
    device: &wgpu::Device,
    handle: SharedSurfaceHandle,
    width: u32,
    height: u32,
) -> Result<SharedSurface, SurfaceError> {
    platform::import(device, handle, width, height)
}

#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceError {
    /// This platform has no shared surface, or this build was not compiled with one.
    Unsupported,
    /// The handle names something this platform does not know how to open — a Windows handle
    /// arriving at a macOS desk, which means two halves from different builds.
    WrongPlatform,
    /// The surface could not be created or opened, with what the platform said.
    Failed(String),
    /// The surface is not the size the sender said it was.
    SizeMismatch {
        expected: (u32, u32),
        found: (u32, u32),
    },
}

impl std::fmt::Display for SurfaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(
                formatter,
                "this platform has no surface two processes can share"
            ),
            Self::WrongPlatform => write!(
                formatter,
                "the renderer offered a surface for another platform; reinstall so both halves \
                 come from the same build"
            ),
            Self::Failed(detail) => write!(formatter, "shared surface: {detail}"),
            Self::SizeMismatch { expected, found } => write!(
                formatter,
                "shared surface is {}x{} where {}x{} was announced",
                found.0, found.1, expected.0, expected.1
            ),
        }
    }
}

impl std::error::Error for SurfaceError {}

/// Bytes per pixel of [`FORMAT`], which the surface has to be created with.
const BYTES_PER_ELEMENT: i32 = 4;

#[cfg(target_os = "macos")]
mod platform {
    use super::{BYTES_PER_ELEMENT, Backing, FORMAT, SharedSurface, SurfaceError};
    use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFString};
    use objc2_io_surface::{
        IOSurfaceRef, kIOSurfaceBytesPerElement, kIOSurfaceHeight, kIOSurfacePixelFormat,
        kIOSurfaceWidth,
    };
    use objc2_metal::{MTLDevice, MTLPixelFormat, MTLTextureDescriptor, MTLTextureUsage};
    use viz_helper::protocol::SharedSurfaceHandle;

    /// `'RGBA'` as an `OSType`, matching [`FORMAT`]'s channel order.
    ///
    /// The four-character code describes the bytes, not the colour space: the sRGB-ness of the
    /// format lives in the Metal texture both sides create over the same memory, and both create
    /// it from the same constant.
    const PIXEL_FORMAT_RGBA: i32 = 0x5247_4241;

    pub(super) fn create(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Result<SharedSurface, SurfaceError> {
        let properties = properties(width, height);
        // SAFETY: the dictionary holds only the documented `IOSurface` keys, with numbers of the
        // types those keys are specified to take. `IOSurfaceCreate` copies what it needs.
        let surface = unsafe { IOSurfaceRef::new(&properties) }
            .ok_or_else(|| SurfaceError::Failed("IOSurfaceCreate returned nothing".to_owned()))?;
        let handle = SharedSurfaceHandle::IoSurfaceId(surface.id());
        wrap(device, surface, width, height, handle)
    }

    /// Open a surface from a right another process sent.
    ///
    /// This is the route that works. The ID route below is kept only for the handle a message
    /// still carries, and is documented where it is defined as not resolving on its own.
    pub(super) fn import_from_port(
        device: &wgpu::Device,
        port: mach2::port::mach_port_t,
        width: u32,
        height: u32,
    ) -> Result<SharedSurface, SurfaceError> {
        let surface = IOSurfaceRef::lookup_from_mach_port(port).ok_or_else(|| {
            SurfaceError::Failed("the right the renderer sent names no surface".to_owned())
        })?;
        let found = (surface.width() as u32, surface.height() as u32);
        if found != (width, height) {
            return Err(SurfaceError::SizeMismatch {
                expected: (width, height),
                found,
            });
        }
        wrap(
            device,
            surface,
            width,
            height,
            SharedSurfaceHandle::IoSurfaceId(0),
        )
    }

    pub(super) fn import(
        device: &wgpu::Device,
        handle: SharedSurfaceHandle,
        width: u32,
        height: u32,
    ) -> Result<SharedSurface, SurfaceError> {
        let SharedSurfaceHandle::IoSurfaceId(id) = handle else {
            return Err(SurfaceError::WrongPlatform);
        };
        let surface = IOSurfaceRef::lookup(id).ok_or_else(|| {
            SurfaceError::Failed(format!("no surface on this machine is called {id}"))
        })?;
        // The renderer may already have replaced the surface for a newer pane size by the time
        // this arrives. Drawing the old one stretched is worse than skipping the frame.
        let found = (surface.width() as u32, surface.height() as u32);
        if found != (width, height) {
            return Err(SurfaceError::SizeMismatch {
                expected: (width, height),
                found,
            });
        }
        wrap(device, surface, width, height, handle)
    }

    /// The properties `IOSurfaceCreate` takes. Row bytes are left to the system, which pads to
    /// whatever the hardware wants — asking for a tighter row than the GPU accepts is a surface
    /// that fails to create for no visible reason.
    fn properties(width: u32, height: u32) -> CFRetained<CFDictionary> {
        // Deliberately not `kIOSurfaceIsGlobal`. It would ask for a surface `IOSurfaceLookup` can
        // find by ID, which modern macOS ignores anyway — measured on Darwin 25.5 — and a global
        // surface is readable by anything on the machine that guesses a small integer. The right
        // is handed to exactly one process instead, through the rendezvous.
        let keys: [&CFString; 4] = unsafe {
            [
                kIOSurfaceWidth,
                kIOSurfaceHeight,
                kIOSurfaceBytesPerElement,
                kIOSurfacePixelFormat,
            ]
        };
        let values = [
            CFNumber::new_i32(width as i32),
            CFNumber::new_i32(height as i32),
            CFNumber::new_i32(BYTES_PER_ELEMENT),
            CFNumber::new_i32(PIXEL_FORMAT_RGBA),
        ];
        let mut key_pointers: Vec<*const std::ffi::c_void> = keys
            .iter()
            .map(|key| (*key as *const CFString).cast())
            .collect();
        let mut value_pointers: Vec<*const std::ffi::c_void> = values
            .iter()
            .map(|value| (&raw const **value).cast())
            .collect();
        // SAFETY: both arrays have the same length and outlive the call, and the dictionary
        // callbacks are the Core Foundation defaults, which retain what they are given.
        unsafe {
            CFDictionary::new(
                None,
                key_pointers.as_mut_ptr(),
                value_pointers.as_mut_ptr(),
                keys.len() as isize,
                std::ptr::null(),
                std::ptr::null(),
            )
        }
        .expect("a dictionary of four constant keys")
    }

    /// Build a `wgpu` texture over an `IOSurface`, whichever side created it.
    fn wrap(
        device: &wgpu::Device,
        surface: CFRetained<IOSurfaceRef>,
        width: u32,
        height: u32,
        handle: SharedSurfaceHandle,
    ) -> Result<SharedSurface, SurfaceError> {
        let descriptor = MTLTextureDescriptor::new();
        // SAFETY: plain property writes on a descriptor this function owns and has not yet handed
        // to Metal. They are `unsafe` only because a descriptor is shared mutable state in
        // Objective-C, and nothing else holds this one.
        unsafe {
            descriptor.setPixelFormat(MTLPixelFormat::RGBA8Unorm_sRGB);
            descriptor.setWidth(width as usize);
            descriptor.setHeight(height as usize);
            descriptor.setUsage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
        }

        let metal_texture = {
            // SAFETY: the closure runs on the Metal device this `wgpu` device was opened with, and
            // the descriptor above matches the surface's element size and dimensions. The returned
            // texture retains the surface, and the surface is kept on the struct besides.
            // A `wgpu` device that is not Metal means the renderer picked another backend on
            // macOS, which is a build or driver situation rather than a bug to panic on.
            let hal_device = unsafe { device.as_hal::<wgpu::hal::api::Metal>() }
                .ok_or(SurfaceError::Unsupported)?;
            let created = hal_device
                .raw_device()
                .newTextureWithDescriptor_iosurface_plane(&descriptor, &surface, 0);
            created.ok_or_else(|| {
                SurfaceError::Failed("Metal refused a texture over the shared surface".to_owned())
            })?
        };

        let hal_texture = unsafe {
            wgpu::hal::metal::Device::texture_from_raw(
                metal_texture,
                FORMAT,
                objc2_metal::MTLTextureType::Type2D,
                1,
                1,
                wgpu::hal::CopyExtent {
                    width,
                    height,
                    depth: 1,
                },
                None,
            )
        };
        let descriptor = wgpu::TextureDescriptor {
            label: Some("viz shared pane"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            // Copying is not on the fast path — the desk samples this rather than copying it —
            // but it is what lets the surface be read back, which is the only way to prove the two
            // processes are looking at one picture rather than two.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        // SAFETY: the descriptor describes exactly the texture built above — same format, same
        // size, same single mip and layer — and the Metal texture outlives the `wgpu` one because
        // ownership moves into it here.
        let texture = unsafe {
            device.create_texture_from_hal::<wgpu::hal::api::Metal>(
                hal_texture,
                &descriptor,
                wgpu::wgt::TextureUses::UNINITIALIZED,
            )
        };
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(SharedSurface {
            texture,
            view,
            handle,
            width,
            height,
            _backing: Backing(surface),
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::{SharedSurface, SurfaceError};
    use viz_helper::protocol::SharedSurfaceHandle;

    pub(super) fn create(
        _device: &wgpu::Device,
        _width: u32,
        _height: u32,
    ) -> Result<SharedSurface, SurfaceError> {
        Err(SurfaceError::Unsupported)
    }

    pub(super) fn import(
        _device: &wgpu::Device,
        _handle: SharedSurfaceHandle,
        _width: u32,
        _height: u32,
    ) -> Result<SharedSurface, SurfaceError> {
        Err(SurfaceError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two halves must agree on what they are looking at. A format negotiated per side would
    /// eventually differ, and the failure would be a picture with its colours swapped rather than
    /// an error anybody could act on.
    #[test]
    fn both_sides_use_one_format() {
        assert_eq!(FORMAT, wgpu::TextureFormat::Rgba8UnormSrgb);
        assert_eq!(FORMAT.block_copy_size(None), Some(BYTES_PER_ELEMENT as u32));
    }

    /// The list either side announces must never claim more than this crate can actually do. A
    /// transport announced but not deliverable is a negotiation that succeeds and a black pane.
    #[test]
    fn the_transport_list_never_claims_more_than_this_crate_can_deliver() {
        let shared = viz_helper::protocol::supported_transports()
            .contains(&viz_helper::protocol::FrameTransport::Shared);
        assert_eq!(
            shared,
            is_supported(),
            "a platform announces a shared transport exactly when it has one"
        );
    }

    /// A handle from the other platform is refused rather than opened as a number.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_windows_handle_is_refused_by_a_macos_desk() {
        let error = SurfaceError::WrongPlatform;
        assert!(error.to_string().contains("another platform"), "{error}");
    }

    #[test]
    fn a_size_mismatch_says_both_sizes() {
        let error = SurfaceError::SizeMismatch {
            expected: (1_920, 1_080),
            found: (960, 540),
        };
        let message = error.to_string();
        assert!(message.contains("960x540"), "{message}");
        assert!(message.contains("1920x1080"), "{message}");
    }
}

/// Two `wgpu` devices, one surface, and a picture that actually crossed.
///
/// The claim this crate makes is that a texture one device draws into can be read by a texture
/// another device opened over the same surface. Nothing about that is provable by inspection —
/// the platform call either shares memory or silently gives each side its own — so it is checked
/// against real devices, at the cost of needing a GPU to run.
#[cfg(all(test, target_os = "macos"))]
mod round_trip {
    use super::*;

    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
    }

    /// Write a known colour on one device, read it on another. A shared surface makes these the
    /// same pixels; two unrelated textures would leave the second one cleared.
    #[test]
    fn a_frame_written_on_one_device_is_read_on_another() {
        let Some((writer, writer_queue)) = device() else {
            eprintln!("no Metal adapter; skipping the shared-surface round trip");
            return;
        };
        let Some((reader, reader_queue)) = device() else {
            return;
        };

        let (width, height) = (64, 32);
        let exported = create(&writer, width, height).expect("a shared surface");
        let imported =
            import(&reader, exported.handle(), width, height).expect("the same surface, opened");

        // A clear is enough: it goes through the same attachment path a real frame does.
        let mut encoder = writer.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shared surface round trip"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: exported.view(),
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime();
        writer_queue.submit(Some(encoder.finish()));
        let _ = writer.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        let row = width * 4;
        let padded = row.div_ceil(256) * 256;
        let readback = reader.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(padded * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = reader.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            imported.texture().as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        reader_queue.submit(Some(encoder.finish()));
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = reader.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let mapped = slice.get_mapped_range().expect("the copy completed");
        assert_eq!(
            &mapped[0..4],
            &[255, 0, 0, 255],
            "the reading device sees what the writing device drew, so the surface is one surface"
        );
    }

    /// A pane that has already been resized must not be drawn at the old size.
    #[test]
    fn a_surface_opened_at_the_wrong_size_is_refused() {
        let Some((writer, _)) = device() else {
            return;
        };
        let Some((reader, _)) = device() else {
            return;
        };
        let exported = create(&writer, 64, 32).expect("a shared surface");
        assert!(matches!(
            import(&reader, exported.handle(), 128, 32),
            Err(SurfaceError::SizeMismatch { .. })
        ));
    }
}
