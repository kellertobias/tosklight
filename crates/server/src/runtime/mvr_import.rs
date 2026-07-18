use super::*;

pub(super) async fn preview_mvr_import(
    State(state): State<AppState>,
    Query(query): Query<MvrPreviewQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<MvrImportPreview>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let document =
        light_mvr::read(&body).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let (definitions, _) = mvr_definitions(&state, &document)?;
    let mut existing = Vec::new();
    if let Some(id) = query.show_id
        && let Some(show) = state
            .desk
            .lock()
            .show(light_core::ShowId(id))
            .map_err(ApiError::store)?
    {
        existing = ShowStore::open(show.path)
            .map_err(ApiError::store)?
            .objects("patched_fixture")
            .map_err(ApiError::store)?
            .into_iter()
            .filter_map(|o| serde_json::from_value::<light_fixture::PatchedFixture>(o.body).ok())
            .collect();
    }
    let missing_profiles = document
        .fixtures
        .iter()
        .filter(|f| resolve_mvr_definition(&definitions, f).is_none())
        .map(|f| format!("{} · {}", f.gdtf_spec, f.gdtf_mode))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut address_conflicts = Vec::new();
    for fixture in &document.fixtures {
        if let (Some(u), Some(a), Some(definition)) = (
            fixture.universe,
            fixture.address,
            resolve_mvr_definition(&definitions, fixture),
        ) {
            let end = a.saturating_add(definition.footprint.saturating_sub(1));
            if existing.iter().any(|e| {
                e.universe == Some(u)
                    && e.address.is_some_and(|start| {
                        start <= end
                            && start.saturating_add(e.definition.footprint.saturating_sub(1)) >= a
                    })
            }) {
                address_conflicts.push(format!(
                    "{} conflicts at universe {} address {}-{}",
                    fixture.name, u, a, end
                ));
            }
        }
    }
    let token = Uuid::new_v4();
    let now = Instant::now();
    let mut imports = state.mvr_imports.lock();
    imports.retain(|_, item| now.duration_since(item.created) < Duration::from_secs(30 * 60));
    imports.insert(
        token,
        StagedMvrImport {
            document: document.clone(),
            created: now,
        },
    );
    Ok(Json(MvrImportPreview {
        token,
        fixtures: document
            .fixtures
            .iter()
            .map(|f| MvrPreviewFixture {
                uuid: f.uuid,
                name: f.name.clone(),
                gdtf_spec: f.gdtf_spec.clone(),
                gdtf_mode: f.gdtf_mode.clone(),
                universe: f.universe,
                address: f.address,
                matched: resolve_mvr_definition(&definitions, f).is_some(),
            })
            .collect(),
        scenery: document.geometry.len(),
        missing_profiles,
        warnings: address_conflicts.clone(),
        address_conflicts,
    }))
}

pub(super) fn resolve_mvr_definition(
    definitions: &[light_fixture::FixtureDefinition],
    fixture: &light_mvr::MvrFixture,
) -> Option<light_fixture::FixtureDefinition> {
    let spec = fixture
        .gdtf_spec
        .rsplit('/')
        .next()
        .unwrap_or(&fixture.gdtf_spec)
        .trim_end_matches(".gdtf");
    definitions
        .iter()
        .find(|d| {
            d.mode.eq_ignore_ascii_case(&fixture.gdtf_mode)
                && (d.model.eq_ignore_ascii_case(spec)
                    || d.name.eq_ignore_ascii_case(spec)
                    || format!("{}@{}", d.manufacturer, d.model).eq_ignore_ascii_case(spec))
        })
        .cloned()
}

pub(super) type MvrDefinitions = (
    Vec<light_fixture::FixtureDefinition>,
    Vec<(light_fixture::FixtureDefinition, Vec<u8>)>,
);

pub(super) fn mvr_definitions(
    state: &AppState,
    document: &light_mvr::MvrDocument,
) -> Result<MvrDefinitions, ApiError> {
    let mut definitions = state
        .fixture_library
        .lock()
        .definitions()
        .map_err(ApiError::fixture)?;
    let mut imported = Vec::new();
    for fixture in &document.fixtures {
        if resolve_mvr_definition(&definitions, fixture).is_some() {
            continue;
        }
        let name = fixture.gdtf_spec.to_ascii_lowercase();
        let Some(bytes) = document.files.get(&name).or_else(|| {
            document
                .files
                .iter()
                .find(|(path, _)| path.ends_with(&format!("/{name}")))
                .map(|(_, data)| data)
        }) else {
            continue;
        };
        let Ok(modes) = light_mvr::read_gdtf(bytes) else {
            continue;
        };
        for mode in modes {
            let footprint = mode
                .channels
                .iter()
                .flat_map(|c| c.offsets.iter())
                .max()
                .copied()
                .unwrap_or(0)
                + 1;
            let parameters = mode
                .channels
                .into_iter()
                .map(|channel| {
                    let normalized = channel
                        .attribute
                        .replace([' ', '_'], ".")
                        .to_ascii_lowercase();
                    light_fixture::Parameter {
                        attribute: light_core::AttributeKey(normalized.clone()),
                        components: channel
                            .offsets
                            .into_iter()
                            .map(|offset| light_fixture::ChannelComponent {
                                offset,
                                byte_order: light_fixture::ByteOrder::MsbFirst,
                            })
                            .collect(),
                        default: 0.0,
                        virtual_dimmer: false,
                        metadata: light_fixture::ParameterMetadata {
                            wrap: normalized.contains("pan"),
                            ..Default::default()
                        },
                        capabilities: Vec::new(),
                    }
                })
                .collect();
            let definition = light_fixture::FixtureDefinition {
                schema_version: 1,
                id: light_core::FixtureId::new(),
                revision: 1,
                manufacturer: mode.manufacturer,
                device_type: "other".into(),
                name: mode.model.clone(),
                model: mode.model,
                mode: mode.name,
                footprint,
                heads: vec![light_fixture::LogicalHead {
                    index: 0,
                    name: "Main".into(),
                    shared: true,
                    parameters,
                }],
                color_calibration: None,
                physical: Default::default(),
                model_asset: None,
                icon_asset: None,
                hazardous: false,
                direct_control_protocols: Vec::new(),
                signal_loss_policy: light_fixture::SignalLossPolicy::HoldLast,
                safe_values: Default::default(),
                profile_id: None,
                mode_id: None,
                profile_snapshot: None,
            };
            definitions.push(definition.clone());
            imported.push((definition, bytes.clone()));
        }
    }
    Ok((definitions, imported))
}
