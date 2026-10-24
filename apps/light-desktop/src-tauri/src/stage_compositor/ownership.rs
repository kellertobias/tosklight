use viz_helper::protocol::FrameId;

const MAX_RELEASES: usize = 16;

#[derive(Clone)]
pub(super) struct Picture<T> {
    pub frame: FrameId,
    pub shared: bool,
    pub credit_released: bool,
    pub data: T,
}

/// A retained shared front owns its producer credit until a replacement finishes on the GPU.
/// Copied fronts retain only our texture once their first composition finishes.
pub(super) struct Exchange<T> {
    pub front: Option<Picture<T>>,
    pub ready: Option<Picture<T>>,
    flight: Option<Option<Picture<T>>>,
    clearing: bool,
    released: Vec<FrameId>,
}

impl<T> Default for Exchange<T> {
    fn default() -> Self {
        Self {
            front: None,
            ready: None,
            flight: None,
            clearing: false,
            released: Vec::new(),
        }
    }
}

impl<T: Clone> Exchange<T> {
    pub fn busy(&self) -> bool {
        self.flight.is_some()
    }

    pub fn candidate(&self) -> Option<&Picture<T>> {
        self.ready
            .as_ref()
            .or_else(|| (!self.clearing).then_some(self.front.as_ref()).flatten())
    }

    pub fn owns_slot(&self, frame: FrameId) -> bool {
        [
            &self.front,
            &self.ready,
            self.flight.as_ref().unwrap_or(&None),
        ]
        .into_iter()
        .flatten()
        .any(|picture| {
            !picture.credit_released
                && picture.frame.generation == frame.generation
                && picture.frame.slot == frame.slot
        })
    }

    pub fn release(&mut self, frame: FrameId) -> Result<(), String> {
        if self.released.contains(&frame) {
            return Ok(());
        }
        if self.released.len() >= MAX_RELEASES {
            return Err("stage frame release queue overflow".to_owned());
        }
        self.released.push(frame);
        Ok(())
    }

    pub fn replace_ready(&mut self, picture: Picture<T>) -> Result<(), String> {
        if self.owns_slot(picture.frame) {
            return Err("producer reused a Stage slot before release".to_owned());
        }
        if let Some(previous) = self.ready.take() {
            self.release(previous.frame)?;
        }
        self.ready = Some(picture);
        Ok(())
    }

    pub fn discard_ready(&mut self) -> Result<(), String> {
        if let Some(previous) = self.ready.take() {
            self.release(previous.frame)?;
        }
        Ok(())
    }

    pub fn submit(&mut self, picture: Option<Picture<T>>) -> Result<(), String> {
        if self.busy() {
            return Err("Stage composition already in flight".to_owned());
        }
        if self.ready.as_ref().is_some_and(|ready| {
            picture
                .as_ref()
                .is_some_and(|selected| selected.frame == ready.frame)
        }) {
            self.ready = None;
        }
        self.flight = Some(picture);
        Ok(())
    }

    pub fn complete(&mut self) -> Result<(), String> {
        let Some(mut selected) = self.flight.take() else {
            return Ok(());
        };
        if self.clearing {
            if let Some(selected) = selected.take()
                && !selected.credit_released
            {
                self.release(selected.frame)?;
            }
        }
        if let Some(previous) = self.front.take()
            && !previous.credit_released
            && selected
                .as_ref()
                .is_none_or(|next| next.frame != previous.frame)
        {
            self.release(previous.frame)?;
        }
        if let Some(picture) = selected.as_mut()
            && !picture.shared
            && !picture.credit_released
        {
            self.release(picture.frame)?;
            picture.credit_released = true;
        }
        self.front = selected;
        self.clearing = false;
        Ok(())
    }

    pub fn complete_preserving_front(&mut self) -> Result<(), String> {
        if self.clearing {
            return self.complete();
        }
        match self.flight.take() {
            Some(None) => Ok(()),
            other => {
                self.flight = other;
                Err("Stage suspension completed a nonblank composition".to_owned())
            }
        }
    }

    pub fn clear(&mut self) -> Result<(), String> {
        self.discard_ready()?;
        self.clearing = true;
        if !self.busy() {
            if let Some(previous) = self.front.take()
                && !previous.credit_released
            {
                self.release(previous.frame)?;
            }
            self.clearing = false;
        }
        Ok(())
    }

