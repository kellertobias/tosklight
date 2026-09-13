use std::sync::Arc;

use axum::http::{StatusCode, header};
use http_body_util::BodyExt as _;
use media_application::configuration::{MediaConfiguration, OutputConfiguration};
use media_domain::{
    CommandSource, LayerPersonality, MediaAddress, MediaState, OutputId, Timestamp,
};
use tower::ServiceExt as _;

use crate::routes::bench::{Bench, bench, get, post, send};

#[test]
fn concurrent_resets_are_each_applied_once_without_losing_updates() {
    let output = OutputId::new();
    let state = arc_swap::ArcSwap::from_pointee(MediaState::with_outputs(vec![
        media_domain::OutputState::new(output, LayerPersonality::TwoLayers),
    ]));
    super::submit_to(
        &state,
        vec![media_domain::CommandKind::TakeOverPlayback {
            output,
            take_over: true,
        }],
        Timestamp::ZERO,
    )
    .unwrap();
    let barrier = std::sync::Barrier::new(4);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..500 {
                    barrier.wait();
                    super::submit_to(
                        &state,
                        vec![media_domain::CommandKind::ResetLayer { output, layer: 0 }],
                        Timestamp::ZERO,
                    )
                    .unwrap();
                }
            });
        }
    });
    assert_eq!(
        state.load().output(output).unwrap().layers[0].reset_trigger_id,
        2_000
    );
}

