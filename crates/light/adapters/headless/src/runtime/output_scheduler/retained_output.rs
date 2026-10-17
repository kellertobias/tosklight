//! Retained-frame payload and delivery for activation or stale-frame scheduler fallback.
use super::{
    Runtime,
    slow_phase_trace::{ApplicationPhase, ApplicationSections, trace_slow_output_phases},
};
use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) async fn send_retained_output(
    runtime: &Runtime,
    application: &mut ApplicationSections,
) -> io::Result<u64> {
    let tick_started = Instant::now();
    let (routes, frames, patched_slots) =
        application.measure(ApplicationPhase::PublicationPayload, || {
            let control = runtime.control.lock();
            (
                Arc::clone(&control.last_routes),
                control.last_frames.clone(),
                control.last_patched_slots.clone(),
            )
        });
    let send_started = Instant::now();
    let result = runtime
        .delivery_policy
        .send(
            &runtime.output,
            &runtime.usb,
            &routes,
            &frames,
            &patched_slots,
            &mut *runtime.sequences.lock().await,
        )
        .await;
    application.record_since(ApplicationPhase::Delivery, send_started);
    trace_slow_output_phases(
        tick_started.elapsed(),
        Duration::ZERO,
        Duration::ZERO,
        Duration::ZERO,
        send_started.elapsed(),
    );
    result
}
