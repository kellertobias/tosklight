//! Focused desk-global clock snapshot and delta push; never mutates source authority.
use super::*;
use light_application::{
    ApplicationEvent, DeliveryPolicy, DeskEvent, EventCapability, EventClass, EventDraft,
    EventObject, EventSource,
};
use light_control::{FrameRate, TimecodeRouter, TimecodeSourceSelection};
use light_wire::v2::runtime::RuntimeTimecodeClockSnapshot as Snapshot;

#[derive(Default)]
pub(super) struct ClockPublication {
    current: Option<Snapshot>,
    emitted: Option<Snapshot>,
    emitted_at: Option<std::time::Instant>,
    seen_lock: bool,
}

fn rate(value: FrameRate) -> String {
    match value {
        FrameRate::Fps2997Drop => "29.97 DF".into(),
        _ => value.nominal_frames().to_string(),
    }
}

impl ClockPublication {
    pub(super) fn observe(&mut self, router: &TimecodeRouter) -> Snapshot {
        let selected = match &router.config().selected_source {
            TimecodeSourceSelection::Internal => "internal".into(),
            TimecodeSourceSelection::External { source } => source.clone(),
        };
        if self
            .current
            .as_ref()
            .is_none_or(|old| old.selected_source != selected)
        {
            self.seen_lock = false;
        }
        let active_source = router.active_source().map(str::to_owned);
        let external = matches!(
            &router.config().selected_source,
            TimecodeSourceSelection::External { .. }
        );
        if external && active_source.is_some() {
            self.seen_lock = true;
        }
        let state = if !external {
            "internal"
        } else if active_source.is_some() {
            "locked"
        } else if self.seen_lock {
            "lost"
        } else {
            "waiting"
        };
        let warning = router.rate_warning();
        let desk_rate = rate(router.config().desk_rate);
        let source_rate = warning
            .map(|w| rate(w.source_rate))
            .or_else(|| router.current().map(|t| rate(t.rate)));
        let mut next = Snapshot {
            revision: self.current.as_ref().map_or(0, |old| old.revision),
            selected_source: selected,
            active_source,
            state: state.into(),
            current: router.current().map(|t| {
                format!(
                    "{:02}:{:02}:{:02}:{:02}",
                    t.hours, t.minutes, t.seconds, t.frames
                )
            }),
            source_rate,
            desk_rate,
            rate_warning: warning.map(|w| {
                format!(
                    "Timecode converted from {} to {} fps",
                    rate(w.source_rate),
                    rate(w.desk_rate)
                )
            }),
        };
        if self.current.as_ref() != Some(&next) {
            next.revision = next.revision.saturating_add(1);
            self.current = Some(next.clone());
        }
        next
    }
    pub(super) fn event(
        &mut self,
        router: &TimecodeRouter,
        now: std::time::Instant,
    ) -> Option<Snapshot> {
        let next = self.observe(router);
        if self.emitted.as_ref() == Some(&next) {
            return None;
        }
        let edge = self.emitted.as_ref().is_none_or(|old| {
            old.state != next.state
                || old.selected_source != next.selected_source
                || old.rate_warning != next.rate_warning
        });
        if !edge
            && self
                .emitted_at
                .is_some_and(|at| now.saturating_duration_since(at) < Duration::from_millis(100))
        {
            return None;
        }
        self.emitted = Some(next.clone());
        self.emitted_at = Some(now);
        Some(next)
    }
}

pub(super) async fn snapshot(
    State(state): State<AppState>,
    desk: DeskContext,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(state.output.clock_status()),
    )
        .into_response())
}

pub(super) fn publish(state: &AppState) {
    let Some(snapshot) = state.output.clock_status_event() else {
        return;
    };
    state.events.publish(EventDraft {
        desk_id: None,
        class: EventClass::Projection,
        object: Some(EventObject::new(EventCapability::Desk, "timecode-clock")),
        related_objects: vec![],
        source: EventSource::Runtime,
        correlation_id: None,
        delivery: DeliveryPolicy::Replaceable,
        payload: ApplicationEvent::Desk(DeskEvent::TimecodeClockChanged {
            revision: snapshot.revision,
            selected_source: snapshot.selected_source,
            active_source: snapshot.active_source,
            state: snapshot.state,
            current: snapshot.current,
            source_rate: snapshot.source_rate,
            desk_rate: snapshot.desk_rate,
            rate_warning: snapshot.rate_warning,
        }),
    });
}