async fn take_over(bench: &Bench) {
    let (status, _) = send(
        &bench.router,
        get(format!(
            "/api/v2/outputs/{}/playback/take-over",
            bench.output
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

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
    assert_eq!(body["layers"][0]["effects"].as_array().unwrap().len(), 4);
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
    assert_eq!(body["channels"].as_array().unwrap().len(), 158);
    assert_eq!(body["channels"][0]["absoluteChannel"], 1);
    assert_eq!(body["channels"][0]["name"], "Folder");
    assert_eq!(body["channels"][118]["group"]["kind"], "master");

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
            r#"{"requestId":"settings-1","targetKind":"monitor","monitorBy":"name","monitorValue":" Stage Right ","fullscreen":true,"width":1280,"height":720,"presentation":"fixed-fps","framesPerSecond":50,"personality":"eight-layers","protocol":"sacn","universe":7,"startAddress":1}"#,
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
    assert_eq!(body["framesPerSecond"], 50.0);
    assert_eq!(body["personality"], "eight-layers");
    assert_eq!(body["protocol"], "sacn");
    assert_eq!(body["universe"], 7);
    assert_eq!(body["startAddress"], 1);
    assert_eq!(body["takesEffectOnRestart"], true);
    assert_eq!(body["active"]["width"], 1920);
    assert_eq!(body["active"]["protocol"], "art-net");
    assert_eq!(body["picturePendingRestart"], true);
    assert_eq!(body["soundPendingRestart"], false);
    assert_eq!(body["dmxPendingRestart"], true);

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
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);

    let (status, body) = send(&bench.router, post(uri.clone(), r#"{"folder":3,"file":7}"#)).await;
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
async fn a_layer_and_master_accept_the_network_equivalent_controls() {
    let bench = bench();
    take_over(&bench).await;
    let (_, layer) = send(
        &bench.router,
        post(format!("/api/v2/outputs/{}/layers/0/update", bench.output),
            r#"{"playModeDmx":236,"scaleX":10,"scaleY":2,"scalingMode":"stretch","positionX":-2,"positionY":2,"rotation":360,"volume":0.4,"tintRed":0.2,"tintGreen":0.3,"tintBlue":0.4,"grayscale":0.5,"maskFolder":2,"maskFile":3,"maskScaleX":2,"maskScaleY":1.5,"maskPositionX":-0.75,"maskPositionY":1.25,"maskInvert":true,"maskOpacity":0.8,"speedMultiplierDmx":255,"playbackBpm":120}"#),
    ).await;
    assert_eq!(layer["layers"][0]["playMode"], "Pause");
    assert_eq!(layer["layers"][0]["scalingMode"], "stretch");
    assert_eq!(layer["layers"][0]["speedMultiplier"], "16×");
    assert_eq!(layer["layers"][0]["mask"]["address"]["file"], 3);
    assert_eq!(layer["layers"][0]["mask"]["positionX"], -0.75);
    assert_eq!(layer["layers"][0]["mask"]["positionY"], 1.25);

    let (_, master) = send(
        &bench.router,
        post(format!("/api/v2/outputs/{}/master/update", bench.output),
            r#"{"dimmer":0.4,"volume":0.5,"tintRed":0.6,"tintGreen":0.7,"tintBlue":0.8,"flipMirror":"both","maskFolder":4,"maskFile":5,"maskPositionX":0.75,"maskPositionY":-1.25,"scaleX":1.5,"scaleY":0.75,"scalingMode":"fill","positionX":0.5,"positionY":-0.5,"rotation":30,"shaperLeft":0.1,"shaperRight":0.2,"shaperTop":0.3,"shaperBottom":0.4,"shaperLeftRotation":10,"shaperRightRotation":-10,"shaperTopRotation":20,"shaperBottomRotation":-20,"shaperRotation":15}"#),
    ).await;
    assert_eq!(master["master"]["flipMirror"], "both");
    assert_eq!(master["master"]["mask"]["file"], 5);
    assert_eq!(master["master"]["maskPositionX"], 0.75);
    assert_eq!(master["master"]["maskPositionY"], -1.25);
    assert_eq!(master["master"]["scaleX"], 1.5);
    assert_eq!(master["master"]["scalingMode"], "fill");
    assert_eq!(master["master"]["positionY"], -0.5);
    assert_eq!(master["master"]["rotation"], 30.0);
    assert_eq!(master["master"]["shaperLeft"], 0.1);
    assert_eq!(master["master"]["shaperBottomRotation"], -20.0);
    assert_eq!(master["master"]["shaperRotation"], 15.0);
}

#[tokio::test]
async fn analog_tv_is_a_typed_intent_shaped_effect_edit() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(uri.clone(), r#"{"effectSlot":1,"effectType":"analog-tv"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][1];
    assert_eq!(effect["effectType"], "analog-tv");
    assert_eq!(effect["label"], "Analog TV");
    assert_eq!(effect["supported"], true);
    assert_eq!(effect["parameters"][0]["id"], "tv-curvature");
    assert_eq!(effect["parameters"][0]["value"], 0.30);
    assert_eq!(effect["parameters"][1]["value"], 0.18);
    assert_eq!(effect["parameters"][2]["value"], 0.20);
    assert_eq!(effect["parameters"][3]["value"], 0.08);

    let (status, tuned) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":1,"tvCurvature":0,"effectDistortion":0.6,"imageGrain":0,"effectGlitching":1}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &tuned["layers"][0]["effects"][1];
    assert_eq!(effect["parameters"][0]["value"], 0.0);
    assert_eq!(effect["parameters"][1]["value"], 0.6);
    assert_eq!(effect["parameters"][2]["value"], 0.0);
    assert_eq!(effect["parameters"][3]["value"], 1.0);

    let (_, cleared) = send(
        &bench.router,
        post(uri, r#"{"effectSlot":1,"effectType":"none"}"#),
    )
    .await;
    assert!(cleared["layers"][0]["effects"][1]["effectType"].is_null());
}

#[tokio::test]
async fn opacity_cycle_persists_its_named_interval() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(
            uri,
            r#"{"effectSlot":0,"effectType":"opacity-cycle","cycleInterval":"every-half-beat"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][0];
    assert_eq!(effect["effectType"], "opacity-cycle");
    assert_eq!(effect["label"], "Layer opacity cycle");
    assert_eq!(effect["parameters"][0]["id"], "cycle-interval");
    assert_eq!(effect["parameters"][0]["value"], 1.0);
}

#[tokio::test]
async fn blur_persists_its_live_amount_and_bypass() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":0,"effectType":"blur","blurAmount":0.8}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][0];
    assert_eq!(effect["effectType"], "blur");
    assert_eq!(effect["parameters"][0]["id"], "blur-amount");
    assert_eq!(effect["parameters"][0]["value"], 0.8);

    let (_, bypassed) = send(
        &bench.router,
        post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
    )
    .await;
    assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
}

#[tokio::test]
async fn feedback_persists_all_motion_controls_and_bypass() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":0,"effectType":"feedback","feedbackAmount":0.7,"feedbackMotion":0.4,"feedbackDirection":"rotate-right"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][0];
    assert_eq!(effect["effectType"], "feedback");
    assert_eq!(effect["label"], "Feedback");
    assert_eq!(effect["parameters"][0]["value"], 0.7);
    assert_eq!(effect["parameters"][1]["value"], 0.4);
    assert_eq!(effect["parameters"][2]["value"], 5.0);

    let (_, bypassed) = send(
        &bench.router,
        post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
    )
    .await;
    assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
}

#[tokio::test]
async fn beat_move_persists_amount_direction_return_time_and_bypass() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":0,"effectType":"beat-move","beatMoveAmount":0.3,"beatMoveDirection":"right","beatMoveDecay":0.8}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][0];
    assert_eq!(effect["effectType"], "beat-move");
    assert_eq!(effect["label"], "Beat Move");
    assert_eq!(effect["parameters"][0]["value"], 0.3);
    assert_eq!(effect["parameters"][1]["value"], 3.0);
    assert_eq!(effect["parameters"][2]["value"], 0.8);

    let (_, bypassed) = send(
        &bench.router,
        post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
    )
    .await;
    assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
}

#[tokio::test]
async fn kaleidoscope_persists_live_repetitions_angle_and_bypass() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":0,"effectType":"kaleidoscope","kaleidoscopeRepetitions":8,"kaleidoscopeAngle":37}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][0];
    assert_eq!(effect["effectType"], "kaleidoscope");
    assert_eq!(effect["label"], "Kaleidoscope");
    assert_eq!(effect["parameters"][0]["value"], 8.0);
    assert_eq!(effect["parameters"][1]["value"], 37.0);

    let (_, bypassed) = send(
        &bench.router,
        post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
    )
    .await;
    assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
}

#[tokio::test]
async fn rasterize_persists_print_mode_dot_size_and_bypass() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":0,"effectType":"rasterize","rasterizeMode":"cmyk","rasterizeDotSize":18}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][0];
    assert_eq!(effect["effectType"], "rasterize");
    assert_eq!(effect["label"], "Rasterized Print");
    assert_eq!(effect["parameters"][0]["value"], 1.0);
    assert_eq!(effect["parameters"][1]["value"], 18.0);

    let (_, bypassed) = send(
        &bench.router,
        post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
    )
    .await;
    assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
}

#[tokio::test]
async fn beat_scan_persists_width_edge_falloff_duration_and_bypass() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":0,"effectType":"beat-scan","beatScanWidth":0.12,"beatScanEdge":"soft","beatScanFalloff":0.7,"beatScanDuration":2.25}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][0];
    assert_eq!(effect["effectType"], "beat-scan");
    assert_eq!(effect["label"], "Beat Scan");
    assert_eq!(effect["parameters"][0]["value"], 0.12);
    assert_eq!(effect["parameters"][1]["value"], 1.0);
    assert_eq!(effect["parameters"][2]["value"], 0.7);
    assert_eq!(effect["parameters"][3]["value"], 2.25);

    let (_, bypassed) = send(
        &bench.router,
        post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
    )
    .await;
    assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
}

#[tokio::test]
async fn beat_scale_turn_persists_independent_turn_amounts_decay_and_bypass() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":0,"effectType":"beat-scale-turn","beatScaleAmount":0.22,"beatTurnEnabled":true,"beatTurnRotation":-7,"beatScaleDecay":0.8}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][0];
    assert_eq!(effect["effectType"], "beat-scale-turn");
    assert_eq!(effect["label"], "Beat Scale and Turn");
    assert_eq!(effect["parameters"][0]["value"], 0.22);
    assert_eq!(effect["parameters"][1]["value"], 1.0);
    assert_eq!(effect["parameters"][2]["value"], -7.0);
    assert_eq!(effect["parameters"][3]["value"], 0.8);

    let (_, bypassed) = send(
        &bench.router,
        post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
    )
    .await;
    assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
}

#[tokio::test]
async fn beat_grid_wave_persists_origin_shape_colour_brightness_and_bypass() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":0,"effectType":"beat-grid-wave","beatGridDensity":36,"beatGridHeight":0.72,"beatGridDuration":1.8,"beatGridOrigin":"left","beatGridHue":280,"beatGridBrightness":1.4}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][0];
    assert_eq!(effect["effectType"], "beat-grid-wave");
    assert_eq!(effect["label"], "Beat Grid Wave");
    assert_eq!(effect["parameters"][0]["value"], 36.0);
    assert_eq!(effect["parameters"][1]["value"], 0.72);
    assert_eq!(effect["parameters"][2]["value"], 1.8);
    assert_eq!(effect["parameters"][3]["value"], 4.0);
    assert_eq!(effect["parameters"][4]["value"], 280.0);
    assert_eq!(effect["parameters"][5]["value"], 1.4);

    let (_, bypassed) = send(
        &bench.router,
        post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
    )
    .await;
    assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
}

