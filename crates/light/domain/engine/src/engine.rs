use crate::{
    EngineSnapshot, OutputContinuityState, ProfileEncodingIndex, ProfileProjectionIndex,
    RuntimeGeneration,
};
use arc_swap::ArcSwap;
use chrono::{DateTime, Utc};
use light_core::{FixtureId, SharedClock, Xyz};
use light_fixture::{HighlightLook, HighlightLookCompatibility};
use light_output::DmxFrame;
use light_playback::PlaybackEngine;
use light_programmer::{ActiveDynamicSessionSource, HighlightOutputLayer, ProgrammerRegistry};
use parking_lot::{Mutex, RwLock};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

pub struct Engine {
    supported_programming_contract: u16,
    pub(crate) generation: ArcSwap<RuntimeGeneration>,
    pub(crate) programmers: ProgrammerRegistry,
    pub(crate) timecode_frame: AtomicU64,
    pub(crate) programmer_fade_millis: AtomicU64,
    /// Exact BPM bits. AtomicU64 keeps snapshot recompilation lock-free without rounding the
    /// operator's decimal Speed Group value to an integer.
    pub(crate) speed_groups_bpm: [AtomicU64; 5],
    pub(crate) speed_groups_paused: [AtomicBool; 5],
    pub(crate) sequence_master_fade_millis: AtomicU64,
    pub(crate) release_fade_millis: AtomicU64,
    pub(crate) output_continuity: Mutex<OutputContinuityState>,
    dynamic_programmer_cache: Mutex<DynamicProgrammerCache>,
    /// Where each Programmer's stored values live in the current frame, remembered against the
    /// registry's shared value vectors so an unchanged Programmer costs no lookup by name.
    pub(crate) programmer_addresses: Mutex<crate::programmer_resolution::ProgrammerAddressMemo>,
    /// TL-639: unchanged Programmer evaluations reused across static resolutions and frames.
    pub(crate) programmer_memo: Mutex<crate::programmer_memo::ProgrammerContributionMemo>,
    pub(crate) programmer_releases: Mutex<crate::programmer_release::ProgrammerReleaseMemo>,
    pub(crate) preload_sources: Mutex<crate::preload_sources::PreloadSourceMemo>,
    pub(crate) group_master_flashes: RwLock<HashMap<String, f32>>,
    pub(crate) group_master_transitions: Mutex<HashMap<String, GroupMasterTransition>>,
    /// Runtime-only Group color intent. It deliberately stays out of the portable show and desk
    /// persistence; external controllers must restore it after a process restart.
    pub(crate) group_colors: RwLock<HashMap<String, GroupColorContribution>>,
    /// Live Highlight is an output overlay, not programmer/show data. Ownership and remembered
    /// selection live in the server; the engine only needs the currently lit fixture identities.
    pub(crate) highlight_layers: RwLock<HashMap<FixtureId, HighlightOutputLayer>>,
    /// What an external tracking source holds outright. Live, unpersisted, and applied last: see
    /// `tracked_positions`.
    pub(crate) tracking_frame: RwLock<Arc<crate::TrackedInputFrame>>,
    /// Installation-owned Highlight intent. A bare engine starts in review-required compatibility
    /// mode so callers that have not installed desk configuration retain exact legacy raw output.
    pub(crate) highlight_look: RwLock<HighlightLook>,
    /// The active show's colour programming model: set when a show is installed, read by every
    /// frame. `true` is Color Intent.
    pub(crate) color_intent: AtomicBool,
    /// Working room a frame borrows and hands back, so the vectors a render fills are grown once
    /// rather than every tick. Held under one lock because only one render fills them at a time.
    pub(crate) scratch: Arc<Mutex<FrameScratch>>,
    pub(crate) preload_playback_scratch: Arc<Mutex<FrameScratch>>,
    /// Where a frame's published output waits between frames. These leave the engine and are held
    /// by whoever reads them, so they cannot be overwritten — they are borrowed and returned.
    pub(crate) universe_pool: Arc<crate::ValuePool<HashMap<light_core::Universe, DmxFrame>>>,
    pub(crate) patched_slot_pool: Arc<crate::ValuePool<HashMap<light_core::Universe, u16>>>,
    pub(crate) profile_scratch_pool: Arc<crate::ValuePool<crate::ResolvedProfileFixtureOutput>>,
    pub(crate) visualization_pool: Arc<crate::ValuePool<crate::ResolvedValues>>,
    pub(crate) clock: SharedClock,
    /// Threads a frame may split its independent per-fixture work over (TL-639 round 5); one
    /// keeps every frame on the caller. Never changes an output value.
    output_workers: AtomicUsize,
    /// Per-chunk resolved fixture outputs of a parallel render, kept between frames.
    pub(crate) render_chunks: Mutex<crate::render_fixtures::RenderChunks>,
    /// The ranges and first-write lists of a parallel resolution's offers, kept between frames
    /// (TL-639 round 7).
    pub(crate) offer_scratch: Mutex<crate::contribution::OfferScratch>,
    /// The output worker pool, built for the current worker count on first use.
    output_pool: Mutex<Option<Arc<crate::parallel::OutputPool>>>,
}

