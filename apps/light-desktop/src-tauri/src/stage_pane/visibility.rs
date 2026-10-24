//! Independent web/native visibility; keyboard focus never determines Stage demand.
use super::{PaneRect, Running, StagePane, ToHelper};

pub(super) struct PaneVisibility {
    pub web: bool,
    pub window: bool,
    geometry: bool,
    last_sent: Option<bool>,
}
impl PaneVisibility {
    pub fn new(pane: PaneRect, window: bool) -> Self {
        Self {
            web: true,
            window,
            geometry: valid_area(pane),
            last_sent: None,
        }
    }
    pub fn set_geometry(&mut self, pane: PaneRect) {
        self.geometry = valid_area(pane);
    }
    pub fn visible(&self) -> bool {
        self.web && self.window && self.geometry
    }
    fn transition(&mut self, producer_visible: bool) -> Option<bool> {
        let next = self.visible() && producer_visible;
        if self.last_sent == Some(next) {
            return None;
        }
        self.last_sent = Some(next);
        Some(next)
    }
}
fn valid_area(pane: PaneRect) -> bool {
    [pane.x, pane.y, pane.width, pane.height]
        .iter()
        .all(|v| v.is_finite())
        && pane.width > 0.0
        && pane.height > 0.0
}
impl Running {
    pub(super) fn refresh_visibility(&mut self) {
        self.compositor.set_window_visible(self.visibility.window);
        self.compositor.set_visible(self.visibility.visible());
        if let Some(visible) = self
            .visibility
            .transition(self.compositor.producer_visible())
        {
            self.send(&ToHelper::Visibility { visible });
        }
        for frame in self.compositor.drain_released_frames() {
            self.send(&ToHelper::ReleaseFrame { frame });
        }
    }
}
impl StagePane {
    pub(super) fn set_web_visible(&self, visible: bool) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        if let Some(running) = guard.as_mut() {
            running.visibility.web = visible;
            running.refresh_visibility();
        }
        Ok(())
    }
    pub(super) fn set_window_visible(&self, visible: bool) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        if let Some(running) = guard.as_mut() {
            running.visibility.window = visible;
            running.refresh_visibility();
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pane() -> PaneRect {
        PaneRect {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 360.0,
        }
    }
    #[test]
    fn every_visibility_source_and_occlusion_can_stop_the_producer() {
        let mut state = PaneVisibility::new(pane(), true);
        assert_eq!(state.transition(true), Some(true));
        assert_eq!(state.transition(true), None);
        state.web = false;
        assert_eq!(state.transition(true), Some(false));
        state.window = false;
        state.web = true;
        assert_eq!(state.transition(true), None);
        state.window = true;
        assert_eq!(state.transition(false), None);
        assert_eq!(state.transition(true), Some(true));
    }
    #[test]
    fn collapsed_and_invalid_geometry_stays_quiet_until_a_real_rectangle_returns() {
        let mut state = PaneVisibility::new(PaneRect::default(), true);
        assert_eq!(state.transition(true), Some(false));
        state.set_geometry(PaneRect {
            width: f32::NAN,
            ..pane()
        });
        assert_eq!(state.transition(true), None);
        state.set_geometry(pane());
        assert_eq!(state.transition(true), Some(true));
    }
}
