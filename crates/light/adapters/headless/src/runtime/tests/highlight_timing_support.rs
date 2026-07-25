fn persistent_raw_values(
    programmer: &light_programmer::ProgrammerState,
) -> HashMap<light_core::AttributeKey, u32> {
    programmer
        .values
        .iter()
        .filter_map(|value| match value.value {
            light_core::AttributeValue::RawDmxExact(raw) => {
                Some((value.attribute.clone(), raw))
            }
            _ => None,
        })
        .collect()
}

fn transient_raw_values(
    programmer: &light_programmer::ProgrammerState,
) -> HashMap<light_core::AttributeKey, u32> {
    programmer
        .transient_values
        .iter()
        .flat_map(|action| &action.values)
        .filter_map(|value| match value.value {
            light_core::AttributeValue::RawDmxExact(raw) => {
                Some((value.attribute.clone(), raw))
            }
            _ => None,
        })
        .collect()
}

fn persisted_programmer(
    state: &AppState,
    session_id: SessionId,
) -> light_programmer::ProgrammerState {
    let session = state
        .desk
        .lock()
        .persisted_sessions()
        .unwrap()
        .into_iter()
        .find(|persisted| persisted.id == session_id)
        .unwrap();
    serde_json::from_str(&session.programmer_json).unwrap()
}
