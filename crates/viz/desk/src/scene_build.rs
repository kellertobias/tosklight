//! Assemble one coherent renderer scene from the desk's read models.

use crate::transform::{self, PlacementSource};
use crate::wire::{ObjectRecord, PatchSnapshot, StageLayoutBody};
use glam::Vec3;
use light_fixture::{FixtureProfile, PatchPolicy, apply_runtime_profile_compatibility};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use viz_project::{PatchedFixture, PhysicalInstance, ScenePlan};
use viz_scene::{
    CrowdArea, CrowdDensity, CrowdPosture, MediaCrop, MediaProjector as SceneMediaProjector,
    MediaSection, MediaSectionKind, MediaSourceBinding, SceneryKind, SceneryObject,
};

/// Everything the scene builder reads, gathered by the connection before anything is displayed.
pub struct DeskReadModels {
    pub patch: PatchSnapshot,
    pub stage_layout: StageLayoutBody,
    pub venue_objects: Vec<ObjectRecord>,
    pub fixture_visibility: Vec<ObjectRecord>,
    pub patch_layers: Vec<ObjectRecord>,
    pub media_servers: Vec<ObjectRecord>,
    pub media_fallback_assets: Vec<ObjectRecord>,
    pub media_sources: Vec<ObjectRecord>,
    pub led_module_types: Vec<ObjectRecord>,
    pub media_surfaces: Vec<ObjectRecord>,
    pub media_projectors: Vec<ObjectRecord>,
    /// The CAD's Venue groups (`cad_venue_groups`), so a whole selected group is drawn apart.
    pub venue_groups: Vec<ObjectRecord>,
    pub show_name: String,
    pub server_identity: String,
}

/// Each Venue group's member fixture ids, read from the CAD's one groups object. An unreadable or
/// absent object means no groups: selection is then drawn exactly as before groups existed.
fn venue_group_members(objects: &[ObjectRecord]) -> Vec<Vec<uuid::Uuid>> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Group {
        #[serde(default)]
        member_ids: Vec<uuid::Uuid>,
    }
    #[derive(serde::Deserialize)]
    struct Groups {
        #[serde(default)]
        groups: Vec<Group>,
    }
    objects
        .iter()
        .filter_map(|object| serde_json::from_value::<Groups>(object.body.clone()).ok())
        .flat_map(|groups| groups.groups)
        .map(|group| group.member_ids)
        .filter(|members| !members.is_empty())
        .collect()
}