#[tokio::test]
async fn beat_form_flash_persists_size_lifetime_density_variation_and_bypass() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":0,"effectType":"beat-form-flash","beatFormEnlargement":2.4,"beatFormLifetime":1.6,"beatFormDensity":3,"beatFormVariation":0.65}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][0];
    assert_eq!(effect["effectType"], "beat-form-flash");
    assert_eq!(effect["label"], "Beat Form Flash");
    assert_eq!(effect["parameters"][0]["value"], 2.4);
    assert_eq!(effect["parameters"][1]["value"], 1.6);
    assert_eq!(effect["parameters"][2]["value"], 3.0);
    assert_eq!(effect["parameters"][3]["value"], 0.65);

    let (_, bypassed) = send(
        &bench.router,
        post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
    )
    .await;
    assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
}

#[tokio::test]
async fn slot_one_persists_visualizer_parameters_only_for_a_visualizer_layer() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, _) = send(
        &bench.router,
        post(uri.clone(), r#"{"folder":250,"file":1}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut parameters =
        crate::wire::VisualizerParametersView::of(&media_domain::VisualizerParameters::default());
    parameters.size = 0.2;
    let body = serde_json::json!({
        "effectSlot": 0,
        "visualizerParameters": parameters,
    })
    .to_string();
    let (status, tuned) = send(&bench.router, post(uri.clone(), &body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        tuned["layers"][0]["effects"][0]["visualizerParameters"]["size"],
        0.2
    );
    assert_eq!(
        bench.state.load().output(bench.output).unwrap().layers[0].effects[0]
            .visualizer_parameters
            .as_ref()
            .unwrap()
            .size,
        0.2
    );

    let (_, _) = send(&bench.router, post(uri.clone(), r#"{"folder":1,"file":1}"#)).await;
    let (status, rejected) = send(&bench.router, post(uri, &body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(rejected["code"], "visualizer-controls-source");
}

#[tokio::test]
async fn invalid_or_unsupported_effect_edits_leave_the_chain_untouched() {
    let bench = bench();
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    for (body, code) in [
        (r#"{"effectType":"analog-tv"}"#, "effect-slot-required"),
        (
            r#"{"effectSlot":4,"effectType":"analog-tv"}"#,
            "effect-slot-out-of-range",
        ),
        (
            r#"{"effectSlot":0,"effectType":"future-effect"}"#,
            "effect-unsupported",
        ),
        (
            r#"{"effectSlot":0,"effectType":"analog-tv","imageGrain":1.1}"#,
            "control-out-of-range",
        ),
    ] {
        let (status, response) = send(&bench.router, post(uri.clone(), body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(response["code"], code, "{body}");
        assert!(
            bench.state.load().output(bench.output).unwrap().layers[0].effects[0]
                .effect_type
                .is_none(),
            "the rejected edit published nothing"
        );
    }
}

#[tokio::test]
async fn digital_tv_is_a_five_parameter_typed_intent_shaped_effect_edit() {
    let bench = bench();
    take_over(&bench).await;
    let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
    let (status, selected) = send(
        &bench.router,
        post(uri.clone(), r#"{"effectSlot":2,"effectType":"digital-tv"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effect = &selected["layers"][0]["effects"][2];
    assert_eq!(effect["effectType"], "digital-tv");
    assert_eq!(effect["label"], "Digital TV");
    assert_eq!(effect["parameters"][0]["value"], 0.35);
    assert_eq!(effect["parameters"][1]["value"], 0.35);
    assert_eq!(effect["parameters"][2]["value"], 0.25);
    assert_eq!(effect["parameters"][3]["value"], 0.20);
    assert_eq!(effect["parameters"][4]["value"], 0.15);

    let (status, tuned) = send(
        &bench.router,
        post(
            uri,
            r#"{"effectSlot":2,"compressionDamage":0,"blockSize":1,"tileDisplacement":0.6,"chromaDamage":0.4,"effectGlitching":1}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let values = tuned["layers"][0]["effects"][2]["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|parameter| parameter["value"].as_f64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values, vec![0.0, 1.0, 0.6, 0.4, 1.0]);
}

#[tokio::test]
async fn an_invalid_control_range_publishes_none_of_the_update() {
    let bench = bench();
    let (status, body) = send(
        &bench.router,
        post(
            format!("/api/v2/outputs/{}/layers/0/update", bench.output),
            r#"{"dimmer":0.25,"scaleX":10.01}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "control-out-of-range");
    assert_eq!(
        bench.state.load().output(bench.output).unwrap().layers[0].dimmer,
        1.0
    );
}

#[tokio::test]
async fn unknown_fields_are_accepted_rather_than_rejected() {
    let bench = bench();
    take_over(&bench).await;
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
async fn takeover_and_release_are_get_actions_that_must_not_be_cached() {
    let bench = bench();
    for (mode, expected) in [("take-over", true), ("release", false)] {
        let response = bench
            .router
            .clone()
            .oneshot(get(format!(
                "/api/v2/outputs/{}/playback/{mode}",
                bench.output
            )))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            bench
                .state
                .load()
                .output(bench.output)
                .unwrap()
                .ownership
                .web_takeover,
            expected
        );
    }
}

#[tokio::test]
async fn playback_changes_require_explicit_takeover_even_before_dmx() {
    let bench = bench();
    let (status, body) = send(
        &bench.router,
        post(
            format!("/api/v2/outputs/{}/layers/0/update", bench.output),
            r#"{"dimmer":0.5}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "playback-takeover-required");
}

#[tokio::test]
async fn an_output_preview_returns_the_renderer_citp_frame_without_caching() {
    let bench = bench();
    *bench.preview_frame.lock().unwrap() = Some(crate::OutputPreviewFrame {
        sequence: 7,
        width: 320,
        height: 180,
        content_type: "image/jpeg",
        bytes: vec![0xff, 0xd8, 0xff, 0xd9],
    });
    let response = bench
        .router
        .clone()
        .oneshot(get(format!(
            "/api/v2/outputs/{}/preview?width=320&height=180",
            bench.output
        )))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(response.headers()["x-tosklight-preview-sequence"], "7");
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        &[0xff, 0xd8, 0xff, 0xd9][..]
    );
}

#[tokio::test]
async fn a_layer_preview_returns_an_isolated_live_renderer_frame() {
    let bench = bench();
    *bench.preview_frame.lock().unwrap() = Some(crate::OutputPreviewFrame {
        sequence: 8,
        width: 160,
        height: 90,
        content_type: "image/png",
        bytes: vec![0x89, b'P', b'N', b'G'],
    });
    let response = bench
        .router
        .clone()
        .oneshot(get(format!(
            "/api/v2/outputs/{}/layers/0/preview?width=160&height=90",
            bench.output
        )))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(response.headers()["x-tosklight-preview-sequence"], "8");
}

#[tokio::test]
async fn preview_routes_always_return_valid_image_payloads_before_the_renderer_captures() {
    let bench = bench();
    for (path, expected_fill) in [
        (format!("/api/v2/outputs/{}/preview", bench.output), "black"),
        (
            format!("/api/v2/outputs/{}/layers/0/preview", bench.output),
            "none",
        ),
    ] {
        let response = bench.router.clone().oneshot(get(path)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/svg+xml");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(
            std::str::from_utf8(&body)
                .unwrap()
                .contains(&format!("fill=\"{expected_fill}\""))
        );
    }
}

#[tokio::test]
async fn a_live_desk_keeps_the_web_ui_read_only_with_a_reason() {
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
    assert_eq!(body["code"], "playback-takeover-required");

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
async fn a_live_native_desk_can_configure_an_effect_without_taking_over_dmx() {
    let bench = bench();
    let mut next = MediaState::clone(&bench.state.load());
    next.outputs[0]
        .ownership
        .observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(0));
    bench.state.store(Arc::new(next));
    let uri = format!(
        "/api/v2/outputs/{}/layers/0/native-effects/update",
        bench.output
    );

    let (status, body) = send(
        &bench.router,
        post(
            uri.clone(),
            r#"{"effectSlot":0,"effectType":"blur","blurAmount":0.7}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["dmxActive"], true);
    assert_eq!(body["playbackTakeover"], false);
    assert_eq!(body["layers"][0]["effects"][0]["effectType"], "blur");

    for payload in [
        r#"{"effectSlot":0,"effectMix":0.7}"#,
        r#"{"effectSlot":0,"effectEnabled":true,"folder":2}"#,
    ] {
        let (status, body) = send(&bench.router, post(uri.clone(), payload)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "native-effect-configuration-only");
    }
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
    let (status, body) = send(&bench.router, get("/api/v2/outputs/nonsense/state".into())).await;
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
    take_over(&bench).await;
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
