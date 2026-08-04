use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::attribute_configuration as wire;
use std::collections::VecDeque;

pub(super) const ATTRIBUTE_CONFIGURATION_KIND: &str = "attribute_configuration";
pub(super) const ATTRIBUTE_CONFIGURATION_ID: &str = "default";
const REPLAY_LIMIT: usize = 1_024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReplayKey {
    session_id: Uuid,
    show_id: light_core::ShowId,
    request_id: String,
}

#[derive(Clone)]
struct ReplayEntry {
    key: ReplayKey,
    request: wire::AttributeConfigurationUpdateRequest,
    outcome: wire::AttributeConfigurationUpdateOutcome,
}

#[derive(Default)]
pub(super) struct AttributeConfigurationReplayCache {
    entries: VecDeque<ReplayEntry>,
}

impl AttributeConfigurationReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        request: &wire::AttributeConfigurationUpdateRequest,
    ) -> Result<Option<wire::AttributeConfigurationUpdateOutcome>, ApiError> {
        let Some(entry) = self.entries.iter().find(|entry| &entry.key == key) else {
            return Ok(None);
        };
        if &entry.request != request {
            return Err(ApiError::conflict(
                "request_id was already used for a different Attribute configuration update",
            ));
        }
        let mut replayed = entry.outcome.clone();
        replayed.replayed = true;
        Ok(Some(replayed))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        request: wire::AttributeConfigurationUpdateRequest,
        outcome: wire::AttributeConfigurationUpdateOutcome,
    ) {
        self.entries.push_back(ReplayEntry {
            key,
            request,
            outcome,
        });
        while self.entries.len() > REPLAY_LIMIT {
            self.entries.pop_front();
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct InstalledAttributeConfiguration {
    pub(super) show_id: Option<light_core::ShowId>,
    pub(super) show_revision: u64,
    pub(super) object_revision: u64,
    pub(super) configuration: light_core::AttributeConfiguration,
    pub(super) validation_error: Option<String>,
}

impl InstalledAttributeConfiguration {
    pub(super) fn recommended(show_id: Option<light_core::ShowId>, show_revision: u64) -> Self {
        Self {
            show_id,
            show_revision,
            object_revision: 0,
            configuration: light_core::AttributeConfiguration::recommended(),
            validation_error: None,
        }
    }

    pub(super) fn for_entry(entry: Option<&ShowEntry>) -> Self {
        let Some(entry) = entry else {
            return Self::recommended(None, 0);
        };
        let result = ActiveShowRepository::open(&entry.path)
            .and_then(|store| store.portable_document())
            .map(|document| Self::for_document(&document));
        match result {
            Ok(installed) => installed,
            Err(error) => {
                let mut installed = Self::recommended(Some(entry.id), entry.revision);
                installed.validation_error = Some(format!(
                    "Attribute configuration could not be loaded; recommended defaults are active: {error}"
                ));
                installed
            }
        }
    }

    pub(super) fn for_document(document: &light_show::PortableShowDocument) -> Self {
        let Some(object) =
            document.object(ATTRIBUTE_CONFIGURATION_KIND, ATTRIBUTE_CONFIGURATION_ID)
        else {
            return Self::recommended(Some(document.id()), document.revision().value());
        };
        let decoded =
            serde_json::from_value::<light_core::AttributeConfiguration>(object.body().clone())
                .and_then(|configuration| {
                    configuration
                        .migrate_canonical_attributes()
                        .map_err(<serde_json::Error as serde::de::Error>::custom)
                })
                .map(light_core::AttributeConfiguration::with_current_built_ins)
                .and_then(|configuration| {
                    configuration
                        .validate()
                        .map_err(<serde_json::Error as serde::de::Error>::custom)?;
                    Ok(configuration)
                });
        match decoded {
            Ok(configuration) => Self {
                show_id: Some(document.id()),
                show_revision: document.revision().value(),
                object_revision: object.revision(),
                configuration,
                validation_error: None,
            },
            Err(error) => {
                let mut installed =
                    Self::recommended(Some(document.id()), document.revision().value());
                installed.object_revision = object.revision();
                installed.validation_error = Some(format!(
                    "Stored Attribute configuration is invalid; recommended defaults are active and the original object is preserved: {error}"
                ));
                installed
            }
        }
    }
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/attribute-configuration", get(snapshot))
        .route(
            "/api/v2/attribute-configuration/update",
            post(update_configuration),
        )
}

async fn snapshot(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
) -> Result<Json<wire::AttributeConfigurationSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    let installed = state.attributes.snapshot();
    if installed.show_id != Some(show_id) {
        return Err(ApiError::conflict(
            "Attribute configuration is not ready for the active show",
        ));
    }
    Ok(Json(wire_snapshot(&installed)))
}

async fn update_configuration(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::AttributeConfigurationUpdateRequest>,
) -> Result<Json<wire::AttributeConfigurationUpdateOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show_objects_v2::validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let key = ReplayKey {
        session_id: session.id.0,
        show_id,
        request_id: request.request_id.clone(),
    };
    if let Some(outcome) = state.attributes.replay(&key, &request).await? {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state.attributes.replay(&key, &request).await? {
        return Ok(Json(outcome));
    }
    let before = state.attributes.snapshot();
    if before.show_id != Some(show_id) || before.show_revision != request.expected_show_revision {
        return Err(ApiError::conflict(format!(
            "Attribute configuration show revision conflict: expected {}, actual {}",
            request.expected_show_revision, before.show_revision
        )));
    }
    if before.object_revision != request.expected_object_revision {
        return Err(ApiError::conflict(format!(
            "Attribute configuration object revision conflict: expected {}, actual {}",
            request.expected_object_revision, before.object_revision
        )));
    }
    let configuration = apply_patch(&before.configuration, request.patch.clone())?;
    configuration
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::AttributeConfiguration,
            ATTRIBUTE_CONFIGURATION_ID,
            request.expected_object_revision,
            serde_json::to_value(configuration)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        )?],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action).await?;
    let change = result
        .changes
        .first()
        .expect("one Attribute configuration mutation returns one change");
    emit(
        &state,
        "attribute_configuration_changed",
        serde_json::json!({
            "show_id": show_id,
            "show_revision": result.show_revision,
            "object_revision": change.object_revision,
        }),
    );
    let outcome = wire::AttributeConfigurationUpdateOutcome {
        request_id: request.request_id.clone(),
        replayed: false,
        snapshot: wire_snapshot(&state.attributes.snapshot()),
        event_sequence: result.event_sequence,
    };
    state
        .attributes
        .remember(key, request, outcome.clone())
        .await;
    Ok(Json(outcome))
}