/// Build the scene and its bindings, plus any warnings the operator must see.
pub fn build(models: &DeskReadModels) -> ScenePlan {
    let profiles = decode_profiles(&models.patch);
    let mut fixtures = Vec::with_capacity(models.patch.fixtures.len());
    let mut warnings = Vec::new();
    // Where every instance's transform actually came from. A rig standing in the wrong place is
    // first of all a question of which of the four sources placed it, so the diagnostics say.
    let mut placements = Vec::with_capacity(models.patch.fixtures.len());
    let mut grid_index = 0_usize;
    let hidden_fixtures = hidden_fixture_ids(&models.fixture_visibility, "visible3d");
    let hidden_layers = hidden_object_ids(&models.patch_layers, "visible3d");

    for fixture in &models.patch.fixtures {
        if hidden_fixtures.contains(&fixture.fixture_id)
            || hidden_layers.contains(&fixture.layer_id)
        {
            continue;
        }
        let Some(profile) = profiles.get(&(fixture.profile_id, fixture.profile_revision)) else {
            warnings.push(format!(
                "{}: profile revision {} has no snapshot; the fixture is shown as a generic body",
                fixture.name, fixture.profile_revision
            ));
            continue;
        };
        let mut instances = Vec::with_capacity(1 + fixture.multipatch.len());
        let root_placement = transform::resolve(
            models
                .stage_layout
                .positions3d
                .get(&fixture.fixture_id.to_string()),
            fixture.location,
            fixture.rotation,
            models
                .stage_layout
                .positions
                .get(&fixture.fixture_id.to_string()),
            grid_index,
        );
        grid_index += 1;
        placements.push(root_placement.source);
        instances.push(PhysicalInstance {
            scenery_size_metres: placed_scenery_size(fixture.scenery_size_metres),
            scenery_options: fixture.scenery_options.clone(),
            model_scale: light_fixture::resolved_model_scale(fixture.model_scale),
            instance_id: fixture.fixture_id,
            name: fixture.name.clone(),
            split_patches: split_patches(&fixture.split_patches),
            position: root_placement.position,
            rotation_degrees: root_placement.rotation_degrees,
            invert_pan: fixture.invert_pan,
            invert_tilt: fixture.invert_tilt,
            bracket_angle: fixture.bracket_angle,
            shaper_angle: fixture.shaper_angle,
            installed_appearance: fixture.installed_appearance.clone(),
        });
        for multipatch in &fixture.multipatch {
            // A multi-patch instance shares the logical fixture's values but keeps its own
            // transform, so it never inherits the root's placement.
            let placement = transform::resolve(
                models
                    .stage_layout
                    .positions3d
                    .get(&multipatch.id.to_string()),
                multipatch.location,
                multipatch.rotation,
                None,
                grid_index,
            );
            grid_index += 1;
            placements.push(placement.source);
            instances.push(PhysicalInstance {
                scenery_size_metres: placed_scenery_size(multipatch.scenery_size_metres),
                // Venue objects cannot be multi-patched, so an instance never has options.
                scenery_options: Default::default(),
                model_scale: 1.0,
                instance_id: multipatch.id,
                name: if multipatch.name.is_empty() {
                    format!("{} \u{2022} multi-patch", fixture.name)
                } else {
                    multipatch.name.clone()
                },
                split_patches: split_patches(&multipatch.split_patches),
                position: placement.position,
                rotation_degrees: placement.rotation_degrees,
                invert_pan: multipatch.invert_pan,
                invert_tilt: multipatch.invert_tilt,
                bracket_angle: multipatch.bracket_angle,
                shaper_angle: multipatch.shaper_angle,
                installed_appearance: multipatch.installed_appearance.clone(),
            });
        }
        fixtures.push(PatchedFixture {
            fixture_id: fixture.fixture_id,
            name: fixture.name.clone(),
            number: fixture.fixture_number,
            profile: profile.clone(),
            mode_id: fixture.mode_id,
            instances,
        });
    }

    let mut plan = viz_project::compile(&fixtures);
    // Desk output already contains each physical instance's Pan/Tilt inversion. Decode those
    // wire positions directly; applying the patch flags again would undo the desk's output.
    // Keep the instance metadata for semantic preview positioning.
    for binding in &mut plan.bindings {
        binding.invert_pan = false;
        binding.invert_tilt = false;
    }
    let masters: std::collections::HashMap<_, _> = models
        .patch
        .fixtures
        .iter()
        .filter_map(|fixture| Some((fixture.fixture_id, fixture.position_master?)))
        .collect();
    for instance in &mut plan.scene.fixtures {
        instance.position_master = masters.get(&instance.fixture_id).copied();
    }
    // A generated Venue object is a fixture the scenery pass builds instead of a body, so it
    // follows the same point its fixture does. The object is keyed by the instance it was built
    // for, and the instance knows its fixture.
    let instance_fixtures: std::collections::HashMap<_, _> = plan
        .scene
        .fixtures
        .iter()
        .map(|instance| (instance.instance_id, instance.fixture_id))
        .collect();
    for object in &mut plan.scene.scenery {
        object.position_master = instance_fixtures
            .get(&object.id)
            .and_then(|fixture_id| masters.get(fixture_id))
            .copied();
    }
    plan.warnings.extend(warnings);
    if !placements.is_empty() {
        plan.warnings.push(placement_summary(&placements));
    }
    plan.scene.show_id = Some(models.patch.show_id);
    plan.scene.show_name = models.show_name.clone();
    plan.scene.source_identity = models.server_identity.clone();
    plan.scene.revision = models.patch.patch_revision;
    plan.scene.venue_groups = venue_group_members(&models.venue_groups);
    // Standalone `venue` records predate Venue fixtures. Once the patch carries its own scenery
    // they only duplicate it, so they are drawn solely for a show that has none.
    let legacy_venue: &[ObjectRecord] = if patch_carries_scenery(models, &profiles) {
        &[]
    } else {
        &models.venue_objects
    };
    plan.scene.scenery = build_scenery(&plan.scene, legacy_venue);
    plan.scene.crowds = build_crowds(models, &profiles);
    build_media(&mut plan.scene, models, &mut plan.warnings);
    plan.scene.recompute_bounds();
    plan
}

