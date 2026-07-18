use super::*;

type SelectionGroups = HashMap<String, light_programmer::GroupDefinition>;

#[derive(Clone, Debug)]
pub(super) struct ParsedMixedSelection {
    pub(super) fixtures: Vec<light_core::FixtureId>,
    pub(super) sources: Vec<light_programmer::SelectionReference>,
}

#[derive(Clone, Copy)]
enum TermKind {
    Fixture,
    LiveGroup,
    DereferencedGroup,
}

fn fixture_by_number(
    snapshot: &EngineSnapshot,
    token: &str,
) -> Result<Vec<light_core::FixtureId>, String> {
    let number = token
        .parse::<u32>()
        .map_err(|_| "fixture number is invalid")?;
    snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_number == Some(number))
        .map(selectable_fixture_ids)
        .ok_or_else(|| format!("fixture {number} does not exist"))
}

fn group_members(
    snapshot: &EngineSnapshot,
    groups: &SelectionGroups,
    id: &str,
    skip_missing: bool,
) -> Result<Vec<light_core::FixtureId>, String> {
    if skip_missing && !groups.contains_key(id) {
        return Ok(Vec::new());
    }
    let members = light_programmer::resolve_group(id, groups).map_err(|error| {
        if skip_missing && error.contains("does not exist") {
            String::new()
        } else {
            error
        }
    })?;
    let valid = snapshot
        .fixtures
        .iter()
        .flat_map(|fixture| {
            std::iter::once(fixture.fixture_id)
                .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
        })
        .collect::<HashSet<_>>();
    Ok(members
        .into_iter()
        .filter(|fixture| valid.contains(fixture))
        .collect())
}

fn fixture_reference(
    fixture_id: light_core::FixtureId,
    remove: bool,
) -> light_programmer::SelectionReference {
    if remove {
        light_programmer::SelectionReference::RemoveFixture { fixture_id }
    } else {
        light_programmer::SelectionReference::Fixture { fixture_id }
    }
}

fn live_group_reference(group_id: String, remove: bool) -> light_programmer::SelectionReference {
    if remove {
        light_programmer::SelectionReference::RemoveLiveGroup { group_id }
    } else {
        light_programmer::SelectionReference::LiveGroup { group_id }
    }
}

fn push_term(
    sources: &mut Vec<light_programmer::SelectionReference>,
    snapshot: &EngineSnapshot,
    groups: &SelectionGroups,
    kind: TermKind,
    id: &str,
    remove: bool,
    skip_missing: bool,
) -> Result<(), String> {
    match kind {
        TermKind::LiveGroup => {
            if !skip_missing || groups.contains_key(id) {
                group_members(snapshot, groups, id, skip_missing)?;
                sources.push(live_group_reference(id.to_owned(), remove));
            }
        }
        TermKind::DereferencedGroup => {
            sources.extend(
                group_members(snapshot, groups, id, skip_missing)?
                    .into_iter()
                    .map(|fixture| fixture_reference(fixture, remove)),
            );
        }
        TermKind::Fixture => {
            sources.extend(
                fixture_by_number(snapshot, id)?
                    .into_iter()
                    .map(|fixture| fixture_reference(fixture, remove)),
            );
        }
    }
    Ok(())
}

fn push_range(
    sources: &mut Vec<light_programmer::SelectionReference>,
    snapshot: &EngineSnapshot,
    groups: &SelectionGroups,
    kind: TermKind,
    start: &str,
    end: &str,
    remove: bool,
) -> Result<(), String> {
    let start = start.parse::<i32>().map_err(|_| "range start is invalid")?;
    let end = end.parse::<i32>().map_err(|_| "range end is invalid")?;
    let step = if start <= end { 1 } else { -1 };
    let mut current = start;
    loop {
        push_term(
            sources,
            snapshot,
            groups,
            kind,
            &current.to_string(),
            remove,
            true,
        )?;
        if current == end {
            return Ok(());
        }
        current += step;
    }
}

fn group_marker(tokens: &[String], index: &mut usize, default_to_group: bool) -> TermKind {
    let repeated = tokens
        .get(*index + 1)
        .is_some_and(|candidate| candidate == "GROUP");
    if repeated {
        *index += 2;
        TermKind::DereferencedGroup
    } else if *index == 0 && default_to_group {
        *index += 1;
        TermKind::DereferencedGroup
    } else {
        *index += 1;
        TermKind::LiveGroup
    }
}

fn parse_selection_sources(
    snapshot: &EngineSnapshot,
    groups: &SelectionGroups,
    tokens: &[String],
    default_to_group: bool,
) -> Result<Vec<light_programmer::SelectionReference>, String> {
    let mut sources = Vec::new();
    let mut index = 0;
    let mut remove = false;
    let mut kind = if default_to_group {
        TermKind::LiveGroup
    } else {
        TermKind::Fixture
    };
    while index < tokens.len() {
        match tokens[index].as_str() {
            "+" | "-" => {
                remove = tokens[index] == "-";
                kind = TermKind::Fixture;
                index += 1;
            }
            "GROUP" => kind = group_marker(tokens, &mut index, default_to_group),
            "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS" => {
                kind = TermKind::Fixture;
                index += 1;
            }
            token => {
                if tokens
                    .get(index + 1)
                    .is_some_and(|candidate| candidate == "THRU")
                {
                    let end = tokens
                        .get(index + 2)
                        .ok_or("THRU requires an end reference")?;
                    push_range(&mut sources, snapshot, groups, kind, token, end, remove)?;
                    index += 3;
                } else {
                    push_term(&mut sources, snapshot, groups, kind, token, remove, false)?;
                    index += 1;
                }
            }
        }
    }
    Ok(sources)
}

pub(super) fn parse_group_mixed_selection(
    snapshot: &EngineSnapshot,
    tokens: &[String],
    default_to_group: bool,
) -> Result<ParsedMixedSelection, String> {
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<SelectionGroups>();
    let sources = parse_selection_sources(snapshot, &groups, tokens, default_to_group)?;
    let fixtures = light_programmer::resolve_selection_references(&sources, &groups);
    Ok(ParsedMixedSelection { fixtures, sources })
}
