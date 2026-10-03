use super::*;

#[test]
fn installed_appearance_crosses_the_planning_contract_without_identity_loss() {
    let appearance = InstalledFixtureAppearance {
        light_source: InstalledLightSource::Tungsten,
        color_temperature_kelvin: Some(3_200),
        luminous_output_lumens: Some(18_000.0),
        gel: GelAssignment::BuiltIn {
            catalog_id: "touring-gels".into(),
            entry_id: "deep-red".into(),
            embedded_fallback: GelDefinitionSnapshot {
                number: "R1".into(),
                name: "Deep red".into(),
                display_srgb: "#D92838".into(),
                visualizer_srgb: "#C01020".into(),
            },
        },
        shaper_angles_degrees: [-10.0, 20.0, 0.0, 179.0],
    };

    let value = serde_json::to_value(InstalledAppearanceDto::from(&appearance)).unwrap();
    assert_eq!(value["lightSource"]["type"], "tungsten");
    assert_eq!(value["colorTemperatureKelvin"], 3_200);
    assert_eq!(value["luminousOutputLumens"], 18_000.0);
    assert_eq!(value["gel"]["catalogId"], "touring-gels");
    assert_eq!(
        value["gel"]["embeddedFallback"]["visualizerSrgb"],
        "#C01020"
    );
    let decoded: InstalledAppearanceDto = serde_json::from_value(value).unwrap();
    assert_eq!(InstalledFixtureAppearance::from(decoded), appearance);
}

#[test]
fn a_venue_object_keeps_its_placed_size_through_the_sheet() {
    let fixture: FixtureDto = serde_json::from_value(serde_json::json!({
        "fixtureId": Uuid::new_v4(),
        "fixtureNumber": null,
        "virtualFixtureNumber": 3,
        "name": "Truss",
        "profileId": Uuid::new_v4(),
        "profileRevision": 1,
        "modeId": Uuid::new_v4(),
        "splitPatches": [{ "split": 1, "universe": null, "address": null }],
        "layerId": "default",
        "location": { "x": 0, "y": 0, "z": 0 },
        "rotation": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "scenerySizeMetres": { "x": 6000.0, "y": 340.0, "z": 340.0 }
    }))
    .expect("fixture DTO");
    let candidate = PatchFixtureCandidate::from(fixture);
    assert_eq!(
        candidate.patch.scenery_size_metres,
        Some(FixtureVector {
            x: 6000.0,
            y: 340.0,
            z: 340.0
        })
    );

    // A fixture written before sizes travelled through this sheet still reads as default size.
    let legacy: FixtureDto = serde_json::from_value(serde_json::json!({
        "fixtureId": Uuid::new_v4(),
        "fixtureNumber": 1,
        "virtualFixtureNumber": null,
        "name": "Wash",
        "profileId": Uuid::new_v4(),
        "profileRevision": 1,
        "modeId": Uuid::new_v4(),
        "splitPatches": [],
        "layerId": "default",
        "location": { "x": 0, "y": 0, "z": 0 },
        "rotation": { "x": 0.0, "y": 0.0, "z": 0.0 }
    }))
    .expect("legacy fixture DTO");
    assert_eq!(
        PatchFixtureCandidate::from(legacy)
            .patch
            .scenery_size_metres,
        None
    );
}

#[test]
fn a_venue_object_keeps_its_model_scale_through_the_sheet() {
    let sheet = |extra: serde_json::Value| -> FixtureDto {
        let mut body = serde_json::json!({
            "fixtureId": Uuid::new_v4(),
            "fixtureNumber": null,
            "virtualFixtureNumber": 6,
            "name": "Hall",
            "profileId": Uuid::new_v4(),
            "profileRevision": 1,
            "modeId": Uuid::new_v4(),
            "splitPatches": [{ "split": 1, "universe": null, "address": null }],
            "layerId": "default",
            "location": { "x": 0, "y": 0, "z": 0 },
            "rotation": { "x": 0.0, "y": 0.0, "z": 0.0 }
        });
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value(body).expect("fixture DTO")
    };
    let scaled = sheet(serde_json::json!({ "modelScale": 2.5 }));
    assert_eq!(serde_json::to_value(&scaled).unwrap()["modelScale"], 2.5);
    assert_eq!(
        PatchFixtureCandidate::from(scaled).patch.model_scale,
        Some(2.5)
    );

    // A payload written before the scale existed is at its built size and writes none back.
    let legacy = sheet(serde_json::json!({}));
    assert!(
        serde_json::to_value(&legacy)
            .unwrap()
            .get("modelScale")
            .is_none()
    );
    assert_eq!(PatchFixtureCandidate::from(legacy).patch.model_scale, None);
}