fn wire_snapshot(
    installed: &InstalledAttributeConfiguration,
) -> wire::AttributeConfigurationSnapshot {
    wire::AttributeConfigurationSnapshot {
        show_id: installed.show_id.map(|id| id.0),
        show_revision: installed.show_revision,
        object_revision: installed.object_revision,
        configuration: wire_configuration(&installed.configuration),
        recommended_configuration: wire_configuration(
            &light_core::AttributeConfiguration::recommended(),
        ),
        descriptors: configured_descriptors(&installed.configuration),
        validation_error: installed.validation_error.clone(),
    }
}

pub(super) fn configured_descriptors(
    configuration: &light_core::AttributeConfiguration,
) -> Vec<wire::ConfiguredAttributeDescriptor> {
    let mut descriptors = light_core::ATTRIBUTE_REGISTRY
        .iter()
        .filter_map(|descriptor| {
            let placement = configuration
                .attribute_placement_for(&light_core::AttributeKey(descriptor.id.into()))?;
            Some(wire::ConfiguredAttributeDescriptor {
                id: descriptor.id.into(),
                label: descriptor.label.into(),
                encoder_group: wire_encoder_group(placement.encoder.group),
                encoder_page: placement.encoder.page,
                encoder_slot: placement.encoder.slot,
                value_type: wire_value_type(descriptor.value_type),
                display_unit: descriptor.display_unit.map(str::to_owned),
                physical_unit: descriptor.physical_unit.map(str::to_owned),
                normalized_min: descriptor.normalized_bounds.map(|bounds| bounds.min),
                normalized_max: descriptor.normalized_bounds.map(|bounds| bounds.max),
                domain_min: descriptor.domain_bounds.map(|bounds| bounds.min),
                domain_max: descriptor.domain_bounds.map(|bounds| bounds.max),
                cyclic: descriptor.cyclic,
                recordable: descriptor.recordable,
                built_in: true,
                retired: light_core::built_in_attribute_is_retired(descriptor.id),
                activation_group_id: configuration
                    .activation_group_for(&light_core::AttributeKey(descriptor.id.into()))
                    .map(|group| group.id.clone()),
                push_turn_of: placement
                    .push_turn_of
                    .as_ref()
                    .map(|attribute| attribute.0.clone()),
            })
        })
        .collect::<Vec<_>>();
    descriptors.extend(
        configuration
            .custom_attributes
            .iter()
            .filter_map(|descriptor| {
                let placement = configuration.attribute_placement_for(&descriptor.id)?;
                Some(wire::ConfiguredAttributeDescriptor {
                    id: descriptor.id.0.clone(),
                    label: descriptor.label.clone(),
                    encoder_group: wire_encoder_group(placement.encoder.group),
                    encoder_page: placement.encoder.page,
                    encoder_slot: placement.encoder.slot,
                    value_type: wire_value_type(descriptor.value_type),
                    display_unit: descriptor.display_unit.clone(),
                    physical_unit: descriptor.physical_unit.clone(),
                    normalized_min: descriptor.normalized_bounds.map(|bounds| bounds.min),
                    normalized_max: descriptor.normalized_bounds.map(|bounds| bounds.max),
                    domain_min: descriptor.domain_bounds.map(|bounds| bounds.min),
                    domain_max: descriptor.domain_bounds.map(|bounds| bounds.max),
                    cyclic: descriptor.cyclic,
                    recordable: descriptor.recordable,
                    built_in: false,
                    retired: descriptor.lifecycle == light_core::CustomAttributeLifecycle::Retired,
                    activation_group_id: configuration
                        .activation_group_for(&descriptor.id)
                        .map(|group| group.id.clone()),
                    push_turn_of: placement
                        .push_turn_of
                        .as_ref()
                        .map(|attribute| attribute.0.clone()),
                })
            }),
    );
    descriptors.sort_by_key(|descriptor| {
        (
            encoder_group_order(descriptor.encoder_group),
            descriptor.encoder_page,
            descriptor.encoder_slot,
            descriptor.id.clone(),
        )
    });
    descriptors
}

