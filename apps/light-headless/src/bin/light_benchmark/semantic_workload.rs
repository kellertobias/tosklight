//! TL-596: the TL-564 semantic workload, loaded from its immutable input directory and rendered
//! through the production Live transaction.
//!
//! The directory holds what `tools/semantic-performance-workload.mjs` wrote (`manifest.json`,
//! `workload.json`, `tracking-<scenario>-<hz>hz.ndjson`) plus the `patch.json` the synthetic
//! rig was built from. Definitions, static bases and Point streams are used exactly as written.
//! The patch snapshot carries identities, profiles, modes and mounts but no DMX addresses or
//! locations, so this harness assigns them deterministically: Points stay unpatched at their
//! tracked centre, every other fixture is packed into consecutive universes and placed on a
//! grid at `--rig-height-mm` (default 9 m, above the Point volume). That placement is the harness's, not the workload's.
use crate::light_benchmark::{
    arguments::ProtocolSelection,
    scenario::{BenchmarkScenario, FixtureInventoryEntry, ScenarioFixtureInventory},
    semantic_programming::{DynamicStart, live_bench, start_dynamics},
    semantic_runner::{LiveScenario, LiveWorkloadDescription, TrackingFeed},
    sustained_show::{benchmark_start, fixed_uuid, routes},
};
use light_core::{AttributeKey, AttributeValue, FixtureId, ManualClock, SessionId};
use light_dynamics::DynamicDefinition;
use light_engine::{Engine, EngineSnapshot, TrackedPointInput, TrackedSampleIdentity};
use light_fixture::{
    FixtureDefinition, FixtureLocation, FixtureProfile, PatchedFixture, PatchedHead, SplitPatch,
    read_fixture_package,
};
use light_programmer::ProgrammerRegistry;
use serde_json::Value;
use std::{collections::HashMap, fs, net::SocketAddr, path::Path, sync::Arc};
use uuid::Uuid;

pub(super) struct WorkloadOptions<'a> {
    pub directory: &'a Path,
    pub package_dir: &'a Path,
    pub tracking_hz: Option<u16>,
    pub tracking_scenario: &'a str,
    pub rate_hz: u16,
    pub publish: bool,
    pub static_bases_only: bool,
    pub rig_height_mm: i32,
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("workload field {key} is missing"))
}

fn uuid(value: &Value, key: &str) -> Result<Uuid, String> {
    text(value, key)?
        .parse()
        .map_err(|error| format!("workload field {key}: {error}"))
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("workload field {key} is missing"))
}