    pub fn drain_released(&mut self) -> Vec<FrameId> {
        std::mem::take(&mut self.released)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspension_blank_preserves_shared_front_and_its_producer_credit() {
        let mut exchange = Exchange::default();
        let first = picture(1, 0, true);
        exchange.replace_ready(first.clone()).unwrap();
        exchange.submit(exchange.candidate().cloned()).unwrap();
        exchange.complete().unwrap();
        exchange.submit(None).unwrap();
        exchange.complete_preserving_front().unwrap();
        assert_eq!(exchange.front.as_ref().unwrap().frame, first.frame);
        assert!(exchange.drain_released().is_empty());
        exchange.clear().unwrap();
        assert_eq!(exchange.drain_released(), vec![first.frame]);
    }
    fn picture(sequence: u64, slot: u8, shared: bool) -> Picture<()> {
        Picture {
            frame: FrameId {
                generation: 1,
                slot,
                sequence,
            },
            shared,
            credit_released: false,
            data: (),
        }
    }

    #[test]
    fn shared_front_stays_held_until_replacement_gpu_completion() {
        let mut exchange = Exchange::default();
        let first = picture(1, 0, true);
        exchange.replace_ready(first.clone()).unwrap();
        exchange.submit(exchange.candidate().cloned()).unwrap();
        assert!(exchange.front.is_none());
        assert!(exchange.drain_released().is_empty());
        exchange.complete().unwrap();
        let next = picture(2, 1, true);
        exchange.replace_ready(next.clone()).unwrap();
        exchange.submit(exchange.candidate().cloned()).unwrap();
        assert_eq!(exchange.front.as_ref().unwrap().frame, first.frame);
        assert!(exchange.drain_released().is_empty());
        exchange.complete().unwrap();
        assert_eq!(exchange.drain_released(), vec![first.frame]);
        assert_eq!(exchange.front.as_ref().unwrap().frame, next.frame);
    }

    #[test]
    fn copy_credit_is_released_after_gpu_and_only_once_on_redraw() {
        let mut exchange = Exchange::default();
        let copy = picture(1, 0, false);
        exchange.replace_ready(copy.clone()).unwrap();
        exchange.submit(exchange.candidate().cloned()).unwrap();
        assert!(exchange.drain_released().is_empty());
        exchange.complete().unwrap();
        assert_eq!(exchange.drain_released(), vec![copy.frame]);
        exchange.submit(exchange.candidate().cloned()).unwrap();
        exchange.complete().unwrap();
        assert!(exchange.drain_released().is_empty());
    }

    #[test]
    fn hidden_ready_discard_and_clear_never_release_an_inflight_shared_front_early() {
        let mut exchange = Exchange::default();
        let first = picture(1, 0, true);
        exchange.replace_ready(first.clone()).unwrap();
        exchange.submit(exchange.candidate().cloned()).unwrap();
        exchange.clear().unwrap();
        assert!(exchange.drain_released().is_empty());
        exchange.complete().unwrap();
        assert_eq!(exchange.drain_released(), vec![first.frame]);
        assert!(exchange.front.is_none());
        let hidden = picture(2, 1, true);
        exchange.replace_ready(hidden.clone()).unwrap();
        exchange.discard_ready().unwrap();
        assert_eq!(exchange.drain_released(), vec![hidden.frame]);
    }

    #[test]
    fn bounds_flight_and_release_storage_and_rejects_early_reuse() {
        let mut exchange = Exchange::default();
        exchange.replace_ready(picture(1, 0, true)).unwrap();
        exchange.submit(exchange.candidate().cloned()).unwrap();
        assert!(exchange.submit(None).is_err());
        assert!(exchange.replace_ready(picture(2, 0, true)).is_err());
        for sequence in 1..=MAX_RELEASES as u64 {
            exchange
                .release(FrameId {
                    generation: 2,
                    slot: 1,
                    sequence,
                })
                .unwrap();
        }
        assert!(
            exchange
                .release(FrameId {
                    generation: 2,
                    slot: 1,
                    sequence: 99
                })
                .is_err()
        );
    }
}
