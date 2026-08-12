//! Outputs, their layers, and the live control of both.
//!
//! Selecting media and setting a dimmer are live control, not edits: they carry no request
//! identity, because a caller that sent a selection twice meant it twice. What protects them is
//! ownership — while a desk is driving an output, the web interface reads but does not write.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;
use media_application::configuration::{load, save};
use media_domain::{
    Applied, Command, CommandKind, CommandSource, MediaState, OutputId, Timestamp, apply,
};

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{
    DmxMapView, OutputConfigurationView, OutputView, UpdateLayer, UpdateOutputConfiguration,
};

pub(super) async fn outputs(State(state): State<ApiState>) -> impl IntoResponse {
    let media = state.state.load();
    let views: Vec<OutputView> = media
        .outputs
        .iter()
        .map(|output| view_of(&state, output, (state.now)()))
        .collect();
    axum::Json(views)
}

pub(super) async fn output_state(
    State(state): State<ApiState>,
    Path(output): Path<String>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(view_of(&state, found, (state.now)())).into_response())
}

/// The stored settings that define one output.
///
/// This is separate from `/state`: state is what the output is drawing now, while configuration
/// is what surface, clock, personality, and DMX ingress the next process start will create.
pub(super) async fn output_configuration(
    State(state): State<ApiState>,
    Path(output): Path<String>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let configuration = state.configuration.load();
    let found = configuration.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(OutputConfigurationView::of(found)).into_response())
}

/// The canonical, absolute DMX map for one configured output.
pub(super) async fn dmx_map(
    State(state): State<ApiState>,
    Path(output): Path<String>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let configuration = state.configuration.load();
    let found = configuration.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(DmxMapView::of(found)).into_response())
}

/// Changes only the stated output settings, after validating the whole server configuration.
///
/// This edit uses the same written-then-live-then-answered path as every stored setting. The view
/// truthfully reports that all accepted fields take effect on restart; publishing the stored
/// configuration does not recreate an already-running output surface or ingress.
pub(super) async fn update_output_configuration(
    State(state): State<ApiState>,
    Path(output): Path<String>,
    TolerantJson(body): TolerantJson<UpdateOutputConfiguration>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }

    let id = parse_output(&output)?;
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let found = configuration
        .outputs
        .iter_mut()
        .find(|candidate| candidate.id == id)
        .ok_or_else(|| unknown_output(id))?;
    *found = body.applied(found).map_err(|error| {
        ApiError::bad_request("output-configuration-invalid", error.to_string())
    })?;
    let view = OutputConfigurationView::of(found);

    // Loading what we would write exercises the one authoritative full-document validator. In
    // particular, this catches resolution and presentation errors, personality footprint bounds,
    // and overlaps with every other enabled output before persistence is attempted.
    load(&save(&configuration)).map_err(|error| {
        ApiError::bad_request("output-configuration-invalid", error.to_string())
    })?;

    edit::commit(&state, configuration, &body.request_id, &view)
}

pub(super) async fn update_layer(
    State(state): State<ApiState>,
    Path((output, layer)): Path<(String, usize)>,
    TolerantJson(body): TolerantJson<UpdateLayer>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let now = (state.now)();

    // Both halves of the update are separate commands, so a dimmer change never rewrites a
    // selection and a selection change never rewrites a dimmer.
    let mut commands = Vec::new();
    let current = {
        let media = state.state.load();
        let found = media.output(id).ok_or_else(|| unknown_output(id))?;
        found
            .layer(layer)
            .ok_or_else(|| {
                ApiError::not_found("unknown-layer", format!("this output has no layer {layer}"))
            })?
            .address
    };

    if body.changes_address() {
        commands.push(CommandKind::SelectMedia {
            output: id,
            layer,
            address: body.address(current),
        });
    }
    if let Some(dimmer) = body.dimmer {
        if !dimmer.is_finite() || !(0.0..=1.0).contains(&dimmer) {
            return Err(ApiError::bad_request(
                "dimmer-out-of-range",
                "dimmer must be between 0 and 1",
            ));
        }
        commands.push(CommandKind::SetLayerDimmer {
            output: id,
            layer,
            dimmer,
        });
    }

    submit(&state, commands, now)?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(view_of(&state, found, now)).into_response())
}

