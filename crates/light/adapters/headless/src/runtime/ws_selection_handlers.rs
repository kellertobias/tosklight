use super::*;

pub(super) fn ws_programmer_align(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        attribute: String,
        mode: String,
        #[serde(default)]
        from: f32,
        #[serde(default = "one_f32")]
        to: f32,
    }
    fn one_f32() -> f32 {
        1.0
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let selected = state
        .programming
        .get(session.id)
        .ok_or("programmer does not exist")?
        .selected;
    let snapshot = state.output.snapshot();
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();
    for fixture_id in selected {
        let parameter = snapshot.fixtures.iter().find_map(|fixture| {
            let owns_parent = fixture.fixture_id == fixture_id;
            fixture.definition.heads.iter().find_map(|head| {
                let owns_head = head.shared && owns_parent
                    || fixture.logical_heads.iter().any(|patched| {
                        patched.fixture_id == fixture_id && patched.head_index == head.index
                    });
                owns_head
                    .then(|| {
                        head.parameters
                            .iter()
                            .find(|parameter| parameter.attribute.0 == input.attribute)
                    })
                    .flatten()
            })
        });
        match parameter {
            Some(parameter) if parameter.capabilities.is_empty() => {
                supported.push((fixture_id, parameter.metadata.wrap))
            }
            Some(_) => {
                return Err(format!(
                    "{} is discrete and cannot be aligned",
                    input.attribute
                ));
            }
            None => unsupported.push(fixture_id),
        }
    }
    if supported.is_empty() {
        return Err(format!(
            "none of the selected fixtures support {}",
            input.attribute
        ));
    }
    for (index, (fixture, wraps)) in supported.iter().enumerate() {
        let value = aligned_normalized(
            &input.mode,
            index,
            supported.len(),
            input.from,
            input.to,
            *wraps,
        )?;
        state.programming.set(
            session.id,
            *fixture,
            light_core::AttributeKey(input.attribute.clone()),
            light_core::AttributeValue::Normalized(value),
        );
    }
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(
        serde_json::json!({"programmer":state.programming.get(session.id),"unsupported_fixtures":unsupported}),
    )
}
