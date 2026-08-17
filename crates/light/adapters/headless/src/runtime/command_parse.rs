pub(super) fn resolve_fixture_reference(
    fixtures: &[light_fixture::PatchedFixture],
    reference: &str,
) -> Result<light_core::FixtureId, String> {
    let (number, head_number) = match reference.split_once('.') {
        Some((fixture, head)) => (
            fixture
                .parse::<u32>()
                .map_err(|_| "fixture number is invalid")?,
            Some(head.parse::<u16>().map_err(|_| "head number is invalid")?),
        ),
        None => (
            reference
                .parse::<u32>()
                .map_err(|_| "fixture number is invalid")?,
            None,
        ),
    };
    if number == 0 {
        return Err("fixture numbers start at 1".into());
    }
    let fixture = fixture_by_number(fixtures, number)
        .ok_or_else(|| format!("fixture {number} does not exist"))?;
    match head_number {
        None => Ok(fixture.fixture_id),
        Some(0) if !fixture.logical_heads.is_empty() => Ok(fixture.fixture_id),
        Some(0) => Err(format!("fixture {number} is not a multi-head fixture")),
        Some(head_number) => ordered_child_ids(fixture)
            .get(usize::from(head_number - 1))
            .copied()
            .ok_or_else(|| format!("fixture {number} has no head {head_number}")),
    }
}

pub(super) fn fixture_by_number(
    fixtures: &[light_fixture::PatchedFixture],
    number: u32,
) -> Option<&light_fixture::PatchedFixture> {
    fixtures
        .iter()
        .find(|fixture| fixture.fixture_number == Some(number))
        .or_else(|| {
            fixtures.get(number.saturating_sub(1) as usize).filter(|_| {
                fixtures
                    .iter()
                    .all(|fixture| fixture.fixture_number.is_none())
            })
        })
}

pub(super) fn ordered_child_ids(
    fixture: &light_fixture::PatchedFixture,
) -> Vec<light_core::FixtureId> {
    fixture
        .definition
        .heads
        .iter()
        .filter(|head| !head.shared)
        .filter_map(|head| {
            fixture
                .logical_heads
                .iter()
                .find(|patched| patched.head_index == head.index)
                .map(|patched| patched.fixture_id)
        })
        .collect()
}

pub(super) fn selectable_fixture_ids(
    fixture: &light_fixture::PatchedFixture,
) -> Vec<light_core::FixtureId> {
    let children = ordered_child_ids(fixture);
    if children.is_empty() {
        vec![fixture.fixture_id]
    } else {
        children
    }
}

pub(super) fn push_unique(
    selected: &mut Vec<light_core::FixtureId>,
    fixture_id: light_core::FixtureId,
) {
    if !selected.contains(&fixture_id) {
        selected.push(fixture_id);
    }
}

pub(super) fn parse_dmx_address(tokens: &[String]) -> Result<(u16, u16), String> {
    let [root, universe, dot, address] = tokens else {
        return Err("DMX requires one universe.address".into());
    };
    if root != "DMX" || dot != "." {
        return Err("DMX requires one universe.address".into());
    }
    if (universe.len() > 1 && universe.starts_with('0'))
        || (address.len() > 1 && address.starts_with('0'))
    {
        return Err("DMX universe.address integers must not contain leading zeros".into());
    }
    let universe = universe
        .parse::<u16>()
        .map_err(|_| "DMX universe must be within 1-65535")?;
    if universe == 0 {
        return Err("DMX universe must be within 1-65535".into());
    }
    let address = address
        .parse::<u16>()
        .map_err(|_| "DMX address must be within 1-512")?;
    if !(1..=512).contains(&address) {
        return Err("DMX address must be within 1-512".into());
    }
    Ok((universe, address))
}

/// Resolves any occupied slot in a root, split, or multipatch footprint to the fixture's normal
/// selectable identities. A multi-head fixture therefore selects all of its logical heads, just
/// like entering its fixture number; every physical multipatch remains one logical fixture.
pub(super) fn resolve_dmx_fixture_selection(
    fixtures: &[light_fixture::PatchedFixture],
    universe: u16,
    address: u16,
) -> Result<Vec<light_core::FixtureId>, String> {
    let owners = fixtures
        .iter()
        .filter(|fixture| fixture.definition.patch_policy() == light_fixture::PatchPolicy::Dmx)
        .filter(|fixture| fixture_owns_dmx_slot(fixture, universe, address))
        .collect::<Vec<_>>();
    match owners.as_slice() {
        [] => Ok(Vec::new()),
        [fixture] => Ok(selectable_fixture_ids(fixture)),
        _ => Err(format!(
            "More than one fixture owns DMX {universe}.{address}; repair the Show Patch overlap"
        )),
    }
}

