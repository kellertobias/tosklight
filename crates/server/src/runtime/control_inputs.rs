use super::*;

mod mappings;
mod subscriptions;
use mappings::{apply_control_mappings, mapped_control_origin};
pub(super) use subscriptions::{disconnect_orphaned_osc_session, handle_subscription_osc};

pub(super) fn spawn_control_inputs(
    state: &AppState,
    cancel: CancellationToken,
) -> Vec<tokio::task::JoinHandle<()>> {
    let configuration = state.configuration.read().clone();
    let mut tasks = Vec::new();
    if state.manual_clock.is_none() {
        let feedback_state = state.clone();
        let feedback_cancel = cancel.clone();
        tasks.push(tokio::spawn(async move{let mut interval=tokio::time::interval(Duration::from_millis(500));loop{tokio::select!{_=feedback_cancel.cancelled()=>break,_=interval.tick()=>send_osc_feedback(&feedback_state,false)}}}));
        let refresh_state = state.clone();
        let refresh_cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            loop {
                tokio::select! {
                    _ = refresh_cancel.cancelled() => break,
                    _ = interval.tick() => { refresh_speed_group_engine(&refresh_state); }
                }
            }
        }));
    }
    for (address, protocol) in [
        (configuration.osc_bind, UdpInputProtocol::Osc),
        (
            configuration.art_timecode_bind,
            UdpInputProtocol::ArtTimeCode,
        ),
    ] {
        let Some(address) = address else {
            continue;
        };
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            match UdpControlInput::bind(address, protocol).await {
                Ok(input) => drive_control_input(state, cancel, input).await,
                Err(error) => tracing::error!(%address,%error,"control input could not bind"),
            }
        }));
    }
    for port in configuration.midi_inputs {
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            match MidiControlInput::open(&port) {
                Ok(input) => drive_control_input(state, cancel, input).await,
                Err(error) => tracing::error!(%port,%error,"MIDI input could not open"),
            }
        }));
    }
    if let Some(address) = configuration.rtp_midi_bind {
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            match RtpMidiInput::bind(address, "Light").await {
                Ok(input) => drive_control_input(state, cancel, input).await,
                Err(error) => tracing::error!(%address,%error,"RTP-MIDI input could not bind"),
            }
        }));
    }
    tasks
}

pub(super) async fn drive_control_input<I: ControlInput>(
    state: AppState,
    cancel: CancellationToken,
    mut input: I,
) {
    loop {
        tokio::select! { _=cancel.cancelled()=>break,event=input.next_event()=>match event { Some(event)=>handle_control_event(&state,event),None=>break } }
    }
}

pub(super) fn handle_control_event(state: &AppState, event: ControlEvent) {
    if let ControlEvent::Timecode(timecode) = &event {
        ingest_timecode(state, timecode.clone());
    }
    let input_locked = if let ControlEvent::Osc { address, .. } = &event {
        let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
        parts
            .get(1)
            .and_then(|alias| osc_control_desk(state, alias))
            .is_some_and(|desk| read_desk_lock(state, desk.id).locked)
    } else {
        false
    };
    if let ControlEvent::Osc {
        address, arguments, ..
    } = &event
        && let Some(configuration) = &state.configuration.read().osc_timecode
        && &configuration.address == address
        && let [
            OscArgument::Int(hours),
            OscArgument::Int(minutes),
            OscArgument::Int(seconds),
            OscArgument::Int(frames),
            ..,
        ] = arguments.as_slice()
        && (0..24).contains(hours)
        && (0..60).contains(minutes)
        && (0..60).contains(seconds)
        && (0..i32::from(configuration.rate.nominal_frames())).contains(frames)
    {
        ingest_timecode(
            state,
            SmpteTimecode {
                hours: *hours as u8,
                minutes: *minutes as u8,
                seconds: *seconds as u8,
                frames: *frames as u8,
                rate: configuration.rate,
                source: format!("osc:{address}"),
                received_at: chrono::Utc::now(),
            },
        );
    }
    if let ControlEvent::Osc {
        address,
        arguments,
        source,
    } = &event
    {
        if !handle_subscription_osc(state, address, arguments, source.as_deref()) && !input_locked {
            handle_playback_osc(state, address, arguments, source.as_deref());
            handle_highlight_osc(state, address, arguments, source.as_deref());
            handle_programmer_osc(state, address, arguments, source.as_deref());
            handle_timing_osc(state, address, arguments);
            handle_encoder_osc(state, address, arguments);
        }
        send_osc_feedback(state, false);
    }
    if input_locked {
        return;
    }
    let mappings = state.engine.snapshot().control_mappings.clone();
    let origin = mapped_control_origin(state, &event);
    if mappings.iter().any(|mapping| mapping.matches(&event)) {
        match state.activation_lock.clone().try_lock_owned() {
            Ok(_activation) => {
                apply_control_mappings(
                    state,
                    &origin,
                    mappings
                        .iter()
                        .filter(|mapping| mapping.matches(&event))
                        .map(|mapping| &mapping.action),
                );
            }
            Err(_) => tracing::warn!("mapped control action skipped during active show transition"),
        }
    }
    emit(
        state,
        "control_event",
        serde_json::to_value(event)
            .unwrap_or_else(|_| serde_json::json!({"error":"serialization failed"})),
    );
}
