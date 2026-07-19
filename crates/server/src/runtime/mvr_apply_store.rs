use super::*;

pub(super) fn mvr_transform(
    matrix: [f64; 12],
) -> (light_fixture::FixtureLocation, light_fixture::FixtureVector) {
    let location = light_fixture::FixtureLocation {
        x: matrix[9]
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
        y: matrix[10]
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
        z: matrix[11]
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
    };
    let rotation = light_fixture::FixtureVector {
        x: (matrix[9].atan2(matrix[10]).to_degrees()) as f32,
        y: (-matrix[8].asin().to_degrees()) as f32,
        z: (matrix[4].atan2(matrix[0]).to_degrees()) as f32,
    };
    (location, rotation)
}

type OccupiedPatch = (u16, u16, u16, String);

fn occupied_patches(objects: &[light_show::VersionedObject]) -> Vec<OccupiedPatch> {
    objects
        .iter()
        .filter_map(|object| {
            serde_json::from_value::<light_fixture::PatchedFixture>(object.body.clone())
                .ok()
                .and_then(|fixture| {
                    Some((
                        fixture.universe?,
                        fixture.address?,
                        fixture.definition.footprint,
                        object.id.clone(),
                    ))
                })
        })
        .collect()
}

fn mvr_fixture_ids(objects: &[light_show::VersionedObject]) -> HashMap<Uuid, String> {
    objects
        .iter()
        .filter_map(|object| {
            Uuid::parse_str(&object.id).ok().and_then(|uuid| {
                object
                    .body
                    .get("fixture_id")?
                    .as_str()
                    .map(|id| (uuid, id.to_owned()))
            })
        })
        .collect()
}

fn store_unresolved_mvr_fixture(
    store: &ShowStore,
    source: &light_mvr::MvrFixture,
) -> Result<(), ApiError> {
    let id = source.uuid.to_string();
    let current = store
        .objects("unresolved_mvr_fixture")
        .map_err(ApiError::store)?
        .into_iter()
        .find(|object| object.id == id)
        .map(|object| object.revision)
        .unwrap_or(0);
    store
        .put_object(
            "unresolved_mvr_fixture",
            &id,
            &serde_json::to_value(source)
                .map_err(|error| ApiError::bad_request(error.to_string()))?,
            current,
        )
        .map_err(ApiError::store)?;
    Ok(())
}

fn resolved_mvr_address(
    store: &ShowStore,
    source: &light_mvr::MvrFixture,
    fixture_id: light_core::FixtureId,
    definition: &light_fixture::FixtureDefinition,
    resolution: Option<&MvrResolution>,
    occupied: &mut Vec<OccupiedPatch>,
    warnings: &mut Vec<String>,
) -> Result<(Option<u16>, Option<u16>), ApiError> {
    let (mut universe, mut address) = match resolution {
        Some(MvrResolution::Address { universe, address }) => (Some(*universe), Some(*address)),
        Some(MvrResolution::ImportUnpatched) => (None, None),
        _ => (source.universe, source.address),
    };
    let Some((requested_universe, requested_address)) = universe.zip(address) else {
        return Ok((universe, address));
    };
    let end = requested_address.saturating_add(definition.footprint.saturating_sub(1));
    let conflict = occupied
        .iter()
        .find(|(other_universe, other_address, footprint, id)| {
            *other_universe == requested_universe
                && *id != fixture_id.0.to_string()
                && *other_address <= end
                && other_address.saturating_add(footprint.saturating_sub(1)) >= requested_address
        })
        .cloned();
    if let Some((_, _, _, id)) = conflict {
        if matches!(resolution, Some(MvrResolution::Replace)) {
            store
                .delete_object("patched_fixture", &id)
                .map_err(ApiError::store)?;
            occupied.retain(|item| item.3 != id);
        } else {
            universe = None;
            address = None;
            warnings.push(format!(
                "{} imported unpatched because its requested address conflicts",
                source.name
            ));
        }
    }
    Ok((universe, address))
}

