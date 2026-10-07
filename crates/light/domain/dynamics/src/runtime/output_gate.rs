use super::*;

/// Output influence independently of controller activation, transport, and destructive Off.
/// Gate timing follows the output clock even when the underlying motion is paused.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicControllerOutputGateSnapshot {
    pub started_at_millis: u64,
    pub delay_millis: u64,
    pub duration_millis: u64,
    pub from: f32,
    pub to: f32,
}

impl DynamicControllerOutputGateSnapshot {
    pub fn mix_at(self, now_millis: u64) -> f32 {
        let start = self.started_at_millis.saturating_add(self.delay_millis);
        if now_millis < start {
            return self.from;
        }
        if self.duration_millis == 0 {
            return self.to;
        }
        let progress =
            (now_millis.saturating_sub(start) as f64 / self.duration_millis as f64).clamp(0.0, 1.0);
        (f64::from(self.from) + (f64::from(self.to) - f64::from(self.from)) * progress) as f32
    }

    pub(super) fn validate(self) -> Result<(), DynamicRuntimeError> {
        if !self.from.is_finite()
            || !(0.0..=1.0).contains(&self.from)
            || !matches!(self.to, 0.0 | 1.0)
        {
            return Err(DynamicRuntimeError::InvalidSnapshot(
                "Dynamic output gate requires finite unit gain and an enabled/disabled endpoint"
                    .into(),
            ));
        }
        Ok(())
    }
}

impl DynamicRuntime {
    /// Fade output without stopping the logical source. A completed disabled gate keeps its
    /// controller, phase, Random stream, retained expressions, and pause state alive. Sampling
    /// continues at zero influence so lifting the overlay reveals the continuing motion.
    ///
    /// Repeating the same endpoint and timing is a no-op; it does not restart the fade. A new
    /// endpoint starts from the current gate influence, including an interrupted transition.
    /// A gate does not cancel destructive Off: callers retaining a previously releasing source
    /// must explicitly cancel that release before applying the gate.
    pub fn set_controller_output_enabled(
        &mut self,
        controller_id: Uuid,
        enabled: bool,
        now_millis: u64,
        delay_millis: u64,
        duration_millis: u64,
    ) -> Result<bool, DynamicRuntimeError> {
        let (instance_id, _) = self
            .controller(controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?;
        let current = self.instances[&instance_id]
            .controller_transitions
            .get(&controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?
            .output_gate;
        let to = if enabled { 1.0 } else { 0.0 };
        if current.is_none() && enabled {
            return Ok(false);
        }
        if current.is_some_and(|gate| {
            gate.to == to
                && gate.delay_millis == delay_millis
                && gate.duration_millis == duration_millis
        }) {
            return Ok(false);
        }
        if enabled && delay_millis == 0 && duration_millis == 0 {
            return self.clear_controller_output_gate(controller_id);
        }
        let gate = DynamicControllerOutputGateSnapshot {
            started_at_millis: now_millis,
            delay_millis,
            duration_millis,
            from: current.map_or(1.0, |gate| gate.mix_at(now_millis)),
            to,
        };
        self.journal_instance(instance_id);
        self.instances
            .get_mut(&instance_id)
            .expect("controller was resolved above")
            .controller_transitions
            .get_mut(&controller_id)
            .expect("transition was resolved above")
            .output_gate = Some(gate);
        Ok(true)
    }

    /// Remove only the output overlay, revealing the same running source immediately.
    pub fn clear_controller_output_gate(
        &mut self,
        controller_id: Uuid,
    ) -> Result<bool, DynamicRuntimeError> {
        let (instance_id, _) = self
            .controller(controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?;
        if self.instances[&instance_id]
            .controller_transitions
            .get(&controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?
            .output_gate
            .is_none()
        {
            return Ok(false);
        }
        self.journal_instance(instance_id);
        self.instances
            .get_mut(&instance_id)
            .expect("controller was resolved above")
            .controller_transitions
            .get_mut(&controller_id)
            .expect("transition was resolved above")
            .output_gate = None;
        Ok(true)
    }
}
