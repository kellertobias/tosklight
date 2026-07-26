use super::*;

pub(super) enum PlaybackPageAvailability {
    Missing,
    Existing,
    Created { event_sequence: u64 },
}

impl PlaybackPageAvailability {
    pub(super) const fn available(&self) -> bool {
        !matches!(self, Self::Missing)
    }

    pub(super) const fn event_sequence(&self) -> Option<u64> {
        match self {
            Self::Created { event_sequence } => Some(*event_sequence),
            Self::Missing | Self::Existing => None,
        }
    }
}

pub(super) fn ensure_playback_page_for_advance(
    state: &AppState,
    show: &ShowEntry,
    requested: u8,
    context: &light_application::ActionContext,
) -> Result<PlaybackPageAvailability, ApiError> {
    let snapshot = state.output.snapshot();
    if snapshot
        .playback_pages
        .iter()
        .any(|page| page.number == requested)
    {
        return Ok(PlaybackPageAvailability::Existing);
    }
    let Some(last) = snapshot
        .playback_pages
        .iter()
        .max_by_key(|page| page.number)
    else {
        return Ok(PlaybackPageAvailability::Missing);
    };
    if last.slots.is_empty() || last.number.checked_add(1) != Some(requested) {
        return Ok(PlaybackPageAvailability::Missing);
    }
    let page = light_playback::PlaybackPage {
        number: requested,
        name: format!("Page {requested}"),
        slots: HashMap::new(),
    };
    let mutation = playback_layout_mutations::put_page(page, 0)?;
    let action = active_show_object_action(context.clone(), show.id, vec![mutation]);
    let result = run_active_show_object_action(state, action)?;
    let change = result
        .changes
        .first()
        .expect("page creation returns one object change");
    emit(
        state,
        "show_object_changed",
        serde_json::json!({"show_id":show.id,"kind":"playback_page","id":requested.to_string(),"revision":change.object_revision}),
    );
    Ok(PlaybackPageAvailability::Created {
        event_sequence: result.event_sequence,
    })
}
