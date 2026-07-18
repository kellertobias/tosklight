use super::*;

pub(super) fn ws_selection_set(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        fixtures: Vec<light_core::FixtureId>,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let snapshot = state.engine.snapshot();
    state.programmers.select(
        session.id,
        expand_selectable_fixture_ids(&snapshot.fixtures, input.fixtures),
    );
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
}

pub(super) fn ws_selection_gesture(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum Source {
        Fixture { fixture_id: light_core::FixtureId },
        LiveGroup { group_id: String },
        DereferencedGroup { group_id: String },
    }
    #[derive(Deserialize)]
    struct Input {
        source: Source,
        #[serde(default)]
        remove: bool,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let snapshot = state.engine.snapshot();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let references = match input.source {
        Source::Fixture { fixture_id } => {
            let Some(fixture) = snapshot.fixtures.iter().find(|fixture| {
                fixture.fixture_id == fixture_id
                    || fixture
                        .logical_heads
                        .iter()
                        .any(|head| head.fixture_id == fixture_id)
            }) else {
                return Err("fixture does not exist".into());
            };
            let selectable = if fixture.fixture_id == fixture_id {
                selectable_fixture_ids(fixture)
            } else {
                vec![fixture_id]
            };
            selectable
                .into_iter()
                .map(|fixture_id| {
                    if input.remove {
                        light_programmer::SelectionReference::RemoveFixture { fixture_id }
                    } else {
                        light_programmer::SelectionReference::Fixture { fixture_id }
                    }
                })
                .collect()
        }
        Source::LiveGroup { group_id } => {
            light_programmer::resolve_group(&group_id, &groups)?;
            vec![if input.remove {
                light_programmer::SelectionReference::RemoveLiveGroup { group_id }
            } else {
                light_programmer::SelectionReference::LiveGroup { group_id }
            }]
        }
        Source::DereferencedGroup { group_id } => {
            light_programmer::resolve_group(&group_id, &groups)?
                .into_iter()
                .map(|fixture_id| {
                    if input.remove {
                        light_programmer::SelectionReference::RemoveFixture { fixture_id }
                    } else {
                        light_programmer::SelectionReference::Fixture { fixture_id }
                    }
                })
                .collect()
        }
    };
    state
        .programmers
        .apply_selection_gesture(session.id, references, &groups);
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
}

pub(super) fn ws_group_select(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        group_id: String,
        #[serde(default)]
        frozen: bool,
        rule: Option<light_programmer::SelectionRule>,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let snapshot = state.engine.snapshot();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let mut fixtures = light_programmer::resolve_group(&input.group_id, &groups)?;
    let rule = input.rule.unwrap_or(light_programmer::SelectionRule::All);
    rule.validate()?;
    fixtures = light_programmer::apply_selection_rule(&fixtures, &rule);
    let expression = if input.frozen {
        light_programmer::SelectionExpression::FrozenGroup {
            group_id: input.group_id,
            source_revision: snapshot.revision,
        }
    } else {
        light_programmer::SelectionExpression::LiveGroup {
            group_id: input.group_id,
            rule,
        }
    };
    state
        .programmers
        .select_expression(session.id, fixtures, expression);
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
}

pub(super) fn ws_selection_macro(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        rule: light_programmer::SelectionRule,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    input.rule.validate()?;
    let current = state
        .programmers
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let (base, expression) = match current.selection_expression {
        Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. }) => {
            let snapshot = state.engine.snapshot();
            let groups = snapshot
                .groups
                .iter()
                .map(|group| (group.id.clone(), group.clone()))
                .collect::<HashMap<_, _>>();
            let base = light_programmer::resolve_group(&group_id, &groups)?;
            (
                base,
                light_programmer::SelectionExpression::LiveGroup {
                    group_id,
                    rule: input.rule.clone(),
                },
            )
        }
        _ => (
            current.selected,
            light_programmer::SelectionExpression::Static,
        ),
    };
    let fixtures = light_programmer::apply_selection_rule(&base, &input.rule);
    state
        .programmers
        .select_expression(session.id, fixtures, expression);
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
}

pub(super) fn ws_programmer_align(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
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
        .programmers
        .get(session.id)
        .ok_or("programmer does not exist")?
        .selected;
    let snapshot = state.engine.snapshot();
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
        state.programmers.set(
            session.id,
            *fixture,
            light_core::AttributeKey(input.attribute.clone()),
            light_core::AttributeValue::Normalized(value),
        );
    }
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(
        serde_json::json!({"programmer":state.programmers.get(session.id),"unsupported_fixtures":unsupported}),
    )
}
