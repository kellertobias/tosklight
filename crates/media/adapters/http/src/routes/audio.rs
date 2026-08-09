//! Audio: the settings, and one snapshot of what is being heard.
//!
//! The analysis itself is pushed over the telemetry socket, because it changes many times a second
//! and polling it would be the wrong shape. What this route offers is one snapshot — enough for a
//! panel to render before its socket is up, and enough for a diagnostic to answer "is anything
//! arriving?" with a single request.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{AudioPanelView, AudioSettingsView, AudioView, UpdateAudio};

/// The audio settings, this machine's inputs, and the analysis as of now.
pub(super) async fn audio(State(state): State<ApiState>) -> impl IntoResponse {
    let configuration = state.configuration.load();
    axum::Json(AudioPanelView {
        settings: AudioSettingsView::of(&configuration.audio, (state.diagnostics.audio_devices)()),
        analysis: AudioView::of(&(state.diagnostics.audio)()),
    })
}

/// Edits the audio settings.
///
/// Stored like every other edit, and then handed to whatever is running: the gains and the beat
/// sensitivity reach the live analysis, so an operator turning them hears the result. A different
/// device means a different stream, which happens on the next start.
pub(super) async fn update_audio(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<UpdateAudio>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }

    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    configuration.audio = body
        .applied(&configuration.audio)
        .map_err(|error| ApiError::bad_request("audio-invalid", error.to_string()))?;

    let view = AudioSettingsView::of(&configuration.audio, (state.diagnostics.audio_devices)());
    edit::commit(&state, configuration, &body.request_id, &view)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use axum::http::StatusCode;

    use crate::diagnostics::{AudioTelemetry, Diagnostics};
    use crate::routes::bench::{bench, bench_with, get, post, send};

    fn hearing_something() -> Diagnostics {
        Diagnostics {
            audio: Arc::new(|| AudioTelemetry {
                capturing: true,
                device: "Desk feed".to_owned(),
                detail: None,
                waveform: vec![0.0, 0.5, -0.5],
                spectrum: vec![0.2, 0.4],
                bass: 0.8,
                mid: 0.4,
                treble: 0.1,
                energy: 0.5,
                peak: 0.9,
                beat: 1.0,
                bpm: 128.0,
                beat_phase: 0.25,
            }),
            audio_devices: Arc::new(|| vec!["Built-in".to_owned(), "Desk feed".to_owned()]),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn the_snapshot_reports_the_settings_the_devices_and_what_is_arriving() {
        let bench = bench_with(hearing_something());
        let (status, body) = send(&bench.router, get("/api/v2/audio".into())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["settings"]["deviceBy"], "system-default");
        assert_eq!(body["settings"]["inputGain"], 1.0);
        assert_eq!(
            body["settings"]["availableDevices"]
                .as_array()
                .expect("a list")
                .len(),
            2
        );
        assert_eq!(body["analysis"]["capturing"], true);
        assert_eq!(body["analysis"]["device"], "Desk feed");
        assert_eq!(body["analysis"]["bands"]["bass"], 0.8);
        assert_eq!(body["analysis"]["bpm"], 128.0);
        assert_eq!(body["analysis"]["waveform"]["points"][1], 0.5);
    }

    #[tokio::test]
    async fn a_machine_with_no_input_says_so_rather_than_showing_a_dead_meter() {
        let bench = bench();
        let (_, body) = send(&bench.router, get("/api/v2/audio".into())).await;

        assert_eq!(body["analysis"]["capturing"], false);
        assert!(body["analysis"]["detail"].is_string());
        assert!(
            body["settings"]["availableDevices"]
                .as_array()
                .expect("a list")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_tuning_edit_is_stored_and_then_handed_to_what_is_running() {
        let bench = bench_with(hearing_something());
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/audio/update".into(),
                r#"{"requestId":"a","inputGain":2.5,"eqBass":1.5}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["inputGain"], 2.5);
        assert_eq!(body["eqBass"], 1.5);

        let stored = bench.stored.lock().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].audio.input_gain, 2.5);
        assert_eq!(
            bench.applied.load(Ordering::SeqCst),
            1,
            "the running analysis was told, so an operator hears the change"
        );
    }

    #[tokio::test]
    async fn choosing_a_named_device_is_stored_and_reported_as_needing_a_restart() {
        let bench = bench_with(hearing_something());
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/audio/update".into(),
                r#"{"requestId":"a","deviceBy":"name","deviceValue":"Desk feed"}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["deviceBy"], "name");
        assert_eq!(body["deviceValue"], "Desk feed");
        assert_eq!(body["deviceTakesEffectOnRestart"], true);
    }

    #[tokio::test]
    async fn an_unusable_tuning_is_refused_and_nothing_is_written() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/audio/update".into(),
                r#"{"requestId":"a","inputGain":50}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "audio-invalid");
        assert!(body["message"].as_str().unwrap().contains("inputGain"));
        assert!(bench.stored.lock().unwrap().is_empty());
        assert_eq!(bench.applied.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn naming_a_device_without_naming_it_is_refused() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/audio/update".into(),
                r#"{"requestId":"a","deviceBy":"name"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "audio-invalid");
        assert!(body["message"].as_str().unwrap().contains("deviceValue"));
    }

    #[tokio::test]
    async fn a_retried_tuning_edit_is_answered_rather_than_repeated() {
        let bench = bench();
        let body = r#"{"requestId":"same","eqTreble":0.5}"#;

        let (_, first) = send(&bench.router, post("/api/v2/audio/update".into(), body)).await;
        let (status, second) = send(&bench.router, post("/api/v2/audio/update".into(), body)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(first, second);
        assert_eq!(bench.stored.lock().unwrap().len(), 1);
        assert_eq!(bench.applied.load(Ordering::SeqCst), 1);
    }
}