fn fixture_owns_dmx_slot(
    fixture: &light_fixture::PatchedFixture,
    universe: u16,
    address: u16,
) -> bool {
    let footprints = fixture.definition.split_footprints();
    binding_owns_dmx_slot(
        fixture.universe,
        fixture.address,
        &fixture.split_patches,
        &footprints,
        universe,
        address,
    ) || fixture.multipatch.iter().any(|instance| {
        binding_owns_dmx_slot(
            instance.universe,
            instance.address,
            &instance.split_patches,
            &footprints,
            universe,
            address,
        )
    })
}

fn binding_owns_dmx_slot(
    legacy_universe: Option<u16>,
    legacy_address: Option<u16>,
    splits: &[light_fixture::SplitPatch],
    footprints: &std::collections::BTreeMap<u16, u16>,
    universe: u16,
    address: u16,
) -> bool {
    if splits.is_empty() {
        return legacy_universe == Some(universe)
            && legacy_address.is_some_and(|start| {
                footprint_contains(start, footprints.get(&1).copied().unwrap_or(0), address)
            });
    }
    splits.iter().any(|split| {
        split.universe == Some(universe)
            && split.address.is_some_and(|start| {
                footprint_contains(
                    start,
                    footprints.get(&split.split).copied().unwrap_or(0),
                    address,
                )
            })
    })
}

fn footprint_contains(start: u16, footprint: u16, address: u16) -> bool {
    footprint > 0
        && u32::from(address) >= u32::from(start)
        && u32::from(address) < u32::from(start) + u32::from(footprint)
}

#[derive(Clone, Copy)]
pub(super) struct FixtureReference {
    pub(super) number: u32,
    pub(super) head: Option<u16>,
}

pub(super) fn parse_fixture_reference_tokens(
    tokens: &[String],
    index: &mut usize,
    end: usize,
) -> Result<FixtureReference, String> {
    let number = tokens
        .get(*index)
        .ok_or("expected a fixture number")?
        .parse::<u32>()
        .map_err(|_| "fixture number is invalid")?;
    if number == 0 {
        return Err("fixture numbers start at 1".into());
    }
    *index += 1;
    let head = if *index < end && tokens[*index] == "." {
        *index += 1;
        let head = tokens
            .get(*index)
            .ok_or("fixture head reference requires a head number")?
            .parse::<u16>()
            .map_err(|_| "head number is invalid")?;
        *index += 1;
        Some(head)
    } else {
        None
    };
    Ok(FixtureReference { number, head })
}

pub(super) fn parse_subset_rule(
    tokens: &[String],
) -> Result<light_programmer::SelectionRule, String> {
    if tokens.is_empty() {
        return Ok(light_programmer::SelectionRule::All);
    }
    if tokens[0] == "OFFSET" {
        if tokens.len() != 1 {
            return Err("OFFSET does not accept another offset".into());
        }
        return Ok(light_programmer::SelectionRule::Even);
    }
    if tokens[0] != "DIV" {
        return Err("unexpected tokens after selection".into());
    }
    if tokens.get(1).is_some_and(|token| token == "DIV") {
        if tokens.len() != 2 {
            return Err("DIV DIV does not accept another offset".into());
        }
        return Ok(light_programmer::SelectionRule::Even);
    }
    let n = tokens.get(1).map_or(Ok(2), |token| {
        token
            .parse::<usize>()
            .map_err(|_| "DIV requires a positive number")
    })?;
    if n == 0 {
        return Err("DIV requires a positive number".into());
    }
    let has_offset = tokens.get(2).is_some();
    let offset = match tokens.get(2).map(String::as_str) {
        None => 0,
        Some("+") => tokens
            .get(3)
            .ok_or("+ requires an offset")?
            .parse::<usize>()
            .map_err(|_| "offset is invalid")?,
        _ => return Err("expected + before the subset offset".into()),
    };
    if tokens.len() > if has_offset { 4 } else { 2 } {
        return Err("unexpected tokens after subset".into());
    }
    Ok(light_programmer::SelectionRule::EveryNth { n, offset })
}