fn hidden_object_ids(
    objects: &[ObjectRecord],
    property: &str,
) -> std::collections::HashSet<String> {
    objects
        .iter()
        .filter(|object| {
            !object
                .body
                .get(property)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true)
        })
        .map(|object| object.id.clone())
        .collect()
}

fn hidden_fixture_ids(objects: &[ObjectRecord], property: &str) -> std::collections::HashSet<Uuid> {
    objects
        .iter()
        .filter(|object| {
            !object
                .body
                .get(property)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true)
        })
        .filter_map(|object| {
            object
                .body
                .get("fixtureId")
                .and_then(serde_json::Value::as_str)
        })
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect()
}

/// Whether any patched fixture is visual-only scenery other than a crowd area.
fn patch_carries_scenery(
    models: &DeskReadModels,
    profiles: &HashMap<(Uuid, u64), Arc<FixtureProfile>>,
) -> bool {
    models.patch.fixtures.iter().any(|fixture| {
        profiles
            .get(&(fixture.profile_id, fixture.profile_revision))
            .is_some_and(|profile| {
                profile.patch_policy == PatchPolicy::VisualOnly && profile.crowd.is_none()
            })
    })
}

fn build_crowds(
    models: &DeskReadModels,
    profiles: &HashMap<(Uuid, u64), Arc<FixtureProfile>>,
) -> Vec<CrowdArea> {
    let hidden_fixtures = hidden_fixture_ids(&models.fixture_visibility, "visible3d");
    let hidden_layers = hidden_object_ids(&models.patch_layers, "visible3d");
    let mut result = Vec::new();
    for (index, fixture) in models.patch.fixtures.iter().enumerate() {
        // A crowd hides with its layer and on its own, exactly like a fixture.
        if hidden_fixtures.contains(&fixture.fixture_id)
            || hidden_layers.contains(&fixture.layer_id)
        {
            continue;
        }
        let Some(profile) = profiles.get(&(fixture.profile_id, fixture.profile_revision)) else {
            continue;
        };
        let Some(crowd) = &profile.crowd else {
            continue;
        };
        let Some(mode) = crowd
            .modes
            .iter()
            .find(|binding| binding.mode_id == fixture.mode_id)
        else {
            continue;
        };
        let placement = transform::resolve(
            models
                .stage_layout
                .positions3d
                .get(&fixture.fixture_id.to_string()),
            fixture.location,
            fixture.rotation,
            models
                .stage_layout
                .positions
                .get(&fixture.fixture_id.to_string()),
            index,
        );
        let authored_footprint = models
            .stage_layout
            .positions3d
            .get(&fixture.fixture_id.to_string());
        // The size the PreViz plan gave the area, in the patch's millimetres (x across, z deep).
        let placed = fixture.scenery_size_metres.map(|size| {
            Vec3::new(
                size.x as f32 / 1000.0,
                size.y as f32 / 1000.0,
                size.z as f32 / 1000.0,
            )
        });
        result.push(CrowdArea {
            id: fixture.fixture_id,
            name: fixture.name.clone(),
            position: Vec3::new(
                placement.position.x,
                placement.position.y,
                placement.position.z,
            ),
            rotation_degrees: placement.rotation_degrees,
            width_metres: valid_crowd_dimension(
                authored_footprint.and_then(|value| value.crowd_width_metres),
                placed.map(|size| size.x),
                crowd.default_width_metres,
            ),
            depth_metres: valid_crowd_dimension(
                authored_footprint.and_then(|value| value.crowd_depth_metres),
                placed.map(|size| size.z),
                crowd.default_depth_metres,
            ),
            posture: match mode.posture {
                light_fixture::CrowdPosture::Sitting => CrowdPosture::Sitting,
                light_fixture::CrowdPosture::StandingStill => CrowdPosture::StandingStill,
                light_fixture::CrowdPosture::Dancing => CrowdPosture::Dancing,
            },
            density: match mode.density {
                light_fixture::CrowdDensity::Sparse => CrowdDensity::Sparse,
                light_fixture::CrowdDensity::Medium => CrowdDensity::Medium,
                light_fixture::CrowdDensity::Dense => CrowdDensity::Dense,
            },
            seed: u64::from_le_bytes(fixture.fixture_id.as_bytes()[..8].try_into().unwrap()),
        });
    }
    result
}