pub(super) fn build(
    options: &WorkloadOptions<'_>,
    protocol: ProtocolSelection,
    loopback_destination: Option<SocketAddr>,
) -> Result<BenchmarkScenario, String> {
    let manifest = read_json(&options.directory.join("manifest.json"))?;
    let workload = read_json(&options.directory.join("workload.json"))?;
    let patch = read_json(&options.directory.join("patch.json"))?;
    if text(&workload, "workloadId")? != text(&manifest, "workloadId")? {
        return Err("workload.json and manifest.json name different workloads".into());
    }
    let centres = point_centres(&workload)?;
    let profiles = load_profiles(&patch, options.package_dir)?;
    let layout = patch_fixtures(&patch, &profiles, &centres, options.rig_height_mm)?;
    let definitions = array(&workload, "definitions")?
        .iter()
        .map(|definition| {
            serde_json::from_value::<DynamicDefinition>(definition.clone())
                .map_err(|error| format!("workload Dynamic definition: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let logical_start = benchmark_start();
    let clock = Arc::new(ManualClock::new(logical_start));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId(fixed_uuid(0xa0, 1));
    programmers.start(session);
    let output_routes = routes(layout.universes, protocol, loopback_destination);
    let packet_count = output_routes.len();
    let engine = Arc::new(Engine::new(programmers.clone()));
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: layout.fixtures.into(),
            dynamics: definitions.clone().into(),
            routes: output_routes.into(),
            revision: 1,
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
    let bases = apply_static_values(&programmers, session, &workload)?;
    let starts = if options.static_bases_only {
        Vec::new()
    } else {
        activations(&workload, &definitions)?
    };
    start_dynamics(&programmers, session, &starts)?;
    let scenario = dirty_scenario(&workload, options.tracking_scenario)?;
    let tracking = options
        .tracking_hz
        .filter(|rate| *rate > 0)
        .map(|rate| tracking_feed(options, &manifest, rate))
        .transpose()?;
    let bench = live_bench(
        Arc::clone(&engine),
        definitions,
        options.rate_hz,
        options.publish,
    )?;
    let animated_targets = starts.iter().map(|start| start.targets.len()).sum();
    let description = LiveWorkloadDescription {
        kind: if options.static_bases_only {
            "tl564_semantic_workload_static_bases_only"
        } else {
            "tl564_semantic_workload"
        },
        typed_lane_attributes: lane_coverage(&starts),
        semantic_base_targets: bases,
        started_dynamics: starts.len(),
        animated_targets,
        manifest_sha256: Some(text(&manifest, "manifestSha256")?.to_owned()),
        workload_id: Some(text(&manifest, "workloadId")?.to_owned()),
        expected_dirty_targets: tracking.as_ref().map(|_| scenario.0),
        expected_moving_points: tracking.as_ref().map(|_| scenario.1),
        harness_rig_height_mm: Some(options.rig_height_mm),
        omitted_from_live_transaction: OMITTED,
    };
    Ok(BenchmarkScenario {
        engine,
        clock,
        logical_start,
        universes: layout.universes,
        fixture_count: layout.patched_count,
        fixture_footprint: None,
        packet_count,
        fixture_inventory: layout.inventory,
        expected_patched_slots: layout.expected_patched_slots,
        workload_tier: "semantic_tl564_workload",
        physical_instance_count: layout.patched_count,
        dynamic_definition_count: starts.len(),
        animated_attribute_count: animated_targets,
        dynamic_lane_attributes: &[],
        dynamic_excluded_fixture_count: 0,
        active_ui_surfaces: &[],
        visualization_enabled: options.publish,
        release_blocking: false,
        programmers,
        dynamic_attribute: AttributeKey::intensity(),
        dynamic_overlaps_static_or_programmer: true,
        programmer_assignment_fraction: "TL-564 static bases",
        dynamic: None,
        live: Some(LiveScenario {
            bench,
            tracking,
            description,
            pending_start: None,
        }),
    })
}

pub(super) const OMITTED: &[&str] = &[
    "ordered Playback unit of work (captured automatic-cue events, persistence checkpoints)",
    "timecode and internal audio",
    "Hold and raw DMX overrides",
    "network and USB send (routes are encoded only)",
];

fn lane_coverage(starts: &[DynamicStart]) -> Vec<String> {
    let mut lanes = starts
        .iter()
        .flat_map(|start| start.definition.lanes.iter())
        .map(|lane| lane.output_owner().0.to_string())
        .collect::<Vec<_>>();
    lanes.sort();
    lanes.dedup();
    lanes
}

/// (expected dirty targets, expected moving Points) of the named dirty scenario.
fn dirty_scenario(workload: &Value, key: &str) -> Result<(usize, usize), String> {
    let scenarios = workload
        .get("dirty")
        .and_then(|dirty| dirty.get("scenarios"))
        .and_then(Value::as_array)
        .ok_or("workload has no dirty scenarios")?;
    let scenario = scenarios
        .iter()
        .find(|scenario| scenario.get("key").and_then(Value::as_str) == Some(key))
        .ok_or_else(|| format!("workload has no dirty scenario {key}"))?;
    Ok((
        scenario
            .get("dirtyTargetCount")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        array(scenario, "movingPointIds")?.len(),
    ))
}

fn point_centres(workload: &Value) -> Result<HashMap<Uuid, [f32; 3]>, String> {
    let motion = workload
        .get("tracking")
        .and_then(|tracking| tracking.get("motion"))
        .and_then(Value::as_array)
        .ok_or("workload has no tracking motion")?;
    motion
        .iter()
        .map(|point| {
            let centre = array(point, "center")?;
            let axis = |index: usize| {
                centre
                    .get(index)
                    .and_then(Value::as_f64)
                    .map(|value| value as f32)
                    .ok_or("tracking centre is not three numbers")
            };
            Ok((uuid(point, "pointId")?, [axis(0)?, axis(1)?, axis(2)?]))
        })
        .collect()
}

/// Shipped packages named by the patch's rig, with the runtime compatibility the real patch
/// compiler applies. The package's profile id and revision must match the patch snapshot.
fn load_profiles(
    patch: &Value,
    package_dir: &Path,
) -> Result<HashMap<Uuid, FixtureProfile>, String> {
    let packages = patch
        .get("rig")
        .and_then(|rig| rig.get("packages"))
        .and_then(Value::as_array)
        .ok_or("patch.json has no rig packages")?;
    let mut profiles = HashMap::new();
    for package in packages {
        let path = package_dir.join(format!("{}.toskfixture", text(package, "package")?));
        let bytes = fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
        let mut profile = read_fixture_package(&bytes)
            .map_err(|error| format!("load fixture package {}: {error}", path.display()))?;
        let revision = package.get("profile_revision").and_then(Value::as_u64);
        if profile.id.0 != uuid(package, "profile_id")?
            || Some(u64::from(profile.revision)) != revision
        {
            return Err(format!(
                "{} does not match the patch's profile identity",
                path.display()
            ));
        }
        profile.photograph_asset = None;
        profile.stage_icon_asset = None;
        profile.model_asset = None;
        light_fixture::apply_runtime_profile_compatibility(&mut profile);
        profiles.insert(profile.id.0, profile);
    }
    Ok(profiles)
}

struct Layout {
    fixtures: Vec<PatchedFixture>,
    patched_count: usize,
    universes: u16,
    expected_patched_slots: HashMap<u16, u16>,
    inventory: ScenarioFixtureInventory,
}

fn patch_fixtures(
    patch: &Value,
    profiles: &HashMap<Uuid, FixtureProfile>,
    centres: &HashMap<Uuid, [f32; 3]>,
    rig_height_mm: i32,
) -> Result<Layout, String> {
    let mut definitions = HashMap::<(Uuid, Uuid), Arc<FixtureDefinition>>::new();
    let mut fixtures = Vec::new();
    let mut slots = Vec::<u16>::new();
    let mut inventory = HashMap::<(String, String), (usize, u16)>::new();
    for (index, entry) in array(patch, "fixtures")?.iter().enumerate() {
        let fixture_id = uuid(entry, "fixture_id")?;
        let profile_id = uuid(entry, "profile_id")?;
        let mode_id = uuid(entry, "mode_id")?;
        let profile = profiles
            .get(&profile_id)
            .ok_or_else(|| format!("patch profile {profile_id} is not in the rig packages"))?;
        let definition = match definitions.get(&(profile_id, mode_id)) {
            Some(definition) => Arc::clone(definition),
            None => {
                let definition = Arc::new(
                    profile
                        .resolved_definition(mode_id)
                        .map_err(|error| format!("resolve {}: {error}", profile.name))?,
                );
                definitions.insert((profile_id, mode_id), Arc::clone(&definition));
                definition
            }
        };
        let centre = centres.get(&fixture_id);
        let mut fixture = base_fixture(FixtureId(fixture_id), entry, &definition)?;
        if let Some([x, y, z]) = centre {
            fixture.location = millimetres(*x, *y, *z);
        } else {
            let footprint = definition.footprint;
            let universe_index = slots
                .iter()
                .position(|used| 512 - *used >= footprint)
                .unwrap_or_else(|| {
                    slots.push(0);
                    slots.len() - 1
                });
            let address = slots[universe_index] + 1;
            slots[universe_index] += footprint;
            let universe = universe_index as u16 + 1;
            fixture.universe = Some(universe);
            fixture.address = Some(address);
            fixture.split_patches = vec![SplitPatch {
                split: 1,
                universe: Some(universe),
                address: Some(address),
            }];
            // A grid at truss height above the Point volume (Points sit between 3.5 and 8 m).
            fixture.location = FixtureLocation {
                x: (index % 12) as i32 * 1_000 - 5_500,
                y: (index / 12) as i32 * 1_400 - 7_000,
                z: rig_height_mm,
            };
            let key = (profile.manufacturer.clone(), profile.name.clone());
            let row = inventory.entry(key).or_insert((0, footprint));
            row.0 += 1;
        }
        fixtures.push(fixture);
    }
    let patched_count = fixtures
        .iter()
        .filter(|fixture| fixture.universe.is_some())
        .count();
    let mut entries = inventory
        .into_iter()
        .map(
            |((manufacturer, name), (quantity, footprint))| FixtureInventoryEntry {
                manufacturer,
                name,
                mode: "TL-564 synthetic rig".into(),
                quantity,
                footprint,
                dmx_slots: quantity * usize::from(footprint),
            },
        )
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        (&left.manufacturer, &left.name).cmp(&(&right.manufacturer, &right.name))
    });
    let total_slots = entries.iter().map(|entry| entry.dmx_slots).sum();
    Ok(Layout {
        fixtures,
        patched_count,
        universes: u16::try_from(slots.len()).map_err(|_| "too many universes")?,
        expected_patched_slots: slots
            .iter()
            .enumerate()
            .map(|(index, used)| (index as u16 + 1, *used))
            .collect(),
        inventory: ScenarioFixtureInventory {
            scenario: "tl564_synthetic_semantic_rig",
            entries,
            manufacturer_fixture_slots: total_slots,
            rgb_par_fill_slots: 0,
            total_slots,
        },
    })
}

fn millimetres(x: f32, y: f32, z: f32) -> FixtureLocation {
    let mm = |metres: f32| (metres * 1_000.0).round() as i32;
    FixtureLocation {
        x: mm(x),
        y: mm(y),
        z: mm(z),
    }
}

fn base_fixture(
    fixture_id: FixtureId,
    entry: &Value,
    definition: &Arc<FixtureDefinition>,
) -> Result<PatchedFixture, String> {
    let logical_heads = array(entry, "logical_heads")?
        .iter()
        .map(|head| {
            Ok(PatchedHead {
                profile_head_id: Some(uuid(head, "profile_head_id")?),
                head_index: head
                    .get("head_index")
                    .and_then(Value::as_u64)
                    .ok_or("logical head index is missing")? as u16,
                fixture_id: FixtureId(uuid(head, "fixture_id")?),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let position_master = match entry.get("position_master") {
        Some(Value::String(_)) => Some(uuid(entry, "position_master")?),
        _ => None,
    };
    let mut fixture = crate::light_benchmark::sustained_show::patched_definition(
        fixture_id,
        entry
            .get("fixture_number")
            .and_then(Value::as_u64)
            .map(|number| number as u32),
        text(entry, "name")?.to_owned(),
        definition.as_ref().clone(),
    );
    fixture.logical_heads = logical_heads;
    fixture.position_master = position_master;
    Ok(fixture)
}

fn apply_static_values(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    workload: &Value,
) -> Result<usize, String> {
    let values = array(workload, "staticValues")?
        .iter()
        .map(|mutation| {
            if text(mutation, "type")? != "set_fixture" {
                return Err("only set_fixture static values are supported".to_owned());
            }
            let value = serde_json::from_value::<AttributeValue>(
                mutation
                    .get("value")
                    .cloned()
                    .ok_or("static value is missing")?,
            )
            .map_err(|error| format!("static value: {error}"))?;
            Ok((
                FixtureId(uuid(mutation, "fixture_id")?),
                AttributeKey(text(mutation, "attribute")?.into()),
                value,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let count = values.len();
    programmers.set_many(session, values);
    Ok(count)
}

fn activations(
    workload: &Value,
    definitions: &[DynamicDefinition],
) -> Result<Vec<DynamicStart>, String> {
    array(workload, "activations")?
        .iter()
        .map(|activation| {
            let id = uuid(activation, "dynamicId")?;
            let definition = definitions
                .iter()
                .find(|definition| definition.id == id)
                .ok_or_else(|| format!("activation names unknown Dynamic {id}"))?
                .clone();
            let start = activation
                .get("start")
                .ok_or("activation has no start body")?;
            let overrides = start.get("overrides").cloned().unwrap_or_default();
            if overrides
                != serde_json::json!({
                    "size": 1, "speed_multiplier": { "numerator": 1, "denominator": 1 },
                    "phase_offset_degrees": 0,
                })
            {
                return Err("only neutral activation overrides are supported".into());
            }
            let targets = array(start, "targets")?
                .iter()
                .map(|target| {
                    target
                        .as_str()
                        .and_then(|target| target.parse().ok())
                        .map(FixtureId)
                        .ok_or("activation target is not a UUID".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(DynamicStart {
                link: Uuid::new_v5(&id, b"tosklight:tl596:activation"),
                definition,
                targets,
            })
        })
        .collect()
}

fn tracking_feed(
    options: &WorkloadOptions<'_>,
    manifest: &Value,
    rate: u16,
) -> Result<TrackingFeed, String> {
    let name = format!("tracking-{}-{rate}hz.ndjson", options.tracking_scenario);
    let path = options.directory.join(&name);
    let contents =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let stream_sha256 = manifest
        .get("tracking")
        .and_then(|tracking| tracking.get("streams"))
        .and_then(Value::as_array)
        .and_then(|streams| {
            streams.iter().find(|stream| {
                stream.get("scenario").and_then(Value::as_str) == Some(options.tracking_scenario)
                    && stream.get("rateHz").and_then(Value::as_u64) == Some(u64::from(rate))
            })
        })
        .and_then(|stream| stream.get("sha256"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut frames = Vec::new();
    for line in contents.lines().filter(|line| !line.trim().is_empty()) {
        let frame: Value =
            serde_json::from_str(line).map_err(|error| format!("{name}: {error}"))?;
        let t_micros = frame
            .get("t_micros")
            .and_then(Value::as_u64)
            .ok_or("frame has no t_micros")?;
        let sequence = frame
            .get("sequence")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let points = array(&frame, "points")?
            .iter()
            .map(|point| {
                let point_id = uuid(point, "point_id")?;
                let position = array(point, "position_metres")?;
                let axis = |index: usize| {
                    position
                        .get(index)
                        .and_then(Value::as_f64)
                        .map(|value| value as f32)
                        .ok_or("Point position is not three numbers".to_owned())
                };
                Ok(TrackedPointInput {
                    binding_id: Uuid::new_v5(&point_id, b"tosklight:tl596:binding"),
                    fixture_id: FixtureId(point_id),
                    position_metres: [axis(0)?, axis(1)?, axis(2)?],
                    sample: TrackedSampleIdentity {
                        source_id: "tl596-workload-stream".into(),
                        source_generation: 1,
                        source_epoch: 1,
                        sequence,
                        sender_timestamp_micros: t_micros,
                        accepted_at_millis: t_micros / 1_000,
                    },
                    position_received_at_millis: t_micros / 1_000,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        frames.push((t_micros, points.into()));
    }
    TrackingFeed::new(
        rate,
        options.tracking_scenario.to_owned(),
        stream_sha256,
        frames,
    )
}