pub(super) fn parse_fixture_selection(
    fixtures: &[light_fixture::PatchedFixture],
    tokens: &[String],
) -> Result<Vec<light_core::FixtureId>, String> {
    if tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS"
        )
    }) {
        let normalized = tokens
            .iter()
            .filter(|token| {
                !matches!(
                    token.as_str(),
                    "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS"
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        return parse_fixture_selection(fixtures, &normalized);
    }
    if tokens == ["THRU"] {
        let mut ordered = fixtures
            .iter()
            .filter(|fixture| fixture.definition.patch_policy() == light_fixture::PatchPolicy::Dmx)
            .collect::<Vec<_>>();
        ordered.sort_by_key(|fixture| fixture.fixture_number.unwrap_or(u32::MAX));
        let mut selected = Vec::new();
        for fixture in ordered {
            for selectable in selectable_fixture_ids(fixture) {
                push_unique(&mut selected, selectable);
            }
        }
        return Ok(selected);
    }
    let div = tokens
        .iter()
        .position(|token| token == "DIV" || token == "OFFSET")
        .unwrap_or(tokens.len());
    if let Some(minus) = tokens[..div].iter().position(|token| token == "-") {
        if minus == 0 || minus + 1 == div {
            return Err("- requires fixture selections on both sides".into());
        }
        let mut selected = parse_fixture_selection(fixtures, &tokens[..minus])?;
        let mut start = minus + 1;
        while start < div {
            let end = tokens[start..div]
                .iter()
                .position(|token| token == "-")
                .map_or(div, |offset| start + offset);
            if start == end {
                return Err("- requires a fixture selection".into());
            }
            let removed = parse_fixture_selection(fixtures, &tokens[start..end])?;
            selected.retain(|fixture| !removed.contains(fixture));
            start = end + 1;
        }
        let rule = parse_subset_rule(&tokens[div..])?;
        return Ok(light_programmer::apply_selection_rule(&selected, &rule));
    }
    let mut selected = Vec::new();
    let mut index = 0;
    while index < div {
        if tokens[index] == "+" {
            return Err("expected a fixture reference before +".into());
        }
        let first = parse_fixture_reference_tokens(tokens, &mut index, div)?;
        if tokens.get(index).is_some_and(|token| token == "THRU") {
            index += 1;
            let last = parse_fixture_reference_tokens(tokens, &mut index, div)?;
            if last.number < first.number {
                return Err("fixture range is invalid".into());
            }
            match (first.head, last.head) {
                (None, None) => {
                    for number in first.number..=last.number {
                        let Some(fixture) = fixture_by_number(fixtures, number) else {
                            continue;
                        };
                        let children = ordered_child_ids(fixture);
                        if children.is_empty() {
                            push_unique(&mut selected, fixture.fixture_id);
                        } else {
                            for child in children {
                                push_unique(&mut selected, child);
                            }
                        }
                    }
                }
                (Some(0), Some(0)) => {
                    for number in first.number..=last.number {
                        let fixture_id =
                            resolve_fixture_reference(fixtures, &format!("{number}.0"))?;
                        push_unique(&mut selected, fixture_id);
                    }
                }
                (Some(first_head), Some(last_head))
                    if first.number == last.number && first_head > 0 && last_head >= first_head =>
                {
                    for head in first_head..=last_head {
                        let fixture_id = resolve_fixture_reference(
                            fixtures,
                            &format!("{}.{}", first.number, head),
                        )?;
                        push_unique(&mut selected, fixture_id);
                    }
                }
                _ => {
                    return Err(
                        "head ranges must be .0 across fixtures or child heads within one fixture"
                            .into(),
                    );
                }
            }
        } else {
            match first.head {
                Some(head) => push_unique(
                    &mut selected,
                    resolve_fixture_reference(fixtures, &format!("{}.{}", first.number, head))?,
                ),
                None => {
                    if let Some(fixture) = fixture_by_number(fixtures, first.number) {
                        for selectable in selectable_fixture_ids(fixture) {
                            push_unique(&mut selected, selectable);
                        }
                    }
                }
            }
        }
        if index < div && tokens[index] != "+" {
            return Err("expected + between fixture ranges".into());
        }
        if index < div {
            index += 1;
            if index == div {
                return Err("expected a fixture reference after +".into());
            }
        }
    }
    let rule = parse_subset_rule(&tokens[div..])?;
    Ok(light_programmer::apply_selection_rule(&selected, &rule))
}