/// A crowd area's side: the desk's authored stage layout first, then the size the plan placed it
/// at, then its profile's default. A value outside 1-250 m is not a crowd and is passed over.
fn valid_crowd_dimension(authored: Option<f32>, placed: Option<f32>, fallback: f32) -> f32 {
    let valid = |value: &f32| value.is_finite() && (1.0..=250.0).contains(value);
    authored
        .filter(valid)
        .or(placed.filter(valid))
        .unwrap_or(fallback)
}

fn build_media(scene: &mut viz_scene::Scene, models: &DeskReadModels, warnings: &mut Vec<String>) {
    use viz_document::{
        LedModuleType, MediaFallbackAsset, MediaProjector, MediaServer, MediaSource, MediaSurface,
        MediaSurfaceSectionKind, ProjectionScreenMaterial,
    };
    let servers: HashMap<Uuid, MediaServer> = models
        .media_servers
        .iter()
        .filter_map(|record| serde_json::from_value::<MediaServer>(record.body.clone()).ok())
        .map(|server| (server.id, server))
        .collect();
    let fallback_assets: HashMap<Uuid, MediaFallbackAsset> = models
        .media_fallback_assets
        .iter()
        .filter_map(|record| serde_json::from_value::<MediaFallbackAsset>(record.body.clone()).ok())
        .map(|asset| (asset.id, asset))
        .collect();
    let sources: Vec<MediaSource> = models
        .media_sources
        .iter()
        .filter_map(|record| serde_json::from_value(record.body.clone()).ok())
        .collect();
    let modules: HashMap<Uuid, LedModuleType> = models
        .led_module_types
        .iter()
        .filter_map(|record| serde_json::from_value::<LedModuleType>(record.body.clone()).ok())
        .map(|module| (module.id, module))
        .collect();
    let surfaces: Vec<MediaSurface> = models
        .media_surfaces
        .iter()
        .filter_map(|record| serde_json::from_value(record.body.clone()).ok())
        .collect();
    let projectors: Vec<MediaProjector> = models
        .media_projectors
        .iter()
        .filter_map(|record| serde_json::from_value(record.body.clone()).ok())
        .collect();

    scene.media_sources = sources
        .iter()
        .filter_map(|source| {
            let server = servers.get(&source.server_id)?;
            Some(MediaSourceBinding {
                id: source.id,
                server_id: source.server_id,
                host: server.citp.host.clone(),
                port: server.citp.port,
                advertised_source_id: source.advertised_source_id,
                name: source.name.clone(),
                aspect_ratio: source.aspect_ratio,
                fallback_rgba: None,
            })
        })
        .collect();
    for asset in fallback_assets.values() {
        let Ok(bytes) = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &asset.bytes_base64,
        ) else {
            continue;
        };
        let Ok(decoded) = image::load_from_memory(&bytes) else {
            continue;
        };
        let rgba = decoded
            .resize_exact(
                viz_render_edge(),
                viz_render_edge(),
                image::imageops::FilterType::Triangle,
            )
            .to_rgba8()
            .into_raw();
        scene.media_sources.push(MediaSourceBinding {
            id: asset.id,
            server_id: Uuid::nil(),
            host: String::new(),
            port: 0,
            advertised_source_id: 0,
            name: asset.name.clone(),
            aspect_ratio: Some(asset.width as f32 / asset.height.max(1) as f32),
            fallback_rgba: Some(rgba),
        });
    }
    for surface in &surfaces {
        for section in &surface.sections {
            let kind = match &section.kind {
                MediaSurfaceSectionKind::ProjectionScreen {
                    material,
                    edge_feather,
                } => {
                    let (colour, gain, roughness) = match material {
                        ProjectionScreenMaterial::White => ([0.92, 0.92, 0.9], 1.0, 0.72),
                        ProjectionScreenMaterial::GreyHomeCinema => ([0.28, 0.29, 0.3], 0.82, 0.78),
                        ProjectionScreenMaterial::Custom {
                            gain,
                            tint_srgb,
                            roughness,
                        } => (srgb(tint_srgb).unwrap_or([0.8; 3]), *gain, *roughness),
                    };
                    MediaSectionKind::ProjectionScreen {
                        colour,
                        gain,
                        roughness,
                        edge_feather: *edge_feather,
                    }
                }
                MediaSurfaceSectionKind::Tv {
                    bezel_metres,
                    spill,
                } => MediaSectionKind::Tv {
                    bezel_metres: *bezel_metres,
                    spill: *spill,
                },
                MediaSurfaceSectionKind::Led {
                    module_type_id,
                    rows,
                    columns,
                    occupied_cells,
                } => {
                    let Some(module) = modules.get(module_type_id) else {
                        warnings.push(format!(
                            "{}: LED module type {} is unavailable",
                            section.name, module_type_id
                        ));
                        continue;
                    };
                    MediaSectionKind::Led {
                        rows: *rows,
                        columns: *columns,
                        occupied_cells: occupied_cells.clone(),
                        module_size: [module.width_metres, module.height_metres],
                        module_gap: [module.horizontal_gap_metres, module.vertical_gap_metres],
                        pixel_pitch_millimetres: module.pixel_pitch_millimetres,
                    }
                }
            };
            scene.media_sections.push(MediaSection {
                id: section.id,
                surface_id: surface.id,
                name: section.name.clone(),
                source_id: surface.source_id,
                fallback_source_id: surface.fallback.as_ref().map(|fallback| fallback.asset_id),
                position: Vec3::from_array(section.transform.position_metres),
                rotation_degrees: Vec3::from_array(section.transform.rotation_degrees),
                size: Vec3::new(section.width_metres, section.height_metres, 0.04),
                crop: MediaCrop {
                    left: section.crop.left,
                    top: section.crop.top,
                    width: section.crop.width,
                    height: section.crop.height,
                },
                kind,
            });
        }
    }
    scene.media_projectors = projectors
        .into_iter()
        .map(|projector| SceneMediaProjector {
            id: projector.id,
            surface_id: projector.surface_id,
            name: projector.name,
            position: Vec3::from_array(projector.transform.position_metres),
            rotation_degrees: Vec3::from_array(projector.transform.rotation_degrees),
            cone_length_metres: projector.cone_length_metres,
            spill: projector.spill,
        })
        .collect();
}