fn wire_configuration(
    configuration: &light_core::AttributeConfiguration,
) -> wire::AttributeConfiguration {
    wire::AttributeConfiguration {
        version: configuration.version,
        custom_attributes: configuration
            .custom_attributes
            .iter()
            .map(|descriptor| wire::CustomAttributeDescriptor {
                id: descriptor.id.0.clone(),
                label: descriptor.label.clone(),
                value_type: wire_value_type(descriptor.value_type),
                display_unit: descriptor.display_unit.clone(),
                physical_unit: descriptor.physical_unit.clone(),
                normalized_bounds: descriptor.normalized_bounds.map(wire_bounds),
                domain_bounds: descriptor.domain_bounds.map(wire_bounds),
                cyclic: descriptor.cyclic,
                recordable: descriptor.recordable,
                lifecycle: match descriptor.lifecycle {
                    light_core::CustomAttributeLifecycle::Active => {
                        wire::CustomAttributeLifecycle::Active
                    }
                    light_core::CustomAttributeLifecycle::Retired => {
                        wire::CustomAttributeLifecycle::Retired
                    }
                },
            })
            .collect(),
        placements: configuration
            .placements
            .iter()
            .map(|placement| wire::AttributePlacement {
                attribute: placement.attribute.0.clone(),
                encoder_group: wire_encoder_group(placement.encoder.group),
                encoder_page: placement.encoder.page,
                encoder_slot: placement.encoder.slot,
                push_turn_of: placement
                    .push_turn_of
                    .as_ref()
                    .map(|attribute| attribute.0.clone()),
            })
            .collect(),
        activation_groups: configuration
            .activation_groups
            .iter()
            .map(|group| wire::AttributeActivationGroup {
                id: group.id.clone(),
                label: group.label.clone(),
                members: group
                    .members
                    .iter()
                    .map(|member| member.0.clone())
                    .collect(),
            })
            .collect(),
    }
}

