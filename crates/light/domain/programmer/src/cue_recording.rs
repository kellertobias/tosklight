use crate::{GroupProgrammerValue, ProgrammerRegistry, ProgrammerState};
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId, TimedValue};
use serde::Serialize;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CueRecordingSource {
    CurrentCapture,
    PreloadPendingOrActive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CueRecordingCapturedSource {
    Normal,
    PendingPreload,
    ActivePreload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CueRecordingCaptureError {
    MissingSession,
}

impl Display for CueRecordingCaptureError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSession => formatter.write_str("programmer session does not exist"),
        }
    }
}

impl Error for CueRecordingCaptureError {}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CueRecordingFixtureValue {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
    pub programmer_order: u64,
    pub fade: bool,
    pub fade_millis: Option<u64>,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CueRecordingGroupValue {
    pub group_id: String,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
    pub programmer_order: u64,
    pub fade: bool,
    pub fade_millis: Option<u64>,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CueRecordingCapture {
    pub source: CueRecordingCapturedSource,
    pub fixture_values: Vec<CueRecordingFixtureValue>,
    pub group_values: Vec<CueRecordingGroupValue>,
}

impl CueRecordingCapture {
    pub fn is_empty(&self) -> bool {
        self.fixture_values.is_empty() && self.group_values.is_empty()
    }

    pub fn used_active_preload_fallback(&self) -> bool {
        self.source == CueRecordingCapturedSource::ActivePreload
    }
}

impl ProgrammerRegistry {
    /// Own an exact recording source while holding the user's mutation gate.
    ///
    /// This capture deliberately excludes selection, transient values, Highlight, modes,
    /// priority, connectivity, and every other non-recordable part of Programmer state.
    pub fn capture_cue_recording(
        &self,
        session: SessionId,
        source: CueRecordingSource,
    ) -> Result<CueRecordingCapture, CueRecordingCaptureError> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let key = self.key(session);
        let states = self.states.read();
        let state = states
            .get(&key)
            .ok_or(CueRecordingCaptureError::MissingSession)?;
        Ok(capture(state, source))
    }
}

fn capture(state: &ProgrammerState, requested: CueRecordingSource) -> CueRecordingCapture {
    let source = captured_source(state, requested);
    let (fixture_values, group_values) = source_values(state, source);
    CueRecordingCapture {
        source,
        fixture_values: ordered_fixture_values(fixture_values),
        group_values: ordered_group_values(group_values),
    }
}

fn captured_source(
    state: &ProgrammerState,
    requested: CueRecordingSource,
) -> CueRecordingCapturedSource {
    match requested {
        CueRecordingSource::CurrentCapture if state.blind && state.preload_capture_programmer => {
            CueRecordingCapturedSource::PendingPreload
        }
        CueRecordingSource::CurrentCapture => CueRecordingCapturedSource::Normal,
        CueRecordingSource::PreloadPendingOrActive if has_pending_preload(state) => {
            CueRecordingCapturedSource::PendingPreload
        }
        CueRecordingSource::PreloadPendingOrActive if has_active_preload(state) => {
            CueRecordingCapturedSource::ActivePreload
        }
        CueRecordingSource::PreloadPendingOrActive => CueRecordingCapturedSource::PendingPreload,
    }
}

fn has_pending_preload(state: &ProgrammerState) -> bool {
    !state.preload_pending.is_empty() || has_group_values(&state.preload_group_pending)
}

fn has_active_preload(state: &ProgrammerState) -> bool {
    !state.preload_active.is_empty() || has_group_values(&state.preload_group_active)
}

fn has_group_values(values: &crate::groups::GroupProgrammerValues) -> bool {
    values.values().any(|attributes| !attributes.is_empty())
}

fn source_values(
    state: &ProgrammerState,
    source: CueRecordingCapturedSource,
) -> (&[TimedValue], &crate::groups::GroupProgrammerValues) {
    match source {
        CueRecordingCapturedSource::Normal => (&state.values, &state.group_values),
        CueRecordingCapturedSource::PendingPreload => {
            (&state.preload_pending, &state.preload_group_pending)
        }
        CueRecordingCapturedSource::ActivePreload => {
            (&state.preload_active, &state.preload_group_active)
        }
    }
}

fn ordered_fixture_values(values: &[TimedValue]) -> Vec<CueRecordingFixtureValue> {
    let mut values = values.iter().map(fixture_value).collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.programmer_order
            .cmp(&right.programmer_order)
            .then_with(|| left.fixture_id.0.cmp(&right.fixture_id.0))
            .then_with(|| left.attribute.cmp(&right.attribute))
    });
    values
}

fn ordered_group_values(
    values: &crate::groups::GroupProgrammerValues,
) -> Vec<CueRecordingGroupValue> {
    let mut values = values
        .iter()
        .flat_map(|(group_id, values)| {
            values
                .iter()
                .map(move |(attribute, value)| group_value(group_id, attribute, value))
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.programmer_order
            .cmp(&right.programmer_order)
            .then_with(|| left.group_id.cmp(&right.group_id))
            .then_with(|| left.attribute.cmp(&right.attribute))
    });
    values
}

fn fixture_value(value: &TimedValue) -> CueRecordingFixtureValue {
    CueRecordingFixtureValue {
        fixture_id: value.fixture_id,
        attribute: value.attribute.clone(),
        value: value.value.clone(),
        programmer_order: value.programmer_order,
        fade: value.fade,
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

fn group_value(
    group_id: &str,
    attribute: &AttributeKey,
    value: &GroupProgrammerValue,
) -> CueRecordingGroupValue {
    CueRecordingGroupValue {
        group_id: group_id.to_owned(),
        attribute: attribute.clone(),
        value: value.value.clone(),
        programmer_order: value.programmer_order,
        fade: value.fade,
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}