const fn viz_render_edge() -> u32 {
    512
}

fn srgb(value: &str) -> Option<[f32; 3]> {
    let value = value.strip_prefix('#')?;
    if value.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()? as f32 / 255.0,
        u8::from_str_radix(&value[2..4], 16).ok()? as f32 / 255.0,
        u8::from_str_radix(&value[4..6], 16).ok()? as f32 / 255.0,
    ])
}

fn split_patches(splits: &[crate::wire::SplitAssignment]) -> Vec<(u16, Option<(u16, u16)>)> {
    splits
        .iter()
        .map(|split| {
            (
                split.split,
                match (split.universe, split.address) {
                    (Some(universe), Some(address)) => Some((universe, address)),
                    _ => None,
                },
            )
        })
        .collect()
}

/// Decode the immutable profile snapshots the patch carries.
fn decode_profiles(patch: &PatchSnapshot) -> HashMap<(Uuid, u64), Arc<FixtureProfile>> {
    patch
        .profile_revisions
        .iter()
        .filter_map(|revision| {
            if revision.profile_snapshot.is_null() {
                return None;
            }
            let mut profile: FixtureProfile =
                serde_json::from_value(revision.profile_snapshot.clone()).ok()?;
            apply_runtime_profile_compatibility(&mut profile);
            Some((
                (revision.profile_id, revision.profile_revision),
                Arc::new(profile),
            ))
        })
        .collect()
}

