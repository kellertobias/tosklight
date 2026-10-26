//! One retained attachment, identified by exact dependencies, never a hash-only match.
mod key;
pub(in crate::renderer) use key::Key;
#[cfg(test)]
mod tests;

#[derive(Default)]
pub(in crate::renderer) struct BeamCache {
    committed: Option<Key>,
    pending: Option<Key>,
    pub resource_generation: u64,
    #[cfg(test)]
    pub force_fresh: bool,
}
impl BeamCache {
    pub fn eligible(beams: bool, fused: bool, cloudiness: f32) -> bool {
        beams && !fused && cloudiness.is_finite() && cloudiness <= 0.001
    }
    /// Called before acquire; an abandoned encoder can never commit its identity on a later frame.
    pub fn begin_frame(&mut self) {
        self.pending = None;
    }
    pub fn invalidate(&mut self) {
        self.committed = None;
        self.pending = None;
    }
    pub fn resources_changed(&mut self) {
        self.resource_generation = self
            .resource_generation
            .checked_add(1)
            .expect("GPU resource generation exhausted");
        self.invalidate();
    }
    /// A miss will overwrite the single texture, so retire its old identity before encoding.
    pub fn prepare(&mut self, next: Key) -> bool {
        #[cfg(test)]
        let force = self.force_fresh;
        #[cfg(not(test))]
        let force = false;
        if !force && self.committed.as_ref() == Some(&next) {
            return true;
        }
        self.committed = None;
        self.pending = Some(next);
        false
    }
    /// Only the facade calls this, after queue.submit accepted the frame's command buffer.
    pub fn submitted(&mut self) {
        if let Some(next) = self.pending.take() {
            self.committed = Some(next);
        }
    }
}