fn apply_patch(
    before: &light_core::AttributeConfiguration,
    patch: wire::AttributeConfigurationPatch,
) -> Result<light_core::AttributeConfiguration, ApiError> {
    if patch.custom_attributes.is_none()
        && patch.placements.is_none()
        && patch.activation_groups.is_none()
    {
        return Err(ApiError::bad_request(
            "Attribute configuration update requires at least one changed field",
        ));
    }
    let mut configuration = before.clone();
    if let Some(custom_attributes) = patch.custom_attributes {
        configuration.custom_attributes = custom_attributes
            .into_iter()
            .map(|descriptor| light_core::CustomAttributeDescriptor {
                id: light_core::AttributeKey(descriptor.id),
                label: descriptor.label,
                value_type: domain_value_type(descriptor.value_type),
                display_unit: descriptor.display_unit,
                physical_unit: descriptor.physical_unit,
                normalized_bounds: descriptor.normalized_bounds.map(domain_bounds),
                domain_bounds: descriptor.domain_bounds.map(domain_bounds),
                cyclic: descriptor.cyclic,
                recordable: descriptor.recordable,
                lifecycle: match descriptor.lifecycle {
                    wire::CustomAttributeLifecycle::Active => {
                        light_core::CustomAttributeLifecycle::Active
                    }
                    wire::CustomAttributeLifecycle::Retired => {
                        light_core::CustomAttributeLifecycle::Retired
                    }
                },
            })
            .collect();
    }
    if let Some(placements) = patch.placements {
        configuration.placements = placements
            .into_iter()
            .map(|placement| light_core::AttributePlacement {
                attribute: light_core::AttributeKey(placement.attribute),
                encoder: light_core::EncoderPlacement::new(
                    domain_encoder_group(placement.encoder_group),
                    placement.encoder_page,
                    placement.encoder_slot,
                ),
                push_turn_of: placement.push_turn_of.map(light_core::AttributeKey),
            })
            .collect();
    }
    if let Some(activation_groups) = patch.activation_groups {
        configuration.activation_groups = activation_groups
            .into_iter()
            .map(|group| light_core::AttributeActivationGroup {
                id: group.id,
                label: group.label,
                members: group
                    .members
                    .into_iter()
                    .map(light_core::AttributeKey)
                    .collect(),
            })
            .collect();
    }
    Ok(configuration)
}

fn wire_bounds(bounds: light_core::AttributeBounds) -> wire::AttributeBounds {
    wire::AttributeBounds {
        min: bounds.min,
        max: bounds.max,
    }
}

fn domain_bounds(bounds: wire::AttributeBounds) -> light_core::AttributeBounds {
    light_core::AttributeBounds {
        min: bounds.min,
        max: bounds.max,
    }
}

fn wire_value_type(value: light_core::AttributeValueType) -> wire::AttributeValueType {
    match value {
        light_core::AttributeValueType::Continuous => wire::AttributeValueType::Continuous,
        light_core::AttributeValueType::Color => wire::AttributeValueType::Color,
        light_core::AttributeValueType::Indexed => wire::AttributeValueType::Indexed,
        light_core::AttributeValueType::Control => wire::AttributeValueType::Control,
    }
}

fn domain_value_type(value: wire::AttributeValueType) -> light_core::AttributeValueType {
    match value {
        wire::AttributeValueType::Continuous => light_core::AttributeValueType::Continuous,
        wire::AttributeValueType::Color => light_core::AttributeValueType::Color,
        wire::AttributeValueType::Indexed => light_core::AttributeValueType::Indexed,
        wire::AttributeValueType::Control => light_core::AttributeValueType::Control,
    }
}