/// A part added from the CAD dialog is written by the patch sheet's own candidate, which sends
/// `null` for every choice it has not made. That is nothing chosen, not a malformed mutation.
#[test]
fn a_new_venue_object_with_no_choices_sent_as_null_is_accepted() {
    let fixture: FixtureDto = serde_json::from_value(serde_json::json!({
        "fixtureId": Uuid::new_v4(),
        "fixtureNumber": null,
        "virtualFixtureNumber": 3,
        "name": "Four-Point Truss Corner 2-Way",
        "profileId": Uuid::new_v4(),
        "profileRevision": 1,
        "modeId": Uuid::new_v4(),
        "splitPatches": [{ "split": 1, "universe": null, "address": null }],
        "layerId": "default",
        "directControl": null,
        "location": { "x": 0, "y": 0, "z": 0 },
        "rotation": { "x": 0, "y": 0, "z": 0 },
        "multipatch": [],
        "shaperAngle": null,
        "scenerySizeMetres": null,
        "sceneryOptions": null,
        "modelScale": null
    }))
    .expect("a new part's fixture DTO");
    assert!(fixture.scenery_options.is_empty());
    let patch = PatchFixtureCandidate::from(fixture).patch;
    assert_eq!(patch.scenery_options, SceneryOptions::default());
    assert_eq!(patch.model_scale, None);
}

#[test]
fn a_venue_object_keeps_its_scenery_options_through_the_sheet() {
    let fixture: FixtureDto = serde_json::from_value(serde_json::json!({
        "fixtureId": Uuid::new_v4(),
        "fixtureNumber": null,
        "virtualFixtureNumber": 4,
        "name": "Chain",
        "profileId": Uuid::new_v4(),
        "profileRevision": 1,
        "modeId": Uuid::new_v4(),
        "splitPatches": [{ "split": 1, "universe": null, "address": null }],
        "layerId": "default",
        "location": { "x": 0, "y": 0, "z": 0 },
        "rotation": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "sceneryOptions": {
            "colourSrgb": "#1A2B3C",
            "chainTop": "direct",
            "chainBottom": "steelflex_loop",
            "handrails": "right"
        }
    }))
    .expect("fixture DTO");
    let options = PatchFixtureCandidate::from(fixture).patch.scenery_options;
    assert_eq!(
        options,
        SceneryOptions {
            colour_srgb: Some("#1A2B3C".into()),
            chain_top: Some(ChainTopEnd::Direct),
            chain_bottom: Some(ChainBottomEnd::SteelflexLoop),
            handrails: Some(light_fixture::StairHandrails::Right),
        }
    );
    assert_eq!(
        serde_json::to_value(SceneryOptionsDto::from(&options)).unwrap(),
        serde_json::json!({
            "colourSrgb": "#1A2B3C",
            "chainTop": "direct",
            "chainBottom": "steelflex_loop",
            "handrails": "right"
        })
    );

    // A payload written before options travelled through this sheet reads as nothing chosen.
    let legacy: FixtureDto = serde_json::from_value(serde_json::json!({
        "fixtureId": Uuid::new_v4(),
        "fixtureNumber": null,
        "virtualFixtureNumber": 5,
        "name": "Curtain",
        "profileId": Uuid::new_v4(),
        "profileRevision": 1,
        "modeId": Uuid::new_v4(),
        "splitPatches": [],
        "layerId": "default",
        "location": { "x": 0, "y": 0, "z": 0 },
        "rotation": { "x": 0.0, "y": 0.0, "z": 0.0 }
    }))
    .expect("legacy fixture DTO");
    assert!(
        serde_json::to_value(&legacy)
            .unwrap()
            .get("sceneryOptions")
            .is_none()
    );
    assert!(
        PatchFixtureCandidate::from(legacy)
            .patch
            .scenery_options
            .is_empty()
    );
}

