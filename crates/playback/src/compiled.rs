use crate::*;

/// Immutable lookup data built when a Cuelist enters a playback generation.
///
/// Attribute histories retain only authored changes rather than cloning the complete tracked
/// state for every Cue. Rendering can therefore resolve any current/previous pair without
/// rebuilding a `HashMap`, while memory grows with authored changes instead of
/// `cue_count * tracked_attribute_count`.
#[derive(Clone, Debug, Default)]
pub(crate) struct CompiledCueList {
    attributes: Vec<CompiledAttribute>,
    by_address: HashMap<AttributeAddress, usize>,
    by_fixture: HashMap<FixtureId, Vec<usize>>,
    fixtures: Vec<(FixtureId, usize)>,
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledAttribute {
    fixture_id: FixtureId,
    attribute: AttributeKey,
    first_cue_index: usize,
    history: Vec<CompiledChange>,
}

#[derive(Clone, Debug)]
struct CompiledChange {
    cue_index: usize,
    value: Option<AttributeValue>,
    fade_millis: Option<u64>,
    delay_millis: Option<u64>,
}

impl CompiledCueList {
    pub(crate) fn new(cue_list: &CueList) -> Self {
        let mut compiled = Self::default();
        for (cue_index, cue) in cue_list.cues.iter().enumerate() {
            for change in &cue.changes {
                compiled.push_change(cue_index, change);
            }
        }
        compiled
    }

    pub(crate) fn attributes(&self) -> &[CompiledAttribute] {
        &self.attributes
    }

    pub(crate) fn attributes_through(&self, cue_index: usize) -> &[CompiledAttribute] {
        let end = self
            .attributes
            .partition_point(|attribute| attribute.first_cue_index <= cue_index);
        &self.attributes[..end]
    }

    pub(crate) fn attributes_for_fixture(
        &self,
        fixture_id: FixtureId,
    ) -> impl Iterator<Item = &CompiledAttribute> {
        self.by_fixture
            .get(&fixture_id)
            .into_iter()
            .flatten()
            .map(|index| &self.attributes[*index])
    }

    pub(crate) fn fixture_ids_through(
        &self,
        cue_index: usize,
    ) -> impl Iterator<Item = FixtureId> + '_ {
        self.fixtures
            .iter()
            .take_while(move |(_, first_cue_index)| *first_cue_index <= cue_index)
            .map(|(fixture_id, _)| *fixture_id)
    }

    pub(crate) fn contains(&self, fixture_id: FixtureId, attribute: &AttributeKey) -> bool {
        self.by_address
            .contains_key(&(fixture_id, attribute.clone()))
    }

    pub(crate) fn value(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
        cue_index: usize,
        tracking_wrap: bool,
    ) -> Option<&AttributeValue> {
        let index = self.by_address.get(&(fixture_id, attribute.clone()))?;
        self.attributes[*index].value(cue_index, tracking_wrap)
    }

    fn push_change(&mut self, cue_index: usize, change: &CueChange) {
        let address = change.address();
        let attribute_index = if let Some(index) = self.by_address.get(&address) {
            *index
        } else {
            let index = self.attributes.len();
            self.attributes.push(CompiledAttribute {
                fixture_id: change.fixture_id,
                attribute: change.attribute.clone(),
                first_cue_index: cue_index,
                history: Vec::new(),
            });
            self.by_address.insert(address, index);
            if !self.by_fixture.contains_key(&change.fixture_id) {
                self.fixtures.push((change.fixture_id, cue_index));
            }
            self.by_fixture
                .entry(change.fixture_id)
                .or_default()
                .push(index);
            index
        };
        self.attributes[attribute_index]
            .history
            .push(CompiledChange {
                cue_index,
                value: change.value.clone(),
                fade_millis: change.fade_millis,
                delay_millis: change.delay_millis,
            });
    }
}

impl CompiledAttribute {
    pub(crate) fn fixture_id(&self) -> FixtureId {
        self.fixture_id
    }

