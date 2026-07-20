use super::{
    ActiveCueContext, CueUpdateMode, ExistingContentMode, ProgrammingUpdateMenuEntry,
    ProgrammingUpdateMenuInput, ProgrammingUpdateObjectReference, ProgrammingUpdateTargetRequest,
    UpdateTargetFilter, preview_cue_update, preview_group_update, preview_preset_update,
};
use crate::ActiveShowObjectKind;
use light_core::CueListId;
use light_playback::CueList;
use light_programmer::{GroupDefinition, Preset, resolve_group};
use light_show::{PortableShowDocument, PortableShowObject};
use std::collections::{HashMap, HashSet};

pub(super) fn preview_update_menu(
    document: &PortableShowDocument,
    active: &[ActiveCueContext],
    input: &ProgrammingUpdateMenuInput,
    filter: UpdateTargetFilter,
) -> Vec<ProgrammingUpdateMenuEntry> {
    let catalogs = UpdateMenuCatalog::read(document, active, input);
    let mut entries = active
        .iter()
        .filter_map(|context| catalogs.cue_entry(context, input))
        .collect::<Vec<_>>();
    entries.extend(catalogs.preset_entry(input));
    entries.extend(catalogs.group_entries(input));
    entries
        .into_iter()
        .filter(|entry| menu_filter(entry, filter))
        .collect()
}

struct UpdateMenuCatalog<'a> {
    cue_lists: HashMap<CueListId, StoredObject<'a, CueList>>,
    ambiguous_cue_lists: HashSet<CueListId>,
    preset: Option<StoredObject<'a, Preset>>,
    groups: Option<GroupCatalog<'a>>,
}

impl<'a> UpdateMenuCatalog<'a> {
    fn read(
        document: &'a PortableShowDocument,
        active: &[ActiveCueContext],
        input: &ProgrammingUpdateMenuInput,
    ) -> Self {
        let (cue_lists, ambiguous_cue_lists) = if active.is_empty() {
            (HashMap::new(), HashSet::new())
        } else {
            cue_list_catalog(document)
        };
        let preset = input.active_preset_id.as_deref().and_then(|id| {
            document
                .object("preset", id)
                .and_then(|object| decode_object(object).ok())
        });
        Self {
            cue_lists,
            ambiguous_cue_lists,
            preset,
            groups: (!input.referenced_group_ids.is_empty())
                .then(|| group_catalog(document).ok())
                .flatten(),
        }
    }

    fn cue_entry(
        &self,
        context: &ActiveCueContext,
        input: &ProgrammingUpdateMenuInput,
    ) -> Option<ProgrammingUpdateMenuEntry> {
        if self.ambiguous_cue_lists.contains(&context.cue_list_id) {
            return None;
        }
        let stored = self.cue_lists.get(&context.cue_list_id)?;
        let existing = preview_cue_update(
            &stored.value,
            &context.into(),
            CueUpdateMode::ExistingOnly,
            &input.values,
        )
        .ok()?;
        let add_new = preview_cue_update(
            &stored.value,
            &context.into(),
            CueUpdateMode::AddNew,
            &input.values,
        )
        .ok()?;
        Some(ProgrammingUpdateMenuEntry {
            target: cue_target(context),
            object_revision: stored.object.revision(),
            object: object_reference(ActiveShowObjectKind::CueList, stored.object),
            programmer_revision: input.values_fingerprint.clone(),
            active_or_referenced: true,
            existing_preview: existing,
            add_new_preview: add_new,
        })
    }

    fn preset_entry(
        &self,
        input: &ProgrammingUpdateMenuInput,
    ) -> Option<ProgrammingUpdateMenuEntry> {
        let id = input.active_preset_id.as_deref()?;
        let stored = self.preset.as_ref()?;
        menu_entry(
            ProgrammingUpdateTargetRequest::Preset {
                object_id: id.to_owned(),
            },
            ActiveShowObjectKind::Preset,
            stored.object,
            &input.values_fingerprint,
            preview_preset_update(
                id,
                &stored.value,
                ExistingContentMode::UpdateExisting,
                &input.values,
            )
            .ok()?,
            preview_preset_update(
                id,
                &stored.value,
                ExistingContentMode::AddNew,
                &input.values,
            )
            .ok()?,
        )
    }

