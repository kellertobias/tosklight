use super::*;

/// One temporary output effect. Identity is unique for the lease's lifetime; there is no
/// wrapping counter and no reference from the control state back into its owning capability.
pub(in crate::runtime) struct OutputTransitionOverlay {
    pub(in crate::runtime) owner: Arc<()>,
    pub(in crate::runtime) hold: bool,
    pub(in crate::runtime) blackout: bool,
    pub(in crate::runtime) fade_gain: Option<f32>,
}

/// Owns only temporary output effects. No mutex guard crosses an await.
/// Move this lease with any admitted worker that can still publish after request cancellation.
#[must_use = "dropping the transition lease removes its temporary output effects"]
pub(in crate::runtime) struct OutputTransitionLease {
    control: OutputControlCapability,
    owner: Arc<()>,
    grand_master_write: Arc<()>,
    blackout_write: Arc<()>,
}

impl OutputResource {
    pub(in crate::runtime) fn begin_transition_hold(&self) -> OutputTransitionLease {
        self.begin_transition(true, false, None)
    }

    pub(in crate::runtime) fn begin_transition_blackout(&self) -> OutputTransitionLease {
        self.begin_transition(false, true, None)
    }

    pub(in crate::runtime) fn begin_transition_fade(&self) -> OutputTransitionLease {
        self.begin_transition(false, false, Some(1.0))
    }

    fn begin_transition(
        &self,
        hold: bool,
        blackout: bool,
        fade_gain: Option<f32>,
    ) -> OutputTransitionLease {
        let mut control = self.control.lock();
        let owner = Arc::new(());
        let lease = OutputTransitionLease {
            control: self.control.clone(),
            owner: Arc::clone(&owner),
            grand_master_write: Arc::clone(&control.grand_master_write),
            blackout_write: Arc::clone(&control.blackout_write),
        };
        control.transitions.push(OutputTransitionOverlay {
            owner,
            hold,
            blackout,
            fade_gain,
        });
        lease
    }
}

impl OutputTransitionLease {
    /// Changes only this fade's multiplier. Invalid input leaves the previous gain intact.
    pub(in crate::runtime) fn set_fade_gain(&self, gain: f32) -> bool {
        if !gain.is_finite() || !(0.0..=1.0).contains(&gain) {
            return false;
        }
        let mut control = self.control.lock();
        let Some(overlay) = control
            .transitions
            .iter_mut()
            .find(|overlay| Arc::ptr_eq(&overlay.owner, &self.owner))
        else {
            return false;
        };
        let Some(current) = overlay.fade_gain.as_mut() else {
            return false;
        };
        *current = gain;
        true
    }

    /// Installs destination base controls only where no base write has intervened since begin.
    /// Equal-value operator writes count. Overlay effects and flash are never persisted here.
    /// Call inside the activation commit; this synchronous merge has no external lock acquisition.
    pub(in crate::runtime) fn restore_destination_control(&self, runtime: &PersistedOutputRuntime) {
        let mut control = self.control.lock();
        let restore_master = Arc::ptr_eq(&control.grand_master_write, &self.grand_master_write);
        let restore_blackout = Arc::ptr_eq(&control.blackout_write, &self.blackout_write);
        if restore_master {
            control.options.grand_master = runtime.grand_master;
            control.grand_master_write = Arc::new(());
        }
        if restore_blackout {
            control.options.blackout = runtime.blackout;
            control.blackout_write = Arc::new(());
        }
        if restore_master && restore_blackout {
            control.revision = runtime.revision;
        }
    }
}

impl Drop for OutputTransitionLease {
    fn drop(&mut self) {
        let mut control = self.control.lock();
        control
            .transitions
            .retain(|overlay| !Arc::ptr_eq(&overlay.owner, &self.owner));
    }
}
