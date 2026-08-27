use crate::light_benchmark::statistics::{Distribution, distribution};
use light_application::show_compiler::{
    prepare_normalized_show_candidate_incremental, prepare_show_candidate,
};
use light_core::{CueListId, FixtureId};
use light_fixture::{FixtureProfile, PatchedFixture};
use light_playback::{Cue, CueList, CueListMode, IntensityPriorityMode, RestartMode, WrapMode};
use light_show::{PortableShowDocument, ShowStore};
use serde::Serialize;
use serde_json::json;
use std::{hint::black_box, sync::Arc, time::Instant};
use uuid::Uuid;

const SMALL_FIXTURES: usize = 120;
const LARGE_FIXTURES: usize = 1_200;
const CUE_LISTS: usize = 100;
const GROUPS: usize = 100;
const PRESETS: usize = 400;
const SAMPLES: usize = 200;
const WARMUPS: usize = 20;
const ABSOLUTE_P95_MICROSECONDS: f64 = 5_000.0;

#[derive(Debug, Serialize)]
pub struct ShowMutationReport {
    pub object_count: usize,
    pub small_fixture_count: usize,
    pub large_fixture_count: usize,
    pub small: Distribution,
    pub large: Distribution,
    pub untouched_projections_shared: bool,
    pub full_compile_equivalent: bool,
    pub size_independent: bool,
    pub absolute_latency_met: bool,
    pub gate_met: bool,
}

pub fn run() -> Result<ShowMutationReport, String> {
    let (_store, document) = generated_document()?;
    let previous = prepare_show_candidate(&document, document.transaction())
        .map_err(|error| error.message)?
        .into_parts()
        .1;
    let small = with_fixtures(&previous, SMALL_FIXTURES)?;
    let large = with_fixtures(&previous, LARGE_FIXTURES)?;
    let small_samples = samples(&document, &small)?;
    let large_samples = samples(&document, &large)?;
    let small_distribution = distribution(&small_samples).expect("mutation samples are non-empty");
    let large_distribution = distribution(&large_samples).expect("mutation samples are non-empty");

    let transaction = cue_transaction(&document, "equivalence");
    let incremental =
        prepare_normalized_show_candidate_incremental(&document, transaction.clone(), &large)
            .map_err(|error| error.message)?
            .into_parts()
            .1;
    let full = prepare_show_candidate(&document, transaction)
        .map_err(|error| error.message)?
        .into_parts()
        .1;
    let untouched_projections_shared = Arc::ptr_eq(&incremental.fixtures, &large.fixtures)
        && Arc::ptr_eq(&incremental.groups, &large.groups)
        && Arc::ptr_eq(&incremental.routes, &large.routes)
        && Arc::ptr_eq(&incremental.control_mappings, &large.control_mappings);
    let mut incremental_equivalent = incremental.clone();
    incremental_equivalent.fixtures = Default::default();
    incremental_equivalent.revision = 0;
    let mut full_equivalent = full;
    full_equivalent.fixtures = Default::default();
    full_equivalent.revision = 0;
    let full_compile_equivalent = serde_json::to_value(incremental_equivalent).unwrap()
        == serde_json::to_value(full_equivalent).unwrap();
    let size_independent = large_distribution.p95_microseconds
        <= (small_distribution.p95_microseconds * 2.0)
            .max(small_distribution.p95_microseconds + 2_000.0);
    let absolute_latency_met = large_distribution.p95_microseconds <= ABSOLUTE_P95_MICROSECONDS;
    let gate_met = untouched_projections_shared
        && full_compile_equivalent
        && size_independent
        && absolute_latency_met;
    Ok(ShowMutationReport {
        object_count: CUE_LISTS + GROUPS + PRESETS,
        small_fixture_count: SMALL_FIXTURES,
        large_fixture_count: LARGE_FIXTURES,
        small: small_distribution,
        large: large_distribution,
        untouched_projections_shared,
        full_compile_equivalent,
        size_independent,
        absolute_latency_met,
        gate_met,
    })
}

