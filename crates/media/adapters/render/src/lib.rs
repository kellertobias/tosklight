#![forbid(unsafe_code)]

//! The Media render adapter.
//!
//! One instance owns one output: its surface, its presentation mode, its render clock, and its
//! pipelines. A slow or disconnected output never stops another from presenting, because nothing
//! here is shared between them except the immutable asset data the caller hands in.
//!
//! It owns no DMX parsing, no HTTP, no filesystem scanning, no decoder policy, and no
//! control-source ownership. It takes state and textures and presents an image.

pub mod compositor;
pub mod gpu;
pub mod offscreen;
pub mod texture;
pub mod visualizer;
pub mod window;

pub use compositor::{Compositor, LayerDraw, MAX_LAYERS, PROGRAM_FORMAT};
pub use gpu::{Capabilities, Gpu, GpuError, PresentationSurface};
pub use offscreen::OffScreenOutput;
pub use texture::{SourceTexture, TextureError, block_bytes};
pub use visualizer::{VisualizerError, VisualizerFrame, VisualizerRenderer};
pub use window::{SurfaceLost, WindowSurface, WindowedOutput, select_monitor};

use media_domain::clock::{MeasuredCadence, RenderClock};
use media_domain::geometry::Size;
use media_domain::{MasterState, OutputId, PresentationMode};

/// One output instance: everything that belongs to one presented image.
pub struct OutputRenderer {
    id: OutputId,
    clock: RenderClock,
    compositor: Compositor,
    target: OffScreenOutput,
}

impl OutputRenderer {
    /// Builds an off-screen output. Windowed outputs arrive with the surface lifecycle; the
    /// composition itself is identical either way.
    pub fn off_screen(
        gpu: &Gpu,
        id: OutputId,
        size: Size,
        presentation: PresentationMode,
    ) -> Result<Self, GpuError> {
        if !gpu.supports_resolution(size.width, size.height) {
            return Err(GpuError::UnsupportedSurface);
        }
        Ok(Self {
            id,
            clock: RenderClock::new(presentation),
            compositor: Compositor::new(gpu, size, PROGRAM_FORMAT),
            target: OffScreenOutput::new(gpu, size),
        })
    }

    pub const fn id(&self) -> OutputId {
        self.id
    }

    pub const fn size(&self) -> Size {
        self.compositor.size()
    }

    /// What this output actually presented at, rather than what it asked for.
    pub fn cadence(&self) -> MeasuredCadence {
        self.clock.measured()
    }

    /// Whether the clock says it is time to present.
    pub fn should_present(&self, now: media_domain::Timestamp) -> bool {
        self.clock.should_present(now)
    }

    /// Composites and presents one frame, and records the cadence.
    pub fn present(
        &mut self,
        layers: &[LayerDraw<'_>],
        master: &MasterState,
        now: media_domain::Timestamp,
    ) {
        self.compositor.render(layers, master, self.target.view());
        self.clock.record_present(now);
    }

    /// Reads the presented image back. Used by reference renders and by the CITP preview, which
    /// asks for it only while a desk is subscribed.
    pub fn read_image(&self) -> Vec<u8> {
        self.target.read_image()
    }

    /// Rebuilds this output for a new resolution.
    ///
    /// Monitor changes, refresh-rate changes, sleep and wake, and surface loss all land here, and
    /// all of them affect only this output. The measured cadence is discarded with the surface,
    /// because the cadence from before the change says nothing about the cadence after it.
    pub fn recreate(&mut self, size: Size) {
        self.compositor.resize(size);
        self.target.resize(size);
        self.clock.reset();
    }
}
