use crate::ProgrammerRegistry;
use light_core::SessionId;
use serde::{Deserialize, Serialize};

/// Exact user-owned Programmer capture state shared by every session for one user.
///
/// The tuple is persisted through the corresponding fields on `ProgrammerState`. Its public
/// revision is runtime-only and is advanced by the Programming application boundary, never by
/// low-level domain helpers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProgrammerCaptureMode {
    pub blind: bool,
    pub preview: bool,
    pub preload_capture_programmer: bool,
}

impl Default for ProgrammerCaptureMode {
    fn default() -> Self {
        Self {
            blind: false,
            preview: false,
            preload_capture_programmer: true,
        }
    }
}

impl ProgrammerCaptureMode {
    pub const fn redirects_normal_values_to_preload(self) -> bool {
        self.blind && self.preload_capture_programmer
    }
}

impl ProgrammerRegistry {
    pub fn capture_mode(&self, session: SessionId) -> Option<ProgrammerCaptureMode> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let state_key = self.key(session);
        let state = self.states.read();
        let state = state.get(&state_key)?;
        Some(ProgrammerCaptureMode {
            blind: state.blind,
            preview: state.preview,
            preload_capture_programmer: state.preload_capture_programmer,
        })
    }
}
