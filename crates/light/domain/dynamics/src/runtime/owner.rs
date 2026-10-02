//! Mutable operational ownership of a surviving controller.
//!
//! A Playback assignment can move, change priority or change its page-qualified identity while
//! its logical controller survives. The owner and priority are refreshed in place; instance
//! identity, clocks, pause, phase, Random streams and held samples are never touched.
use super::*;

impl DynamicControllerSource {
    pub const fn physical_playback(playback_number: u16) -> Self {
        Self::Playback {
            playback_number,
            virtual_page: None,
        }
    }

    pub const fn virtual_playback(page: u8, playback_number: u16) -> Self {
        Self::Playback {
            playback_number,
            virtual_page: Some(page),
        }
    }

    /// Only the operational address of the same runtime owner may change.
    fn accepts_owner_refresh(&self, next: &Self) -> bool {
        match (self, next) {
            (Self::Playback { .. }, Self::Playback { .. }) => true,
            _ => self.same_runtime_owner(next),
        }
    }
}

impl DynamicRuntime {
    /// Refresh the current owner identity and priority of a surviving controller. Returns
    /// whether anything changed. Programmer and Cue owners keep their runtime identity.
    pub fn update_controller_owner(
        &mut self,
        controller_id: Uuid,
        source: DynamicControllerSource,
        priority: i16,
    ) -> Result<bool, DynamicRuntimeError> {
        let (instance_id, controller) = self
            .controller(controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?;
        if !controller.source.accepts_owner_refresh(&source) {
            return Err(DynamicRuntimeError::InvalidController);
        }
        if controller.source == source && controller.priority == priority {
            return Ok(false);
        }
        self.journal_instance(instance_id);
        let controller = self
            .instances
            .get_mut(&instance_id)
            .expect("existing instance")
            .controllers
            .get_mut(&controller_id)
            .expect("existing controller");
        controller.source = source;
        controller.priority = priority;
        Ok(true)
    }
}