/// Restarts a layer's media. A live-control action with no payload, so it is a `GET` an
/// integrator can trigger from a URL bar or a microcontroller.
pub(super) async fn reset_layer(
    State(state): State<ApiState>,
    Path((output, layer)): Path<(String, usize)>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let now = (state.now)();
    submit(
        &state,
        vec![CommandKind::ResetLayer { output: id, layer }],
        now,
    )?;

    // A side-effecting GET must never be cached, or a proxy would swallow the second press.
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

/// Explicitly changes playback ownership. Taking over makes subsequent network DMX read-only;
/// releasing immediately lets the next Art-Net or sACN frame drive the output again.
pub(super) async fn set_playback_takeover(
    State(state): State<ApiState>,
    Path((output, mode)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let take_over = match mode.as_str() {
        "take-over" => true,
        "release" => false,
        _ => {
            return Err(ApiError::not_found(
                "unknown-playback-control",
                "use take-over or release",
            ));
        }
    };
    let now = (state.now)();
    submit(
        &state,
        vec![CommandKind::TakeOverPlayback {
            output: id,
            take_over,
        }],
        now,
    )?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(view_of(&state, found, now)).into_response())
}

/// Applies commands through the reducer and publishes one new snapshot.
fn submit(state: &ApiState, commands: Vec<CommandKind>, now: Timestamp) -> Result<(), ApiError> {
    if commands.is_empty() {
        return Ok(());
    }
    let mut next = MediaState::clone(&state.state.load());
    let mut published = false;

    for kind in commands {
        let command = Command::new(kind, CommandSource::Web, now);
        match apply(&mut next, &command) {
            Applied::Changed => published = true,
            Applied::Unchanged => {}
            Applied::RejectedNotOwner => {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "dmx-owns-this",
                    "a lighting desk is currently driving this output; its values cannot be set \
                     from here until it stops sending",
                ));
            }
            Applied::RejectedUnknownOutput => {
                return Err(ApiError::not_found("unknown-output", "no such output"));
            }
            Applied::RejectedUnknownLayer => {
                return Err(ApiError::not_found("unknown-layer", "no such layer"));
            }
        }
    }

    if published {
        state.state.store(Arc::new(next));
    }
    Ok(())
}

fn view_of(state: &ApiState, output: &media_domain::OutputState, now: Timestamp) -> OutputView {
    let configuration = state.configuration.load();
    let name = configuration
        .output(output.id)
        .map(|configured| configured.name.to_string())
        .unwrap_or_else(|| output.id.to_string());

    OutputView::of(output, name, output.ownership.dmx_is_active(now))
}

fn parse_output(raw: &str) -> Result<OutputId, ApiError> {
    uuid::Uuid::parse_str(raw)
        .map(OutputId::from_uuid)
        .map_err(|_| ApiError::bad_request("malformed-output-id", "that is not an output id"))
}