/// Visual-only scenery: a stage floor sized to the rig, plus any `Venue` objects the show stores.
///
/// The floor exists because a beam has to land on something for its footprint to be visible.
fn build_scenery(scene: &viz_scene::Scene, venue: &[ObjectRecord]) -> Vec<SceneryObject> {
    // Scenery the patch generated from its Venue fixtures is already here; the floor and any
    // legacy records go beside it.
    let mut scenery = scene.scenery.clone();
    scenery.reserve(venue.len() + 2);
    let bounds = scene.bounds;
    // The deck extends well past the rig footprint so a wide wash lands on it instead of
    // spilling off the edge into empty space, where its beam would keep going with nothing to
    // stop it.
    const APRON_METRES: f32 = 16.0;
    let width = if bounds.is_empty() {
        24.0
    } else {
        (bounds.extent().x + APRON_METRES).clamp(12.0, 120.0)
    };
    let depth = if bounds.is_empty() {
        20.0
    } else {
        (bounds.extent().z + APRON_METRES).clamp(12.0, 120.0)
    };
    let centre = if bounds.is_empty() {
        Vec3::ZERO
    } else {
        bounds.centre()
    };
    scenery.push(SceneryObject {
        id: Uuid::nil(),
        name: "Stage floor".into(),
        position: Vec3::new(centre.x, -0.05, centre.z),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(width, 0.1, depth),
        // Nearly the colour of the room behind it. This surface exists so a beam has somewhere to
        // land — take it away and the pools go with it — not so an operator looks at it, and a
        // grey slab across the bottom of the picture is a large bright object competing with the
        // rig. Where the ground is is said by the renderer's grid, which is lines rather than a
        // plane and takes no light at all.
        colour: [0.012, 0.013, 0.018],
        roughness: 0.85,
        kind: SceneryKind::Floor,
        chords: 0,
        detail: Default::default(),
        position_master: None,
    });
    // No backdrop is invented. A show that wants one places a `Venue` object; anything else
    // would put a surface in the picture that the operator never rigged.
    for object in venue {
        let Some(position) = venue_vector(&object.body, "position") else {
            continue;
        };
        let size = venue_size(&object.body).unwrap_or(Vec3::splat(1.0));
        let rotation_degrees = venue_rotation(&object.body).unwrap_or(Vec3::ZERO);
        let kind = venue_kind(&object.body);
        let chords = venue_chords(&object.body, kind);
        scenery.push(SceneryObject {
            id: Uuid::nil(),
            name: object
                .body
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&object.id)
                .to_owned(),
            position,
            rotation_degrees,
            size,
            colour: match kind {
                // Mill-finish aluminium: a light, slightly cool grey.
                SceneryKind::Truss => [0.33, 0.335, 0.345],
                SceneryKind::Wall => [0.1, 0.1, 0.11],
                // Stage drape is black wool serge. The generic prop grey made the canonical
                // black legs and backcloth look like painted scenery in PreViz.
                SceneryKind::Curtain => [0.008, 0.008, 0.01],
                // Staging is film-faced multiplex: the dark brown phenolic face of a deck.
                SceneryKind::Riser | SceneryKind::Stairs => MULTIPLEX,
                _ => [0.14, 0.14, 0.15],
            },
            // Wool serge is the matt-est thing in a venue and aluminium the least: a drape
            // that catches the same highlight as the truss over it reads as painted board.
            // Truss is mill-finish aluminium, not polished: a soft, broad highlight.
            roughness: match kind {
                SceneryKind::Curtain => 0.96,
                SceneryKind::Truss => 0.6,
                SceneryKind::Railing => 0.45,
                _ => 0.8,
            },
            kind,
            chords,
            detail: Default::default(),
            position_master: None,
        });
    }
    scenery
}

/// The dark brown phenolic film on a staging deck, in linear light.
const MULTIPLEX: [f32; 3] = [0.036, 0.019, 0.009];

fn venue_vector(body: &serde_json::Value, key: &str) -> Option<Vec3> {
    let value = body.get(key)?;
    let read = |name: &str| value.get(name).and_then(serde_json::Value::as_f64);
    Some(transform::to_world(
        read("x")? as f32,
        read("y")? as f32,
        read("z")? as f32,
    ))
}