fn patched_mvr_fixture(
    source: &light_mvr::MvrFixture,
    definition: &light_fixture::FixtureDefinition,
    fixture_id: light_core::FixtureId,
    address: (Option<u16>, Option<u16>),
    existing: &[light_show::VersionedObject],
) -> light_fixture::PatchedFixture {
    let (location, rotation) = mvr_transform(source.matrix);
    let existing_mib = existing
        .iter()
        .find(|object| object.id == fixture_id.0.to_string())
        .and_then(|object| {
            serde_json::from_value::<light_fixture::PatchedFixture>(object.body.clone()).ok()
        })
        .map(|fixture| {
            (
                fixture.move_in_black_enabled,
                fixture.move_in_black_delay_millis,
            )
        });
    light_fixture::PatchedFixture {
        fixture_id,
        fixture_number: source
            .fixture_id
            .as_deref()
            .and_then(|value| value.parse().ok()),
        virtual_fixture_number: None,
        name: source.name.clone(),
        definition: definition.clone(),
        universe: address.0,
        address: address.1,
        split_patches: Vec::new(),
        layer_id: source.layer.clone().unwrap_or_else(|| "default".into()),
        direct_control: None,
        location,
        rotation,
        logical_heads: definition
            .heads
            .iter()
            .filter(|head| !head.shared)
            .map(|head| light_fixture::PatchedHead {
                profile_head_id: None,
                head_index: head.index,
                fixture_id: light_core::FixtureId::new(),
            })
            .collect(),
        move_in_black_enabled: existing_mib.is_none_or(|settings| settings.0),
        move_in_black_delay_millis: existing_mib.map_or(0, |settings| settings.1),
        highlight_overrides: Default::default(),
        multipatch: Vec::new(),
    }
}

fn store_resolved_mvr_fixture(
    store: &ShowStore,
    source: &light_mvr::MvrFixture,
    patched: &light_fixture::PatchedFixture,
    existing: &[light_show::VersionedObject],
    metadata: &[light_show::VersionedObject],
) -> Result<String, ApiError> {
    let id = patched.fixture_id.0.to_string();
    let current = existing
        .iter()
        .find(|object| object.id == id)
        .map(|object| object.revision)
        .unwrap_or(0);
    store
        .put_object(
            "patched_fixture",
            &id,
            &serde_json::to_value(patched)
                .map_err(|error| ApiError::bad_request(error.to_string()))?,
            current,
        )
        .map_err(ApiError::store)?;
    let meta_current = metadata
        .iter()
        .find(|object| object.id == source.uuid.to_string())
        .map(|object| object.revision)
        .unwrap_or(0);
    store.put_object("mvr_fixture", &source.uuid.to_string(), &serde_json::json!({"fixture_id":id,"gdtf_spec":source.gdtf_spec,"gdtf_mode":source.gdtf_mode}), meta_current).map_err(ApiError::store)?;
    Ok(id)
}

pub(super) fn apply_mvr_to_store(
    store: &ShowStore,
    document: &light_mvr::MvrDocument,
    definitions: &[light_fixture::FixtureDefinition],
    resolutions: &HashMap<Uuid, MvrResolution>,
) -> Result<(usize, usize, Vec<String>), ApiError> {
    let existing_objects = store.objects("patched_fixture").map_err(ApiError::store)?;
    let mut occupied = occupied_patches(&existing_objects);
    let metadata = store.objects("mvr_fixture").map_err(ApiError::store)?;
    let ids = mvr_fixture_ids(&metadata);
    let mut imported = 0;
    let mut unresolved = 0;
    let mut warnings = Vec::new();
    for source in &document.fixtures {
        if matches!(resolutions.get(&source.uuid), Some(MvrResolution::Skip)) {
            continue;
        }
        let Some(definition) = resolve_mvr_definition(definitions, source) else {
            store_unresolved_mvr_fixture(store, source)?;
            unresolved += 1;
            warnings.push(format!(
                "{} requires {} mode {}",
                source.name, source.gdtf_spec, source.gdtf_mode
            ));
            continue;
        };
        let fixture_id = ids
            .get(&source.uuid)
            .and_then(|id| Uuid::parse_str(id).ok())
            .map(light_core::FixtureId)
            .unwrap_or_default();
        let address = resolved_mvr_address(
            store,
            source,
            fixture_id,
            &definition,
            resolutions.get(&source.uuid),
            &mut occupied,
            &mut warnings,
        )?;
        let patched =
            patched_mvr_fixture(source, &definition, fixture_id, address, &existing_objects);
        let id = store_resolved_mvr_fixture(store, source, &patched, &existing_objects, &metadata)?;
        let (universe, address) = address;
        if let (Some(u), Some(a)) = (universe, address) {
            occupied.push((u, a, definition.footprint, id));
        }
        imported += 1;
    }
    if !document.geometry.is_empty() {
        warnings.push(
            "MVR scene geometry was not imported. Add scenery from the Venue fixture library in Show Patch."
                .into(),
        );
    }
    Ok((imported, unresolved, warnings))
}