fn wire_encoder_group(value: light_core::EncoderGroup) -> wire::AttributeEncoderGroup {
    match value {
        light_core::EncoderGroup::Intensity => wire::AttributeEncoderGroup::Intensity,
        light_core::EncoderGroup::Color => wire::AttributeEncoderGroup::Color,
        light_core::EncoderGroup::Position => wire::AttributeEncoderGroup::Position,
        light_core::EncoderGroup::Beam => wire::AttributeEncoderGroup::Beam,
        light_core::EncoderGroup::Shapers => wire::AttributeEncoderGroup::Shapers,
        light_core::EncoderGroup::Focus => wire::AttributeEncoderGroup::Focus,
        light_core::EncoderGroup::Control => wire::AttributeEncoderGroup::Control,
        light_core::EncoderGroup::Media => wire::AttributeEncoderGroup::Media,
    }
}

fn domain_encoder_group(value: wire::AttributeEncoderGroup) -> light_core::EncoderGroup {
    match value {
        wire::AttributeEncoderGroup::Intensity => light_core::EncoderGroup::Intensity,
        wire::AttributeEncoderGroup::Color => light_core::EncoderGroup::Color,
        wire::AttributeEncoderGroup::Position => light_core::EncoderGroup::Position,
        wire::AttributeEncoderGroup::Beam => light_core::EncoderGroup::Beam,
        wire::AttributeEncoderGroup::Shapers => light_core::EncoderGroup::Shapers,
        wire::AttributeEncoderGroup::Focus => light_core::EncoderGroup::Focus,
        wire::AttributeEncoderGroup::Control => light_core::EncoderGroup::Control,
        wire::AttributeEncoderGroup::Media => light_core::EncoderGroup::Media,
    }
}

const fn encoder_group_order(value: wire::AttributeEncoderGroup) -> u8 {
    match value {
        wire::AttributeEncoderGroup::Intensity => 0,
        wire::AttributeEncoderGroup::Color => 1,
        wire::AttributeEncoderGroup::Position => 2,
        wire::AttributeEncoderGroup::Beam => 3,
        wire::AttributeEncoderGroup::Shapers => 4,
        wire::AttributeEncoderGroup::Focus => 5,
        wire::AttributeEncoderGroup::Control => 6,
        wire::AttributeEncoderGroup::Media => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_object_projects_defaults_without_mutating_the_show() {
        let path = std::env::temp_dir().join(format!(
            "light-attribute-configuration-{}.show",
            uuid::Uuid::new_v4()
        ));
        let (store, _) = light_show::ShowStore::create(&path, "Attributes").unwrap();
        let document = store.portable_document().unwrap();
        let installed = InstalledAttributeConfiguration::for_document(&document);

        installed.configuration.validate().unwrap();
        assert_eq!(installed.object_revision, 0);
        assert_eq!(installed.show_revision, document.revision().value());
        assert!(installed.validation_error.is_none());
        assert!(
            store
                .portable_document()
                .unwrap()
                .object(ATTRIBUTE_CONFIGURATION_KIND, ATTRIBUTE_CONFIGURATION_ID)
                .is_none()
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn malformed_object_is_preserved_while_runtime_uses_safe_defaults() {
        let path = std::env::temp_dir().join(format!(
            "light-invalid-attribute-configuration-{}.show",
            uuid::Uuid::new_v4()
        ));
        let (store, _) = light_show::ShowStore::create(&path, "Attributes").unwrap();
        store
            .put_object(
                ATTRIBUTE_CONFIGURATION_KIND,
                ATTRIBUTE_CONFIGURATION_ID,
                &serde_json::json!({"version": 999, "future": {"keep": true}}),
                0,
            )
            .unwrap();
        let document = store.portable_document().unwrap();
        let installed = InstalledAttributeConfiguration::for_document(&document);

        installed.configuration.validate().unwrap();
        assert!(installed.validation_error.is_some());
        assert_eq!(
            store
                .portable_document()
                .unwrap()
                .object(ATTRIBUTE_CONFIGURATION_KIND, ATTRIBUTE_CONFIGURATION_ID)
                .unwrap()
                .body()["future"],
            serde_json::json!({"keep": true})
        );
        std::fs::remove_file(path).unwrap();
    }
}
