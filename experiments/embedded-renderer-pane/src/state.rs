//! What the window's HTML tells the renderer, and what the renderer reports back.
//!
//! The two live in one process here. In the real thing the renderer is a supervised helper and
//! this is the shape that would cross the IPC boundary, so it is deliberately small and made of
//! values rather than handles.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// The rectangle the 3D pane occupies, in logical points from the window's top-left.
///
/// The web side owns this: the pane is a element in a layout, and the renderer draws where that
/// element ended up. Everything outside it must be untouched by the renderer, which is what
/// "clipping" means for the pane contract.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct PaneRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Where the camera is looking, driven by input the web side forwarded.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 0.6,
            pitch: 0.35,
            distance: 1.0,
        }
    }
}

#[derive(Default)]
pub struct Shared {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    pane: PaneRect,
    camera: Camera,
    /// Counted so the readout can show that forwarded input actually arrived, and how much of it.
    input_events: u64,
    /// Frames the renderer has presented, for the same reason.
    frames: u64,
}

impl Shared {
    pub fn set_pane(&self, pane: PaneRect) {
        self.inner.lock().pane = pane;
    }

    pub fn orbit(&self, delta_x: f32, delta_y: f32) {
        let mut inner = self.inner.lock();
        inner.camera.yaw += delta_x * 0.01;
        // Stopped short of straight up and straight down, so the picture never turns over.
        inner.camera.pitch = (inner.camera.pitch + delta_y * 0.01).clamp(-1.4, 1.4);
        inner.input_events += 1;
    }

    pub fn zoom(&self, delta: f32) {
        let mut inner = self.inner.lock();
        inner.camera.distance = (inner.camera.distance * (1.0 + delta * 0.001)).clamp(0.3, 4.0);
        inner.input_events += 1;
    }

    pub fn read(&self) -> (PaneRect, Camera) {
        let inner = self.inner.lock();
        (inner.pane, inner.camera)
    }

    pub fn count_frame(&self) {
        self.inner.lock().frames += 1;
    }

    pub fn report(&self) -> Report {
        let inner = self.inner.lock();
        Report {
            pane: inner.pane,
            input_events: inner.input_events,
            frames: inner.frames,
            yaw: inner.camera.yaw,
            pitch: inner.camera.pitch,
            distance: inner.camera.distance,
        }
    }
}

/// What the status readout in the window shows, so the experiment's claims are visible rather
/// than asserted in a README.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Report {
    pub pane: PaneRect,
    pub input_events: u64,
    pub frames: u64,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}
