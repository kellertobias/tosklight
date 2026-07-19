use super::*;

pub(super) struct PlaybackSlotUpsertPlan {
    pub(super) number: u16,
    pub(super) playback: light_playback::PlaybackDefinition,
    pub(super) page: light_playback::PlaybackPage,
    pub(super) mutations: Vec<light_application::ActiveShowObjectMutation>,
}

pub(super) fn plan_playback_slot_upsert(
    store: &ShowStore,
    page_number: u8,
    slot: u8,
    mut playback: light_playback::PlaybackDefinition,
    expected_playback_revision: u64,
    expected_page_revision: u64,
) -> Result<PlaybackSlotUpsertPlan, ApiError> {
    let playback_objects = store.objects("playback").map_err(ApiError::store)?;
    let page_objects = store.objects("playback_page").map_err(ApiError::store)?;
    let stored_page = page_objects
        .iter()
        .find(|object| object.id == page_number.to_string());
    let mut page = stored_page
        .map(decode_page)
        .transpose()?
        .unwrap_or_else(|| empty_page(page_number));
    let number = page
        .slots
        .get(&slot)
        .copied()
        .map_or_else(|| next_playback_number(&playback_objects), Ok)?;
    playback.number = number;
    page.number = page_number;
    page.slots.insert(slot, number);
    let mutations = vec![
        put_playback(playback.clone(), expected_playback_revision)?,
        put_page(page.clone(), expected_page_revision)?,
    ];
    Ok(PlaybackSlotUpsertPlan {
        number,
        playback,
        page,
        mutations,
    })
}

pub(super) struct PlaybackSlotClearPlan {
    pub(super) number: u16,
    pub(super) mutations: Vec<light_application::ActiveShowObjectMutation>,
}

pub(super) fn plan_playback_slot_clear(
    store: &ShowStore,
    page_number: u8,
    slot: u8,
    expected_playback_revision: u64,
    expected_page_revision: u64,
) -> Result<PlaybackSlotClearPlan, ApiError> {
    let pages = store.objects("playback_page").map_err(ApiError::store)?;
    let primary = pages
        .iter()
        .find(|object| object.id == page_number.to_string())
        .ok_or_else(|| ApiError::not_found("playback page"))?;
    let number = decode_page(primary)?
        .slots
        .get(&slot)
        .copied()
        .ok_or_else(|| ApiError::not_found("paged playback"))?;
    let mut mutations = pages
        .iter()
        .filter_map(|object| {
            page_without_playback(object, number, page_number, expected_page_revision)
        })
        .collect::<Result<Vec<_>, _>>()?;
    mutations.push(delete_active_show_object(
        light_application::ActiveShowObjectKind::Playback,
        number.to_string(),
        expected_playback_revision,
    ));
    Ok(PlaybackSlotClearPlan { number, mutations })
}

fn page_without_playback(
    object: &light_show::VersionedObject,
    number: u16,
    primary_page: u8,
    expected_page_revision: u64,
) -> Option<Result<light_application::ActiveShowObjectMutation, ApiError>> {
    let mut page = match decode_page(object) {
        Ok(page) => page,
        Err(error) => return Some(Err(error)),
    };
    let before = page.slots.len();
    page.slots.retain(|_, playback| *playback != number);
    (page.slots.len() != before).then(|| {
        let expected = if object.id == primary_page.to_string() {
            expected_page_revision
        } else {
            object.revision
        };
        put_page(page, expected)
    })
}

fn put_playback(
    playback: light_playback::PlaybackDefinition,
    expected: u64,
) -> Result<light_application::ActiveShowObjectMutation, ApiError> {
    playback.validate().map_err(ApiError::bad_request)?;
    Ok(put_active_show_object(
        light_application::ActiveShowObjectKind::Playback,
        playback.number.to_string(),
        expected,
        serde_json::to_value(playback).map_err(|error| ApiError::internal(error.to_string()))?,
    ))
}

pub(super) fn put_page(
    page: light_playback::PlaybackPage,
    expected: u64,
) -> Result<light_application::ActiveShowObjectMutation, ApiError> {
    page.validate().map_err(ApiError::bad_request)?;
    Ok(put_active_show_object(
        light_application::ActiveShowObjectKind::PlaybackPage,
        page.number.to_string(),
        expected,
        serde_json::to_value(page).map_err(|error| ApiError::internal(error.to_string()))?,
    ))
}

fn decode_page(
    object: &light_show::VersionedObject,
) -> Result<light_playback::PlaybackPage, ApiError> {
    serde_json::from_value(object.body.clone())
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

fn empty_page(number: u8) -> light_playback::PlaybackPage {
    light_playback::PlaybackPage {
        number,
        name: format!("Page {number}"),
        slots: HashMap::new(),
    }
}

fn next_playback_number(objects: &[light_show::VersionedObject]) -> Result<u16, ApiError> {
    let used = objects
        .iter()
        .filter_map(|object| object.id.parse::<u16>().ok())
        .collect::<HashSet<_>>();
    (1..=light_playback::MAX_PLAYBACKS)
        .find(|number| !used.contains(number))
        .ok_or_else(|| ApiError::bad_request("playback pool is full"))
}

pub(super) fn changed_revision(
    result: &light_application::MutateActiveShowObjectsResult,
    kind: light_application::ActiveShowObjectKind,
    id: &str,
) -> u64 {
    result
        .changes
        .iter()
        .find(|change| change.kind == kind && change.object_id == id)
        .map(|change| change.object_revision)
        .expect("the committed batch returns every requested object change")
}