    pub(crate) fn attribute(&self) -> &AttributeKey {
        &self.attribute
    }

    pub(crate) fn value(&self, cue_index: usize, tracking_wrap: bool) -> Option<&AttributeValue> {
        self.effective_change(cue_index, tracking_wrap)
            .and_then(|change| change.value.as_ref())
    }

    pub(crate) fn timing(&self, cue_index: usize) -> Option<(Option<u64>, Option<u64>)> {
        let change = self.change_at_or_before(cue_index)?;
        (change.cue_index == cue_index).then_some((change.fade_millis, change.delay_millis))
    }

    fn effective_change(&self, cue_index: usize, tracking_wrap: bool) -> Option<&CompiledChange> {
        let tracked = self.change_at_or_before(cue_index);
        if tracking_wrap {
            tracked.or_else(|| self.history.last())
        } else {
            tracked
        }
    }

    fn change_at_or_before(&self, cue_index: usize) -> Option<&CompiledChange> {
        let position = self
            .history
            .partition_point(|change| change.cue_index <= cue_index);
        position
            .checked_sub(1)
            .and_then(|index| self.history.get(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_and_wrap_lookups_preserve_releases() {
        let fixture = FixtureId::new();
        let attribute = AttributeKey("pan".into());
        let untouched = AttributeKey("tilt".into());
        let mut first = Cue::new(1.0);
        first.changes.push(CueChange::set(
            fixture,
            attribute.clone(),
            AttributeValue::Normalized(0.2),
        ));
        let mut second = Cue::new(2.0);
        second.changes.push(CueChange {
            fixture_id: fixture,
            attribute: attribute.clone(),
            value: None,
            automatic_restore: false,
            fade_millis: Some(50),
            delay_millis: Some(25),
        });
        second.changes.push(CueChange::set(
            fixture,
            untouched.clone(),
            AttributeValue::Normalized(0.7),
        ));
        let list = CueList {
            id: CueListId::new(),
            name: String::new(),
            priority: 0,
            mode: CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: IntensityPriorityMode::Htp,
            wrap_mode: Some(WrapMode::Tracking),
            restart_mode: RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![first, second],
        };
        let compiled = CompiledCueList::new(&list);

        assert_eq!(
            compiled.value(fixture, &attribute, 0, false),
            Some(&AttributeValue::Normalized(0.2))
        );
        assert_eq!(compiled.value(fixture, &attribute, 1, false), None);
        assert_eq!(
            compiled.value(fixture, &untouched, 0, true),
            Some(&AttributeValue::Normalized(0.7))
        );
        let pan = compiled
            .attributes()
            .iter()
            .find(|candidate| candidate.attribute() == &attribute)
            .unwrap();
        assert_eq!(pan.timing(1), Some((Some(50), Some(25))));
        assert_eq!(pan.timing(0), Some((None, None)));
    }

    #[test]
    fn cue_bound_indexes_exclude_future_only_addresses_and_fixtures() {
        let first_fixture = FixtureId::new();
        let future_fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.changes.push(CueChange::set(
            first_fixture,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.2),
        ));
        let mut second = Cue::new(2.0);
        second.changes.push(CueChange::set(
            future_fixture,
            AttributeKey("tilt".into()),
            AttributeValue::Normalized(0.7),
        ));
        let list = CueList {
            id: CueListId::new(),
            name: String::new(),
            priority: 0,
            mode: CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: IntensityPriorityMode::Htp,
            wrap_mode: Some(WrapMode::Reset),
            restart_mode: RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![first, second],
        };
        let compiled = CompiledCueList::new(&list);

        assert_eq!(compiled.attributes_through(0).len(), 1);
        assert_eq!(compiled.attributes_through(1).len(), 2);
        assert_eq!(
            compiled.fixture_ids_through(0).collect::<Vec<_>>(),
            vec![first_fixture]
        );
        assert_eq!(
            compiled.fixture_ids_through(1).collect::<Vec<_>>(),
            vec![first_fixture, future_fixture]
        );
    }
}
