//! One latest frame per current source. A lifetime token rejects every retired producer.
use super::*;
use std::collections::HashSet;
use viz_scene::uuid::Uuid;

#[derive(Default)]
pub(super) struct Mailbox {
    active: HashMap<Uuid, Arc<AtomicBool>>,
    fallbacks: HashSet<Uuid>,
    latest: HashMap<Uuid, MediaFrame>,
    health: HashMap<Uuid, (String, bool)>,
    // Atlas sequencing survives worker replacement and removal/re-add without retaining old IDs.
    sequence: u64,
}
impl Mailbox {
    pub fn retain(&mut self, wanted: &HashMap<Uuid, &MediaSourceBinding>) {
        self.active.retain(|id, token| {
            let keep = wanted.contains_key(id);
            if !keep {
                token.store(true, Ordering::Release);
            }
            keep
        });
        self.fallbacks.retain(|id| wanted.contains_key(id));
        self.latest.retain(|id, _| wanted.contains_key(id));
        self.health.retain(|id, _| wanted.contains_key(id));
    }
    fn retire(&mut self, id: Uuid) {
        if let Some(token) = self.active.remove(&id) {
            token.store(true, Ordering::Release);
        }
        self.fallbacks.remove(&id);
        self.latest.remove(&id);
        self.health.remove(&id);
    }
    pub fn activate(&mut self, source: &MediaSourceBinding, token: Arc<AtomicBool>) {
        self.retire(source.id);
        self.active.insert(source.id, token);
        self.health.insert(source.id, (source.name.clone(), false));
    }
    fn current(&self, id: Uuid, token: &Arc<AtomicBool>) -> bool {
        !token.load(Ordering::Acquire)
            && self
                .active
                .get(&id)
                .is_some_and(|current| Arc::ptr_eq(current, token))
    }
    pub fn publish(&mut self, token: &Arc<AtomicBool>, mut frame: MediaFrame) -> bool {
        if !self.current(frame.source_id, token) {
            return false;
        }
        let Some(sequence) = self.sequence.checked_add(1) else {
            return false;
        };
        self.sequence = sequence;
        frame.sequence = sequence;
        self.set_live(frame.source_id, token, true);
        self.latest.insert(frame.source_id, frame);
        true
    }
    pub fn set_live(&mut self, id: Uuid, token: &Arc<AtomicBool>, live: bool) {
        if self.current(id, token)
            && let Some((_, state)) = self.health.get_mut(&id)
        {
            *state = live;
        }
    }
    pub fn rename(&mut self, source: &MediaSourceBinding) {
        if let Some((name, _)) = self.health.get_mut(&source.id) {
            name.clone_from(&source.name);
        }
    }
    pub fn fallback(&mut self, source: &MediaSourceBinding, rgba: &[u8]) {
        if self.active.contains_key(&source.id) {
            self.retire(source.id);
        }
        if !self.fallbacks.insert(source.id) {
            return;
        }
        let Some(sequence) = self.sequence.checked_add(1) else {
            return;
        };
        self.sequence = sequence;
        self.latest.insert(
            source.id,
            MediaFrame {
                source_id: source.id,
                sequence,
                width: EDGE,
                height: EDGE,
                rgba: rgba.to_vec(),
                persistent: true,
            },
        );
    }
    pub fn drain(&mut self) -> Vec<MediaFrame> {
        self.latest.drain().map(|(_, frame)| frame).collect()
    }
    pub fn offline_notice(&self) -> Option<String> {
        let names = self
            .health
            .values()
            .filter_map(|(name, live)| (!*live).then_some(name.as_str()))
            .collect::<Vec<_>>();
        (!names.is_empty()).then(|| {
            format!(
                "Media offline — showing fallback or black; retrying {}",
                names.join(", ")
            )
        })
    }
}
