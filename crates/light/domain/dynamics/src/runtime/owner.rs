//! Mutable operational ownership of a surviving controller.
//!
//! A Playback assignment can move, change priority or change its page-qualified identity while
//! its logical controller survives. The owner and priority are refreshed in place; instance
//! identity, clocks, pause, phase, Random streams and held samples are never touched.
use super::*;

impl DynamicControllerSource {
    pub fn playback(page: u8, position: u16) -> Result<Self, DynamicRuntimeError> {
        if !(1..=99).contains(&page) || !(1..=1000).contains(&position) {
            return Err(DynamicRuntimeError::InvalidController);
        }
        Ok(Self::Playback { page, position })
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
        if let DynamicControllerSource::Playback { page, position } = &source {
            DynamicControllerSource::playback(*page, *position)?;
        }
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

pub(super) fn deserialize_playback_owner_page<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u8, D::Error> {
    let page = u8::deserialize(deserializer)?;
    if (1..=99).contains(&page) {
        Ok(page)
    } else {
        Err(serde::de::Error::custom(
            "Playback page must be within 1-99",
        ))
    }
}
pub(super) fn deserialize_playback_owner_position<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u16, D::Error> {
    let position = u16::deserialize(deserializer)?;
    if (1..=1000).contains(&position) {
        Ok(position)
    } else {
        Err(serde::de::Error::custom(
            "Playback position must be within 1-1000",
        ))
    }
}
