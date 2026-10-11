use crate::*;

impl PlaybackEngine {
    /// Preserve held output while preventing its release from reviving a Solo peer.
    pub fn inhibit_held_release_activation(
        &mut self,
        address: PlaybackAddress,
    ) -> Result<PlaybackMutation<()>, String> {
        self.definition(address).ok_or("Playback does not exist")?;
        let cue_key = self.runtime_key(address).ok();
        let cue_holds = self
            .cuelist_flash_states
            .keys()
            .chain(self.cuelist_swap_states.keys())
            .filter(|held| cue_key.is_some() && self.runtime_key(held.address).ok() == cue_key)
            .cloned()
            .collect::<HashSet<_>>();
        let target = self.dynamic_target_id(address);
        let dynamic_holds = self
            .dynamic_flash_states
            .keys()
            .filter(|held| target.is_some() && self.dynamic_target_id(held.address) == target)
            .cloned()
            .collect::<Vec<_>>();
        let mut changed = false;
        for held in cue_holds {
            for states in [
                &mut self.cuelist_flash_states,
                &mut self.cuelist_swap_states,
            ] {
                if let Some(state) = states.get_mut(&held) {
                    changed |= !state.release_activation_inhibited;
                    state.release_activation_inhibited = true;
                }
            }
        }
        for held in dynamic_holds {
            if let Some(state) = self.dynamic_flash_states.get_mut(&held) {
                changed |= !state.release_activation_inhibited;
                state.release_activation_inhibited = true;
            }
        }
        Ok(PlaybackMutation::new(
            (),
            if changed {
                PlaybackRuntimeEffect::Transient
            } else {
                PlaybackRuntimeEffect::None
            },
        ))
    }
    pub fn cancel_control_source(
        &mut self,
        source: &PlaybackControlSource,
    ) -> PlaybackMutation<()> {
        self.cancel_controls(|candidate| candidate == source)
    }
    pub fn cancel_control_surface(&mut self, surface_id: &str) -> PlaybackMutation<()> {
        self.cancel_controls(|candidate| candidate.surface_id == surface_id)
    }
    fn cancel_controls(
        &mut self,
        matches: impl Fn(&PlaybackControlSource) -> bool,
    ) -> PlaybackMutation<()> {
        let before = self.temporary.len()
            + self.swap_held.len()
            + self.cuelist_flash_states.len()
            + self.cuelist_swap_states.len()
            + self.dynamic_flash_states.len()
            + self.control_states.len();
        let permanent = self
            .active_dynamics
            .iter()
            .map(|(id, active)| (*id, active.enabled && !active.flash_restore_off))
            .collect::<HashMap<_, _>>();
        self.temporary.retain(|(held, _), _| !matches(&held.source));
        self.swap_held.retain(|held| !matches(&held.source));
        self.cuelist_flash_states
            .retain(|held, _| !matches(&held.source));
        self.cuelist_swap_states
            .retain(|held, _| !matches(&held.source));
        self.dynamic_flash_states
            .retain(|held, _| !matches(&held.source));
        self.control_states.retain(|held, _| !matches(&held.source));
        let held_targets = self
            .dynamic_flash_states
            .keys()
            .filter_map(|held| self.dynamic_target_id(held.address))
            .collect::<HashSet<_>>();
        for (id, active) in &mut self.active_dynamics {
            let held = held_targets.contains(id);
            let permanent = permanent[id];
            active.flash = held;
            active.flash_restore_off = held && !permanent;
            active.enabled = held || permanent;
        }
        let after = self.temporary.len()
            + self.swap_held.len()
            + self.cuelist_flash_states.len()
            + self.cuelist_swap_states.len()
            + self.dynamic_flash_states.len()
            + self.control_states.len();
        PlaybackMutation::new(
            (),
            if before != after {
                PlaybackRuntimeEffect::Transient
            } else {
                PlaybackRuntimeEffect::None
            },
        )
    }
}