/// The vectors a render fills and empties again.
#[derive(Default)]
pub(crate) struct FrameScratch {
    pub(crate) playback_evidence: crate::contribution::PlaybackEvidenceCache,
    pub(crate) playback: Vec<light_playback::PlaybackContribution>,
    pub(crate) contributions: Vec<crate::EngineContribution>,
}

#[derive(Default)]
struct DynamicProgrammerCache {
    signature: Vec<(uuid::Uuid, i16, usize, usize, usize, usize)>,
    sources: Vec<ActiveDynamicSessionSource>,
    values: Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>>,
    rows: Arc<Vec<crate::CapturedDynamicProgrammerRow>>,
}

impl Engine {
    /// How many frame buffers the current generation has had to build rather than reuse.
    #[cfg(test)]
    pub(crate) fn frames_built(&self) -> usize {
        self.generation.load().frames().built()
    }

    /// Where the current patch generation keeps each fixture's attributes, for producers that
    /// emit the same pairs every tick and would rather not have them looked up by name each time.
    pub fn frame_addresser(&self) -> crate::FrameAddresser {
        crate::FrameAddresser::new(std::sync::Arc::clone(self.generation.load().slots()))
    }

    pub fn cue_timing_masters(&self) -> (u64, u64) {
        (
            self.sequence_master_fade_millis.load(Ordering::Relaxed),
            self.release_fade_millis.load(Ordering::Relaxed),
        )
    }

    pub fn new(programmers: ProgrammerRegistry) -> Self {
        Self::with_programming_contract_support(
            programmers,
            light_core::programming::PROGRAMMING_CONTRACT_VERSION,
        )
    }

    /// Domain tests can exercise a newer authoring contract independently of production frame
    /// activation. Production passes the version its complete resolver can actually execute.
    pub fn with_programming_contract_support(
        programmers: ProgrammerRegistry,
        supported_programming_contract: u16,
    ) -> Self {
        let clock = programmers.clock();
        let playback = PlaybackEngine::with_clock(Arc::clone(&clock));
        Self {
            supported_programming_contract,
            programmer_releases: Mutex::default(),
            preload_sources: Mutex::default(),
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
            release_fade_millis: AtomicU64::new(0),
            output_continuity: Mutex::new(OutputContinuityState::default()),
            dynamic_programmer_cache: Mutex::new(DynamicProgrammerCache::default()),
            programmer_addresses: Mutex::new(Default::default()),
            programmer_memo: Mutex::new(Default::default()),
            group_master_flashes: RwLock::new(HashMap::new()),
            group_master_transitions: Mutex::new(HashMap::new()),
            group_colors: RwLock::new(HashMap::new()),
            highlight_layers: RwLock::new(HashMap::new()),
            tracking_frame: RwLock::new(Arc::default()),
            highlight_look: RwLock::new(HighlightLook {
                compatibility: HighlightLookCompatibility::NeedsReview,
                ..HighlightLook::default()
            }),
            color_intent: AtomicBool::new(false),
            scratch: Arc::new(Mutex::new(FrameScratch::default())),
            preload_playback_scratch: Arc::new(Mutex::new(FrameScratch::default())),
            universe_pool: Arc::default(),
            patched_slot_pool: Arc::default(),
            profile_scratch_pool: Arc::default(),
            visualization_pool: Arc::default(),
            clock,
            output_workers: AtomicUsize::new(crate::parallel::default_output_workers()),
            render_chunks: Mutex::default(),
            offer_scratch: Mutex::default(),
            output_pool: Mutex::default(),
        }
    }

    /// Threads a frame may split its independent per-fixture work over; at least one.
    pub fn output_workers(&self) -> usize {
        self.output_workers.load(Ordering::Relaxed).max(1)
    }