#[test]
fn mutation_keeps_dmx_placement_intent() {
    let fixture_id = Uuid::new_v4();
    let mutation: MutationDto = serde_json::from_value(serde_json::json!({
        "requestId": "placement-test",
        "fixtures": [],
        "removeFixtureIds": [],
        "placements": [{
            "fixtureIds": [fixture_id],
            "splits": [{
                "split": 1,
                "universe": 2,
                "address": 101,
                "mode": { "type": "consecutive" }
            }]
        }]
    }))
    .expect("mutation DTO");

    let command = mutation.into_command(ShowId(Uuid::new_v4()));
    assert_eq!(command.placements.len(), 1);
    assert_eq!(
        command.placements[0].fixture_ids,
        vec![FixtureId(fixture_id)]
    );
    assert_eq!(command.placements[0].splits[0].universe, Some(2));
    assert_eq!(command.placements[0].splits[0].address, Some(101));
    assert_eq!(
        command.placements[0].splits[0].mode,
        PatchSplitPlacementMode::Consecutive
    );
}
#[test]
fn position_calibration_survives_architect_patch_read_edit_write() {
    let color = serde_json::json!({"version":1,"revision":2,"paths":[{
        "source_identity":{"profile_id":Uuid::new_v4(),"profile_revision":1,"profile_digest":"a".repeat(64),
            "mode_id":Uuid::new_v4(),"head_id":Uuid::new_v4(),"path_id":Uuid::new_v4(),"model_revision":3,"native_layout_signature":"b".repeat(64)},
        "emitters":[{"emitter_id":Uuid::new_v4(),"output_gain":0.0,"provenance":{"quality":"estimated","revision":1}}],"measurements":[]
    }]});
    let calibration = serde_json::json!({
        "revision": 3, "quality": "measured", "source": "Rig record",
        "pan_zero_degrees": -720.5, "tilt_zero_degrees": 12.25,
    });
    let fixture_id = Uuid::new_v4();
    let copy_id = Uuid::new_v4();
    let mut fixture: FixtureDto = serde_json::from_value(serde_json::json!({
        "fixtureId": fixture_id, "fixtureNumber": 1, "virtualFixtureNumber": null,
        "name": "Calibrated", "profileId": Uuid::new_v4(), "profileRevision": 1,
        "modeId": Uuid::new_v4(), "splitPatches": [{"split":1,"universe":null,"address":null}],
        "layerId": "default", "directControl": null,
        "location": {"x":1000,"y":2000,"z":3000}, "rotation": {"x":0,"y":0,"z":0},
        "colorCalibration":color,
        "positionCalibration": calibration, "moveInBlackEnabled": true, "moveInBlackDelayMillis": 0,
        "highlightOverrides": [], "multipatch": [{
            "id":copy_id,"name":"Copy","splitPatches":[{"split":1,"universe":null,"address":null}],
            "location":{"x":0,"y":0,"z":0},"rotation":{"x":0,"y":0,"z":0},
            "positionCalibration":{"pan_zero_degrees":90}, "invertPan":true,
            "colorCalibration":color,
        }],
    }))
    .unwrap();
    fixture.name = "Renamed without recalibrating".into();
    let candidate = PatchFixtureCandidate::from(fixture);
    assert_eq!(
        candidate
            .patch
            .position_calibration
            .as_ref()
            .unwrap()
            .pan_zero_degrees,
        -720.5
    );
    assert_eq!(
        candidate.patch.multipatch[0]
            .position_calibration
            .as_ref()
            .unwrap()
            .pan_zero_degrees,
        90.0
    );
    let projection = PatchFixtureProjection {
        fixture_revision: 1,
        profile: candidate.profile,
        patch: candidate.patch,
    };
    let returned = serde_json::to_value(FixtureDto::from(projection)).unwrap();
    assert_eq!(returned["positionCalibration"], calibration);
    assert_eq!(returned["colorCalibration"], color);
    assert_eq!(returned["multipatch"][0]["colorCalibration"], color);
    assert_eq!(
        returned["multipatch"][0]["positionCalibration"]["pan_zero_degrees"],
        90.0
    );
    assert_eq!(returned["multipatch"][0]["invertPan"], true);
}
