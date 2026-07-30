use crate::{
    EngineSnapshot, MoveInBlackKey, MoveInBlackRuntime, ProfileEncodingIndex,
    ProfileProjectionIndex, ProgrammerTransition, ProgrammerTransitionKey, RuntimeGeneration,
};
use arc_swap::ArcSwap;
use light_core::{FixtureId, SharedClock};
use light_playback::PlaybackEngine;
use light_programmer::ProgrammerRegistry;
use parking_lot::{Mutex, RwLock};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

pub struct Engine {
    pub(crate) generation: ArcSwap<RuntimeGeneration>,
    pub(crate) programmers: ProgrammerRegistry,
    pub(crate) timecode_frame: AtomicU64,
    pub(crate) programmer_fade_millis: AtomicU64,
    /// Exact BPM bits. AtomicU64 keeps snapshot recompilation lock-free without rounding the
    /// operator's decimal Speed Group value to an integer.
    pub(crate) speed_groups_bpm: [AtomicU64; 5],
    pub(crate) speed_groups_paused: [AtomicBool; 5],
    pub(crate) sequence_master_fade_millis: AtomicU64,
    pub(crate) programmer_transitions:
        Mutex<HashMap<ProgrammerTransitionKey, ProgrammerTransition>>,
    dynamic_programmer_cache: Mutex<DynamicProgrammerCache>,
    pub(crate) move_in_black: Mutex<HashMap<MoveInBlackKey, MoveInBlackRuntime>>,
    pub(crate) group_master_flashes: RwLock<HashMap<String, f32>>,
    /// Live Highlight is an output overlay, not programmer/show data. Ownership and remembered
    /// selection live in the server; the engine only needs the currently lit fixture identities.
    pub(crate) highlighted_fixtures: RwLock<HashSet<FixtureId>>,
    pub(crate) clock: SharedClock,
}

#[derive(Default)]
struct DynamicProgrammerCache {
    signature: Vec<(uuid::Uuid, i16, usize, usize, usize, usize)>,
    sources: Vec<(
        uuid::Uuid,
        i16,
        Arc<Vec<light_dynamics::DynamicAddressValue>>,
        Arc<Vec<light_dynamics::DynamicAddressValue>>,
    )>,
    values: Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>>,
}

impl Engine {
    pub fn new(programmers: ProgrammerRegistry) -> Self {
        let clock = programmers.clock();
        let playback = PlaybackEngine::with_clock(Arc::clone(&clock));
        Self {
            generation: ArcSwap::from_pointee(RuntimeGeneration::new(
                EngineSnapshot::default(),
                Arc::new(RwLock::new(playback)),
                Arc::new(HashMap::new()),
                Arc::new(ProfileEncodingIndex::default()),
                Arc::new(ProfileProjectionIndex::default()),
            )),
            programmers,
            timecode_frame: AtomicU64::new(u64::MAX),
            programmer_fade_millis: AtomicU64::new(0),
            speed_groups_bpm: [
                AtomicU64::new(120.0_f64.to_bits()),
                AtomicU64::new(90.0_f64.to_bits()),
                AtomicU64::new(60.0_f64.to_bits()),
                AtomicU64::new(30.0_f64.to_bits()),
                AtomicU64::new(15.0_f64.to_bits()),
            ],
            speed_groups_paused: std::array::from_fn(|_| AtomicBool::new(false)),
            sequence_master_fade_millis: AtomicU64::new(0),
            programmer_transitions: Mutex::new(HashMap::new()),
            dynamic_programmer_cache: Mutex::new(DynamicProgrammerCache::default()),
            move_in_black: Mutex::new(HashMap::new()),
            group_master_flashes: RwLock::new(HashMap::new()),
            highlighted_fixtures: RwLock::new(HashSet::new()),
            clock,
        }
    }

    pub fn set_control_timing(
        &self,
        speed_groups_bpm: [f64; 5],
        programmer_fade_millis: u64,
        sequence_master_fade_millis: u64,
    ) {
        self.programmer_fade_millis
            .store(programmer_fade_millis.min(60_000), Ordering::Relaxed);
        let speed_groups_bpm = speed_groups_bpm.map(|bpm| {
            if bpm.is_finite() {
                bpm.clamp(0.1, 999.0)
            } else {
                120.0
            }
        });
        for (target, bpm) in self.speed_groups_bpm.iter().zip(speed_groups_bpm) {
            target.store(bpm.to_bits(), Ordering::Relaxed);
        }
        self.sequence_master_fade_millis
            .store(sequence_master_fade_millis.min(60_000), Ordering::Relaxed);
        self.generation
            .load()
            .playback()
            .write()
            .set_control_timing(speed_groups_bpm, sequence_master_fade_millis);
    }

    pub fn set_speed_groups_paused(&self, paused: [bool; 5]) {
        for (target, paused) in self.speed_groups_paused.iter().zip(paused) {
            target.store(paused, Ordering::Relaxed);
        }
        self.generation
            .load()
            .playback()
            .write()
            .set_speed_groups_paused(paused);
    }

    pub fn clear_programmer_transitions(&self) {
        self.programmer_transitions.lock().clear();
    }

    /// Returns first-class Dynamic/FAT layers for final priority-then-LTP output arbitration.
    pub fn dynamic_programmer_values(
        &self,
    ) -> Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>> {
        let sources = self.programmers.active_dynamic_sources_for_sessions();
        let signature = sources
            .iter()
            .map(|(id, priority, values, preload)| {
                (
                    *id,
                    *priority,
                    Arc::as_ptr(values) as usize,
                    values.len(),
                    Arc::as_ptr(preload) as usize,
                    preload.len(),
                )
            })
            .collect::<Vec<_>>();
        let mut cache = self.dynamic_programmer_cache.lock();
        if cache.signature != signature {
            let values: Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>> = Arc::new(
                sources
                    .iter()
                    .flat_map(|(id, priority, values, preload)| {
                        values
                            .iter()
                            .chain(preload.iter())
                            .cloned()
                            .map(move |value| (*id, *priority, value))
                    })
                    .collect(),
            );
            cache.values = values;
            // Retaining the source Arcs is part of the cache identity contract: subsequent
            // Arc::make_mut calls must allocate a new source even when the entry count is
            // unchanged, so a same-length value edit cannot leave this projection stale.
            cache.sources = sources;
            cache.signature = signature;
        }
        Arc::clone(&cache.values)
    }
}