    fn group_entries(&self, input: &ProgrammingUpdateMenuInput) -> Vec<ProgrammingUpdateMenuEntry> {
        input
            .referenced_group_ids
            .iter()
            .filter_map(|id| {
                let groups = self.groups.as_ref()?;
                let object = groups.objects.get(id)?;
                let group = groups.definitions.get(id)?;
                let membership = resolve_group(id, &groups.definitions).ok()?;
                menu_entry(
                    ProgrammingUpdateTargetRequest::Group {
                        object_id: id.to_owned(),
                    },
                    ActiveShowObjectKind::Group,
                    object,
                    &input.selection_fingerprint,
                    preview_group_update(
                        group,
                        &membership,
                        ExistingContentMode::UpdateExisting,
                        &input.selection,
                    )
                    .ok()?,
                    preview_group_update(
                        group,
                        &membership,
                        ExistingContentMode::AddNew,
                        &input.selection,
                    )
                    .ok()?,
                )
            })
            .collect()
    }
}

struct StoredObject<'a, T> {
    object: &'a PortableShowObject,
    value: T,
}

struct GroupCatalog<'a> {
    objects: HashMap<String, &'a PortableShowObject>,
    definitions: HashMap<String, GroupDefinition>,
}

fn cue_list_catalog(
    document: &PortableShowDocument,
) -> (
    HashMap<CueListId, StoredObject<'_, CueList>>,
    HashSet<CueListId>,
) {
    let mut catalog = HashMap::new();
    let mut ambiguous = HashSet::new();
    for object in document.objects_of_kind("cue_list") {
        let Ok(stored) = decode_object::<CueList>(object) else {
            continue;
        };
        let id = stored.value.id;
        if catalog.insert(id, stored).is_some() {
            ambiguous.insert(id);
        }
    }
    (catalog, ambiguous)
}

fn group_catalog(document: &PortableShowDocument) -> Result<GroupCatalog<'_>, serde_json::Error> {
    let mut objects = HashMap::new();
    let mut definitions = HashMap::new();
    for object in document.objects_of_kind("group") {
        let mut group = serde_json::from_value::<GroupDefinition>(object.body().clone())?;
        let id = object.key().id().to_owned();
        group.id = id.clone();
        objects.insert(id.clone(), object);
        definitions.insert(id, group);
    }
    Ok(GroupCatalog {
        objects,
        definitions,
    })
}

fn decode_object<T: serde::de::DeserializeOwned>(
    object: &PortableShowObject,
) -> Result<StoredObject<'_, T>, serde_json::Error> {
    Ok(StoredObject {
        object,
        value: serde_json::from_value(object.body().clone())?,
    })
}

fn menu_entry(
    target: ProgrammingUpdateTargetRequest,
    kind: ActiveShowObjectKind,
    object: &PortableShowObject,
    programmer_revision: &str,
    existing_preview: super::UpdatePreview,
    add_new_preview: super::UpdatePreview,
) -> Option<ProgrammingUpdateMenuEntry> {
    Some(ProgrammingUpdateMenuEntry {
        target,
        object_revision: object.revision(),
        object: object_reference(kind, object),
        programmer_revision: programmer_revision.to_owned(),
        active_or_referenced: true,
        existing_preview,
        add_new_preview,
    })
}

fn object_reference(
    kind: ActiveShowObjectKind,
    object: &PortableShowObject,
) -> ProgrammingUpdateObjectReference {
    ProgrammingUpdateObjectReference {
        kind,
        object_id: object.key().id().to_owned(),
        object_revision: object.revision(),
    }
}

fn cue_target(context: &ActiveCueContext) -> ProgrammingUpdateTargetRequest {
    ProgrammingUpdateTargetRequest::Cue {
        cue_list_id: context.cue_list_id,
        playback_number: Some(context.playback_number),
        cue_id: Some(context.cue_id),
        cue_number: Some(context.cue_number),
        validate_active_context: true,
    }
}

fn menu_filter(entry: &ProgrammingUpdateMenuEntry, filter: UpdateTargetFilter) -> bool {
    match filter {
        UpdateTargetFilter::EligibleForUpdateExisting => entry.existing_preview.has_real_change(),
        UpdateTargetFilter::ShowAllActive => entry.active_or_referenced,
    }
}