fn unknown_output(id: OutputId) -> ApiError {
    ApiError::not_found("unknown-output", format!("no output {id}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::http::{StatusCode, header};
    use media_application::configuration::{MediaConfiguration, OutputConfiguration};
    use media_domain::{
        CommandSource, LayerPersonality, MediaAddress, MediaState, OutputId, Timestamp,
    };
    use tower::ServiceExt as _;

    use crate::routes::bench::{bench, get, post, send};

    #[tokio::test]
    async fn an_output_returns_its_whole_state() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", bench.output)),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "Main");
        assert_eq!(body["layers"].as_array().unwrap().len(), 2);
        assert_eq!(body["layers"][0]["playMode"], "Loop");
        assert_eq!(body["dmxActive"], false);
    }

    #[tokio::test]
    async fn an_output_returns_its_whole_editable_configuration() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/configuration", bench.output)),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], bench.output.to_string());
        assert_eq!(body["name"], "Main");
        assert_eq!(body["targetKind"], "off-screen");
        assert!(body["monitorBy"].is_null());
        assert!(body["monitorValue"].is_null());
        assert_eq!(body["fullscreen"], false);
        assert_eq!(body["width"], 1920);
        assert_eq!(body["height"], 1080);
        assert_eq!(body["presentation"], "display-synchronized");
        assert_eq!(body["personality"], "two-layers");
        assert_eq!(body["protocol"], "art-net");
        assert_eq!(body["universe"], 0);
        assert_eq!(body["startAddress"], 1);
        assert_eq!(body["takesEffectOnRestart"], true);
        assert!(
            body.get("statusOverlay").is_none(),
            "the retired setting stays dropped"
        );
        assert!(
            body.get("targetCodec").is_none(),
            "library policy is not an output setting"
        );
    }

    #[tokio::test]
    async fn an_output_returns_its_canonical_absolute_dmx_map() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/dmx-map", bench.output)),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["outputId"], bench.output.to_string());
        assert_eq!(body["outputName"], "Main");
        assert_eq!(body["personality"], "twoLayers");
        assert_eq!(body["layerCount"], 2);
        assert_eq!(body["channels"].as_array().unwrap().len(), 75);
        assert_eq!(body["channels"][0]["absoluteChannel"], 1);
        assert_eq!(body["channels"][0]["name"], "Folder");
        assert_eq!(body["channels"][68]["group"]["kind"], "master");

        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/dmx-map", OutputId::new())),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-output");
    }

    #[tokio::test]
    async fn an_output_configuration_edit_is_intent_shaped_written_and_restart_explicit() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/configuration/update", bench.output);
        let (status, body) = send(
            &bench.router,
            post(
                uri,
                r#"{"requestId":"settings-1","targetKind":"monitor","monitorBy":"name","monitorValue":" Stage Right ","fullscreen":true,"width":1280,"height":720,"presentation":"fixed-fps","framesPerSecond":50,"personality":"eight-layers","protocol":"sacn","universe":7,"startAddress":10}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["targetKind"], "monitor");
        assert_eq!(body["monitorBy"], "name");
        assert_eq!(body["monitorValue"], "Stage Right");
        assert_eq!(body["fullscreen"], true);
        assert_eq!(body["width"], 1280);
        assert_eq!(body["height"], 720);
        assert_eq!(body["presentation"], "fixed-fps");
        assert_eq!(body["framesPerSecond"], 50);
        assert_eq!(body["personality"], "eight-layers");
        assert_eq!(body["protocol"], "sacn");
        assert_eq!(body["universe"], 7);
        assert_eq!(body["startAddress"], 10);
        assert_eq!(body["takesEffectOnRestart"], true);

        let stored = bench.stored.lock().unwrap();
        assert_eq!(
            stored.len(),
            1,
            "an accepted edit was persisted exactly once"
        );
        assert_eq!(stored[0].outputs[0].resolution.width, 1280);
        assert_eq!(bench.applied.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_output_configuration_edit_changes_only_what_it_carries() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/configuration/update", bench.output);
        let (status, body) = send(
            &bench.router,
            post(
                uri,
                r#"{"requestId":"settings-2","universe":4,"newerClientField":{"keptByNewerServer":true}}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK, "unknown fields remain tolerated");
        assert_eq!(body["universe"], 4);
        assert_eq!(body["width"], 1920);
        assert_eq!(body["presentation"], "display-synchronized");
        assert_eq!(body["personality"], "two-layers");
    }

    #[tokio::test]
    async fn an_invalid_output_edit_is_neither_written_nor_published() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/configuration/update", bench.output);
        let (status, body) = send(
            &bench.router,
            post(uri, r#"{"requestId":"settings-3","height":0}"#),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "output-configuration-invalid");
        assert!(body["message"].as_str().unwrap().contains("height"));
        assert!(bench.stored.lock().unwrap().is_empty());
        assert_eq!(
            bench.configuration.load().outputs[0].resolution.height,
            1080
        );
        assert_eq!(bench.applied.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_output_edit_validates_against_every_other_outputs_patch() {
        let bench = bench();
        let mut configuration = MediaConfiguration::clone(&bench.configuration.load());
        let mut second = OutputConfiguration::new("Second");
        second.personality = LayerPersonality::TwoLayers;
        second.start_address = 200;
        let second_id = second.id;
        configuration.outputs.push(second);
        bench.configuration.store(Arc::new(configuration));

        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{second_id}/configuration/update"),
                r#"{"requestId":"overlap","startAddress":50}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "output-configuration-invalid");
        assert!(body["message"].as_str().unwrap().contains("both consume"));
        assert!(bench.stored.lock().unwrap().is_empty());
        assert_eq!(bench.configuration.load().outputs[1].start_address, 200);
    }

    #[tokio::test]
    async fn a_retried_output_edit_is_answered_without_a_second_write() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/configuration/update", bench.output);
        let request = r#"{"requestId":"same-settings","startAddress":20}"#;

        let (_, first) = send(&bench.router, post(uri.clone(), request)).await;
        let (status, second) = send(&bench.router, post(uri, request)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(first, second);
        assert_eq!(bench.stored.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn an_output_edit_the_disk_refuses_does_not_become_live() {
        let bench = bench();
        bench
            .refuse
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/configuration/update", bench.output),
                r#"{"requestId":"settings-4","universe":12}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "configuration-not-written");
        assert_eq!(bench.configuration.load().outputs[0].universe, 0);
        assert_eq!(bench.applied.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_update_carries_only_what_it_changes() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);

        let (status, body) =
            send(&bench.router, post(uri.clone(), r#"{"folder":3,"file":7}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["layers"][0]["address"]["folder"], 3);
        assert_eq!(body["layers"][0]["address"]["file"], 7);
        assert_eq!(body["layers"][0]["dimmer"], 1.0);

        // A dimmer change must not disturb the selection.
        let (_, body) = send(&bench.router, post(uri, r#"{"dimmer":0.25}"#)).await;
        assert_eq!(body["layers"][0]["dimmer"], 0.25);
        assert_eq!(
            body["layers"][0]["address"]["folder"], 3,
            "the selection survived"
        );
        assert_eq!(body["layers"][0]["address"]["file"], 7);
    }

    #[tokio::test]
    async fn unknown_fields_are_accepted_rather_than_rejected() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":0.5,"somethingNewer":true,"nested":{"deep":1}}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "a newer client must not be refused");
        assert_eq!(body["layers"][0]["dimmer"], 0.5);
    }

    #[tokio::test]
    async fn a_malformed_body_names_the_problem_rather_than_crashing() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":"loud"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "body-invalid");
        assert!(body["message"].as_str().unwrap().contains("dimmer"));
    }

    #[tokio::test]
    async fn a_dimmer_outside_its_range_is_refused_with_a_stable_code() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":4.0}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "dimmer-out-of-range");
    }

    #[tokio::test]
    async fn a_reset_is_a_payload_free_get_that_must_not_be_cached() {
        let bench = bench();
        let response = bench
            .router
            .clone()
            .oneshot(get(format!(
                "/api/v2/outputs/{}/layers/0/reset",
                bench.output
            )))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            bench.state.load().output(bench.output).unwrap().layers[0].reset_trigger_id,
            1
        );
    }

    #[tokio::test]
    async fn a_live_desk_makes_the_web_ui_read_only_with_a_reason() {
        let bench = bench();
        // A desk starts sending.
        let mut next = MediaState::clone(&bench.state.load());
        next.outputs[0]
            .ownership
            .observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(0));
        bench.state.store(Arc::new(next));

        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":0.5}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "dmx-owns-this");

        // Reading still works, and reports why.
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", bench.output)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dmxActive"], true);
    }

    #[tokio::test]
    async fn a_reset_still_works_while_a_desk_is_driving() {
        let bench = bench();
        let mut next = MediaState::clone(&bench.state.load());
        next.outputs[0]
            .ownership
            .observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(0));
        bench.state.store(Arc::new(next));

        let response = bench
            .router
            .clone()
            .oneshot(get(format!(
                "/api/v2/outputs/{}/layers/0/reset",
                bench.output
            )))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NO_CONTENT,
            "an administrative action stays available"
        );
    }

    #[tokio::test]
    async fn unknown_outputs_and_layers_are_reported_distinctly() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", OutputId::new())),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-output");

        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/9/update", bench.output),
                r#"{"dimmer":0.5}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-layer");
    }

    #[tokio::test]
    async fn a_malformed_output_id_is_a_bad_request_not_a_not_found() {
        let bench = bench();
        let (status, body) =
            send(&bench.router, get("/api/v2/outputs/nonsense/state".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "malformed-output-id");
    }

    #[tokio::test]
    async fn a_layer_reports_its_mask_even_when_the_mask_is_doing_nothing() {
        let bench = bench();
        let (_, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", bench.output)),
        )
        .await;

        let mask = &body["layers"][0]["mask"];
        assert_eq!(mask["address"]["class"], "blank");
        assert_eq!(mask["opacity"], 0.0);
        assert_eq!(mask["source"], "luminance");
        assert_eq!(
            mask["active"], false,
            "selected-but-faded and not-selected must be tellable apart"
        );
    }

    #[tokio::test]
    async fn selecting_a_blank_address_is_allowed_because_it_clears_a_layer() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        send(&bench.router, post(uri.clone(), r#"{"folder":1,"file":1}"#)).await;

        let (status, body) = send(&bench.router, post(uri, r#"{"file":0}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["layers"][0]["address"]["file"], 0);
        assert_eq!(
            bench.state.load().output(bench.output).unwrap().layers[0].address,
            MediaAddress::new(1, 0)
        );
    }
}