    /// Set the output worker count (at least one). Outputs are identical for every count.
    pub fn set_output_workers(&self, workers: usize) {
        self.output_workers.store(workers.max(1), Ordering::Relaxed);
    }

    /// The output worker pool for the current worker count; `None` with one worker.
    pub fn output_pool(&self) -> Option<Arc<crate::parallel::OutputPool>> {
        let workers = self.output_workers();
        if workers <= 1 {
            return None;
        }
        let mut pool = self.output_pool.lock();
        if pool.as_ref().is_none_or(|pool| pool.workers() != workers) {
            *pool = crate::parallel::OutputPool::new(workers).map(Arc::new);
        }
        pool.clone()
    }

    pub fn supported_programming_contract(&self) -> u16 {
        self.supported_programming_contract
    }

    pub fn set_control_timing(
        &self,
        speed_groups_bpm: [f64; 5],
        programmer_fade_millis: u64,
        sequence_master_fade_millis: u64,
        release_fade_millis: u64,
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
        self.release_fade_millis
            .store(release_fade_millis.min(60_000), Ordering::Relaxed);
        self.generation
            .load()
            .playback()
            .write()
            .set_control_timing(
                speed_groups_bpm,
                sequence_master_fade_millis,
                release_fade_millis,
            );
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
        let mut continuity = self.output_continuity.lock();
        continuity.advance_revision();
        continuity.programmer_transitions.clear();
    }

    /// Returns first-class Dynamic/FAT layers for final priority-then-LTP output arbitration.
    pub fn dynamic_programmer_values(
        &self,
    ) -> Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>> {
        let sources = self.programmers.active_dynamic_sources_for_sessions();
        self.dynamic_programmer_values_from_sources(sources)
    }

    /// Cold startup verifies saved owners before any control surface reconnects. This does
    /// not connect the Programmer or change the active-source policy used by output ticks.
    pub fn retained_dynamic_programmer_values(
        &self,
    ) -> Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>> {
        let sources = self
            .programmers
            .retained_dynamic_source()
            .into_iter()
            .collect();
        self.dynamic_programmer_values_from_sources(sources)
    }

    /// Holds this Engine's retained Programmer authority for a complete cold-owner operation.
    ///
    /// The source may exist before any surface reconnects. This does not activate it or include
    /// pending Preload edits. Acquire this Programmer gate before any desk-interaction gate and
    /// keep the operation synchronous; it shares the registry's reentrant mutation boundary.
    pub fn with_retained_dynamic_programmer_source<R>(
        &self,
        operation: impl FnOnce(Option<light_programmer::ActiveDynamicSessionSource>) -> R,
    ) -> R {
        self.programmers
            .serialized(|| operation(self.programmers.retained_dynamic_source()))
    }

    pub(crate) fn dynamic_programmer_values_from_sources(
        &self,
        sources: Vec<ActiveDynamicSessionSource>,
    ) -> Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>> {
        self.dynamic_programmer_pair_from_sources(sources).0
    }

    /// One immutable tuple/row pair from the same Programmer capture and Arc memo. Both vectors
    /// keep stable identity across unchanged frames so downstream reconciliation can stay warm.
    pub fn captured_dynamic_programmer_values_from_sources(
        &self,
        sources: Vec<ActiveDynamicSessionSource>,
    ) -> (
        Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>>,
        Arc<Vec<crate::CapturedDynamicProgrammerRow>>,
    ) {
        self.dynamic_programmer_pair_from_sources(sources)
    }

    fn dynamic_programmer_pair_from_sources(
        &self,
        sources: Vec<ActiveDynamicSessionSource>,
    ) -> (
        Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>>,
        Arc<Vec<crate::CapturedDynamicProgrammerRow>>,
    ) {
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
            let (values, rows) =
                crate::preload_sources::capture_dynamic_programmer_rows_from_sources(&sources);
            cache.values = values;
            cache.rows = rows;
            // Retaining the source Arcs is part of the cache identity contract: subsequent
            // Arc::make_mut calls must allocate a new source even when the entry count is
            // unchanged, so a same-length value edit cannot leave this projection stale.
            cache.sources = sources;
            cache.signature = signature;
        }
        (Arc::clone(&cache.values), Arc::clone(&cache.rows))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GroupMasterTransition {
    pub(crate) from: f32,
    pub(crate) to: f32,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) duration_millis: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct GroupColorContribution {
    pub(crate) color: Xyz,
    pub(crate) changed_at: DateTime<Utc>,
}