fn venue_rotation(body: &serde_json::Value) -> Option<Vec3> {
    let value = body.get("rotation_degrees")?;
    let read = |name: &str| value.get(name).and_then(serde_json::Value::as_f64);
    Some(transform::rotation_to_world(
        read("x")? as f32,
        read("y")? as f32,
        read("z")? as f32,
    ))
}

/// A venue object's extent. Sizes take the same axis order as positions but never a sign: an
/// extent is a length, and a negative one would turn the object inside out.
fn venue_size(body: &serde_json::Value) -> Option<Vec3> {
    let value = body.get("size")?;
    let read = |name: &str| value.get(name).and_then(serde_json::Value::as_f64);
    Some(
        Vec3::new(read("x")? as f32, read("z")? as f32, read("y")? as f32)
            .abs()
            .max(Vec3::splat(0.01)),
    )
}

/// What a venue object is, so a truss is drawn as structure rather than as an anonymous prop.
fn venue_kind(body: &serde_json::Value) -> SceneryKind {
    let named = ["kind", "venue_kind", "category", "shape", "type"]
        .into_iter()
        .find_map(|key| body.get(key).and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = body
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let says = |needle: &str| named.contains(needle) || name.contains(needle);
    if says("truss") || says("bar") || says("boom") {
        SceneryKind::Truss
    } else if says("mirror_ball") || says("mirror ball") || says("disco ball") {
        SceneryKind::MirrorBall
    } else if says("curtain") || says("drape") || says("serge") {
        SceneryKind::Curtain
    } else if says("railing") || says("handrail") {
        SceneryKind::Railing
    } else if says("stair") || says("steps") {
        SceneryKind::Stairs
    } else if says("wall") || says("cyc") || says("backdrop") || says("screen") {
        SceneryKind::Wall
    } else if says("riser") || says("deck") || says("rostrum") || says("stage") {
        SceneryKind::Riser
    } else {
        SceneryKind::Prop
    }
}

/// How many chords a truss has. Rigs say this in the name — "3 point", "four-point", "pipe" —
/// and a truss drawn with the wrong number of chords is the wrong truss.
fn venue_chords(body: &serde_json::Value, kind: SceneryKind) -> u8 {
    if kind != SceneryKind::Truss {
        return 0;
    }
    let text = ["kind", "venue_kind", "type", "name"]
        .into_iter()
        .filter_map(|key| body.get(key).and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let explicit = body
        .get("chords")
        .and_then(serde_json::Value::as_u64)
        .map(|chords| chords as u8);
    if let Some(chords) = explicit.filter(|chords| (1..=4).contains(chords)) {
        return chords;
    }
    let says = |needles: [&str; 3]| needles.iter().any(|needle| text.contains(needle));
    if says(["pipe", "1 point", "one point"]) || text.contains("bar") {
        1
    } else if says(["2 point", "two point", "ladder"]) || text.contains("2-point") {
        2
    } else if says(["3 point", "three point", "tri"]) || text.contains("3-point") {
        3
    } else {
        // A truss with nothing to say for itself is the one every rig has most of.
        4
    }
}

/// Placement provenance for the diagnostics surface.
pub fn placement_summary(sources: &[PlacementSource]) -> String {
    let count =
        |wanted: PlacementSource| sources.iter().filter(|source| **source == wanted).count();
    format!(
        "placement: {} from stage layout, {} from patch, {} migrated, {} default grid",
        count(PlacementSource::StageLayout3d),
        count(PlacementSource::PatchLocation),
        count(PlacementSource::MigratedLayout2d),
        count(PlacementSource::DefaultGrid),
    )
}

/// The size a generated Venue object was placed at, when the operator set one.
fn placed_scenery_size(stored: Option<crate::wire::Location>) -> Option<Vec3> {
    let stored = stored?;
    // Stored in millimetres, like every other measurement the patch carries.
    [stored.x, stored.y, stored.z]
        .iter()
        .all(|value| *value > 0)
        .then(|| {
            Vec3::new(
                stored.x as f32 / 1_000.0,
                stored.y as f32 / 1_000.0,
                stored.z as f32 / 1_000.0,
            )
        })
}