fn samples(
    document: &PortableShowDocument,
    previous: &light_engine::EngineSnapshot,
) -> Result<Vec<std::time::Duration>, String> {
    let engine = light_engine::Engine::new(light_programmer::ProgrammerRegistry::default());
    engine
        .replace_snapshot(previous.clone())
        .map_err(|error| error.to_string())?;
    for index in 0..WARMUPS {
        let snapshot = prepare_normalized_show_candidate_incremental(
            document,
            cue_transaction(document, &format!("warmup-{index}")),
            previous,
        )
        .map_err(|error| error.message)?
        .into_parts()
        .1;
        let prepared = engine
            .prepare_snapshot(snapshot)
            .map_err(|error| error.to_string())?;
        engine.install_prepared_snapshot(black_box(prepared));
    }
    (0..SAMPLES)
        .map(|index| {
            let transaction = cue_transaction(document, &format!("sample-{index}"));
            let started = Instant::now();
            let snapshot =
                prepare_normalized_show_candidate_incremental(document, transaction, previous)
                    .map_err(|error| error.message)?
                    .into_parts()
                    .1;
            let prepared = engine
                .prepare_snapshot(snapshot)
                .map_err(|error| error.to_string())?;
            engine.install_prepared_snapshot(black_box(prepared));
            Ok(started.elapsed())
        })
        .collect()
}

fn cue_transaction(
    document: &PortableShowDocument,
    name: &str,
) -> light_show::PortableShowTransaction {
    let mut body = document.object("cue_list", "0").unwrap().body().clone();
    body["name"] = json!(name);
    let mut transaction = document.transaction();
    transaction.put("cue_list", "0", body);
    transaction
}

fn generated_document() -> Result<(ShowStore, PortableShowDocument), String> {
    let (store, _) =
        ShowStore::create(":memory:", "Mutation benchmark").map_err(|error| error.to_string())?;
    for index in 0..CUE_LISTS {
        let cue = CueList {
            id: CueListId(Uuid::from_u128(10_000 + index as u128)),
            name: format!("Cue list {index}"),
            priority: 0,
            mode: CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: IntensityPriorityMode::Htp,
            wrap_mode: Some(WrapMode::Tracking),
            restart_mode: RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            auto_off_at_zero: false,
            auto_off_flash_release: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: None,
            speed_multiplier: 1.0,
            cues: (1_u8..=20).map(|number| Cue::new(number.into())).collect(),
        };
        store
            .put_object(
                "cue_list",
                &index.to_string(),
                &serde_json::to_value(cue).unwrap(),
                0,
            )
            .map_err(|error| error.to_string())?;
    }
    for index in 0..GROUPS {
        store
            .put_object(
                "group",
                &index.to_string(),
                &json!({"id": index.to_string(), "name": format!("Group {index}"), "fixtures": []}),
                0,
            )
            .map_err(|error| error.to_string())?;
    }
    for index in 0..PRESETS {
        store
            .put_object(
                "preset",
                &format!("1.{}", index + 1),
                &json!({
                    "number": index + 1,
                    "name": format!("Preset {index}"),
                    "family": "Intensity",
                    "values": {}
                }),
                0,
            )
            .map_err(|error| error.to_string())?;
    }
    let document = store
        .portable_document()
        .map_err(|error| error.to_string())?;
    let migration = prepare_show_candidate(&document, document.transaction())
        .map_err(|error| error.message)?
        .into_parts()
        .0;
    if !migration.is_empty() {
        store
            .apply_portable_transaction(migration)
            .map_err(|error| error.to_string())?;
    }
    let document = store
        .portable_document()
        .map_err(|error| error.to_string())?;
    Ok((store, document))
}

fn with_fixtures(
    previous: &light_engine::EngineSnapshot,
    count: usize,
) -> Result<light_engine::EngineSnapshot, String> {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "ToskLight Benchmark".into();
    profile.name = "Mutation projection".into();
    profile.short_name = "Mutation".into();
    let definition = profile
        .resolved_definition(profile.modes[0].id)
        .map_err(|error| error.to_string())?;
    let fixtures = (0..count)
        .map(|index| PatchedFixture {
            fixture_id: FixtureId(Uuid::from_u128(100_000 + index as u128)),
            fixture_number: Some(index as u32 + 1),
            virtual_fixture_number: None,
            name: format!("Fixture {}", index + 1),
            definition: definition.clone(),
            universe: None,
            address: None,
            split_patches: vec![],
            layer_id: "default".into(),
            note: None,
            position_master: None,
            direct_control: None,
            internal_bindings: Default::default(),
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![],
            group_masters_enabled: true,
            grand_master_enabled: true,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.0,
            shaper_angle: None,
            installed_appearance: Default::default(),
            move_in_black_enabled: false,
            move_in_black_delay_millis: 0,
            highlight_overrides: Default::default(),
            freeze: Default::default(),
        })
        .collect::<Vec<_>>();
    Ok(light_engine::EngineSnapshot {
        fixtures: fixtures.into(),
        ..previous.clone()
    })
}
