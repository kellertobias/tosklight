//! Layer state and persisted effect contracts.
use super::*;

#[test]
fn older_effect_slots_default_to_detected_beats() {
    let stored = r#"{"effectType":"beat-move","enabled":true,"seed":1,"mix":1.0,"parameters":[]}"#;
    let effect: EffectSlot = serde_json::from_str(stored).unwrap();
    assert_eq!(effect.beat_source, BeatSource::DetectedBeat);
    assert_eq!(
        serde_json::to_value(&effect).unwrap()["beatSource"],
        "detected-beat"
    );
}

#[test]
fn scaling_modes_tile_the_channel_in_four_sixty_four_value_ranges() {
    let mut next = 0u16;
    for mode in ScalingMode::ALL {
        let (low, high) = mode.dmx_range();
        assert_eq!(u16::from(low), next);
        assert_eq!(high - low + 1, 64);
        next = u16::from(high) + 1;
    }
    assert_eq!(next, 256);
    for value in 0..=255u8 {
        let mode = ScalingMode::from_dmx(value);
        let (low, high) = mode.dmx_range();
        assert!((low..=high).contains(&value), "{value}");
    }
}

#[test]
fn a_fresh_layer_is_neutral_and_selects_nothing() {
    let layer = LayerState::default();
    assert_eq!(layer.address, MediaAddress::BLANK);
    assert_eq!(layer.scale_x, 1.0);
    assert_eq!(layer.dimmer, 1.0);
    assert_eq!(layer.tint, Tint::WHITE);
    assert_eq!(layer.source_status, SourceStatus::Unselected);
    assert_eq!(layer.playback_bpm, None);
    assert!(!layer.draws());
}

#[test]
fn a_layer_stops_drawing_for_each_documented_reason() {
    let ready = LayerState {
        address: MediaAddress::new(1, 1),
        source_status: SourceStatus::Ready,
        ..Default::default()
    };
    assert!(ready.draws());

    assert!(
        !LayerState {
            dimmer: 0.0,
            ..ready.clone()
        }
        .draws(),
        "dimmer at zero"
    );
    assert!(
        !LayerState {
            address: MediaAddress::BLANK,
            ..ready.clone()
        }
        .draws(),
        "no source selected"
    );
    assert!(
        !LayerState {
            source_status: SourceStatus::Failed {
                failure: SourceFailure::MissingFile
            },
            ..ready.clone()
        }
        .draws(),
        "the selected source cannot load"
    );
}

#[test]
fn a_failure_keeps_the_selected_address() {
    let layer = LayerState {
        address: MediaAddress::new(3, 7),
        source_status: SourceStatus::Failed {
            failure: SourceFailure::DecodeFailed,
        },
        ..Default::default()
    };
    assert_eq!(
        layer.address,
        MediaAddress::new(3, 7),
        "the desk's selection must survive"
    );
    assert!(layer.source_status.is_failed());
}

#[test]
fn failure_codes_are_categories_and_say_whether_a_retry_can_help() {
    assert_eq!(SourceFailure::MissingFile.code(), "MissingFile");
    assert!(SourceFailure::MissingFile.is_retryable());
    assert!(!SourceFailure::UnsupportedCodec.is_retryable());
}

#[test]
fn a_missing_mask_is_no_mask() {
    let mut mask = MaskState {
        opacity: 1.0,
        ..Default::default()
    };
    assert!(
        !mask.is_active(),
        "a blank mask address must not black the layer out"
    );
    mask.address = MediaAddress::new(2, 5);
    assert!(mask.is_active());
    mask.opacity = 0.0;
    assert!(!mask.is_active());
}

#[test]
fn effect_slots_start_empty() {
    let layer = LayerState::default();
    assert_eq!(layer.effects.len(), 4);
    for slot in &layer.effects {
        assert_eq!(slot.effect_type, None);
        assert!(!slot.enabled);
        assert_eq!(slot.mix, 0.0);
    }
}

#[test]
fn analog_tv_has_typed_restrained_defaults_and_normalized_endpoints() {
    let mut effect = EffectSlot::analog_tv();
    assert_eq!(effect.effect_type.as_deref(), Some(ANALOG_TV_EFFECT));
    assert!(effect.enabled);
    assert_eq!(
        effect.analog_tv_parameters(),
        Some(AnalogTvParameters::default())
    );
    assert_eq!(AnalogTvParameters::IDS.len(), effect.parameters.len());

    effect.mix = 2.0;
    effect.parameters = vec![-1.0, 0.4, 1.5, f32::NAN];
    effect.normalize();
    assert_eq!(effect.mix, 1.0);
    assert_eq!(effect.parameters, vec![0.0, 0.4, 1.0, 0.08]);

    effect.mix = 0.0;
    assert_eq!(effect.analog_tv_parameters(), None, "zero mix bypasses");
    effect.mix = 1.0;
    effect.enabled = false;
    assert_eq!(effect.analog_tv_parameters(), None, "disabled bypasses");
}

#[test]
fn digital_tv_has_five_typed_restrained_defaults_and_normalized_endpoints() {
    let mut effect = EffectSlot::digital_tv();
    assert_eq!(effect.effect_type.as_deref(), Some(DIGITAL_TV_EFFECT));
    assert_eq!(
        effect.digital_tv_parameters(),
        Some(DigitalTvParameters::default())
    );
    assert_eq!(DigitalTvParameters::IDS.len(), effect.parameters.len());

    effect.parameters = vec![-1.0, 0.25, 0.5, 1.5, f32::NAN];
    effect.normalize();
    assert_eq!(effect.parameters, vec![0.0, 0.25, 0.5, 1.0, 0.15]);
}

#[test]
fn feedback_has_stable_defaults_directions_and_safe_normalization() {
    let mut effect = EffectSlot::feedback();
    assert_eq!(effect.effect_type.as_deref(), Some(FEEDBACK_EFFECT));
    assert_eq!(
        effect.feedback_parameters(),
        Some(FeedbackParameters::default())
    );
    assert_eq!(
        FeedbackMotion::parse("rotate-right"),
        Some(FeedbackMotion::RotateRight)
    );
    assert_eq!(FeedbackMotion::parse("diagonal"), None);
    assert_eq!(FeedbackMotion::parse("shake"), Some(FeedbackMotion::Shake));
    assert_eq!(
        FeedbackMotion::parse("tunnel"),
        Some(FeedbackMotion::Tunnel)
    );
    assert!(FeedbackParameters::default().amount > 0.9);

    effect.parameters = vec![2.0, -1.0, 5.0];
    effect.normalize();
    assert_eq!(
        effect.feedback_parameters(),
        Some(FeedbackParameters {
            amount: 1.0,
            motion: 0.0,
            direction: FeedbackMotion::RotateRight,
        })
    );
    effect.enabled = false;
    assert_eq!(
        effect.feedback_parameters(),
        None,
        "disabled clears the temporal path"
    );
}

#[test]
fn blur_keeps_legacy_amount_and_exposes_all_five_types() {
    assert_eq!(
        BlurParameters::from_normalized(&[0.6]),
        BlurParameters {
            amount: 0.6,
            blur_type: BlurType::Gaussian,
        }
    );
    for (index, blur_type) in BlurType::ALL.into_iter().enumerate() {
        assert_eq!(BlurType::parse(blur_type.wire_name()), Some(blur_type));
        assert_eq!(BlurType::from_parameter(index as f32), blur_type);
    }
    let mut effect = EffectSlot::blur();
    effect.parameters = vec![2.0, BlurType::Axial.parameter()];
    effect.normalize();
    assert_eq!(effect.parameters, vec![1.0, 4.0]);
}

#[test]
fn beat_move_has_bounded_persisted_controls_and_safe_bypass() {
    let mut effect = EffectSlot::beat_move();
    let defaults = effect.beat_move_parameters().expect("enabled");
    assert_eq!(defaults, BeatMoveParameters::default());
    for direction in BeatMoveDirection::ALL {
        assert_eq!(
            BeatMoveDirection::parse(direction.wire_name()),
            Some(direction)
        );
        assert_eq!(
            BeatMoveDirection::from_parameter(direction.parameter()),
            direction
        );
    }
    effect.parameters = vec![f32::INFINITY, 99.0, -2.0];
    effect.normalize();
    assert_eq!(
        effect.parameters,
        vec![0.15, BeatMoveDirection::Right.parameter(), 0.05]
    );
    effect.enabled = false;
    assert_eq!(effect.beat_move_parameters(), None);
}

#[test]
fn kaleidoscope_has_integer_repetitions_bounded_angle_and_safe_bypass() {
    let mut effect = EffectSlot::kaleidoscope();
    assert_eq!(
        effect.kaleidoscope_parameters(),
        Some(KaleidoscopeParameters::default())
    );

    effect.parameters = vec![1.6, 250.0];
    effect.normalize();
    assert_eq!(effect.parameters, vec![2.0, 180.0]);

    effect.parameters = vec![-4.0, f32::NAN];
    effect.normalize();
    assert_eq!(effect.parameters, vec![0.0, 0.0]);

    effect.enabled = false;
    assert_eq!(effect.kaleidoscope_parameters(), None);
}

#[test]
fn rasterize_has_stable_modes_bounded_dot_size_and_safe_bypass() {
    let mut effect = EffectSlot::rasterize();
    assert_eq!(
        effect.rasterize_parameters(),
        Some(RasterizeParameters::default())
    );
    assert_eq!(
        RasterizeMode::parse("black-and-white"),
        Some(RasterizeMode::BlackAndWhite)
    );
    assert_eq!(RasterizeMode::parse("cmyk"), Some(RasterizeMode::Cmyk));
    assert_eq!(RasterizeMode::parse("rgb"), None);

    effect.parameters = vec![1.0, 100.0];
    effect.normalize();
    assert_eq!(effect.parameters, vec![1.0, 32.0]);
    effect.parameters = vec![0.0, f32::NAN];
    effect.normalize();
    assert_eq!(effect.parameters, vec![0.0, 8.0]);

    effect.enabled = false;
    assert_eq!(effect.rasterize_parameters(), None);
}

#[test]
fn beat_scan_has_typed_edges_bounded_timing_and_no_spawn_count() {
    let mut effect = EffectSlot::beat_scan();
    assert_eq!(
        effect.beat_scan_parameters(),
        Some(BeatScanParameters::default())
    );
    assert_eq!(BeatScanEdge::parse("sharp"), Some(BeatScanEdge::Sharp));
    assert_eq!(BeatScanEdge::parse("soft"), Some(BeatScanEdge::Soft));
    assert_eq!(BeatScanEdge::parse("blurred"), None);
    assert!(
        BeatScanParameters::IDS
            .iter()
            .all(|id| !id.contains("count"))
    );

    effect.parameters = vec![0.5, 1.0, 2.0, 9.0];
    effect.normalize();
    assert_eq!(effect.parameters, vec![0.25, 1.0, 1.0, 3.0]);
    effect.enabled = false;
    assert_eq!(effect.beat_scan_parameters(), None);
}

#[test]
fn beat_scale_turn_has_independent_turn_and_bounded_persisted_controls() {
    let mut effect = EffectSlot::beat_scale_turn();
    assert_eq!(
        effect.beat_scale_turn_parameters(),
        Some(BeatScaleTurnParameters::default())
    );
    effect.parameters = vec![2.0, 1.0, 90.0, -1.0];
    effect.normalize();
    assert_eq!(effect.parameters, vec![1.0, 1.0, 30.0, 0.05]);
    effect.enabled = false;
    assert_eq!(effect.beat_scale_turn_parameters(), None);
}

#[test]
fn beat_grid_wave_has_typed_origins_and_bounded_persisted_controls() {
    let mut effect = EffectSlot::beat_grid_wave();
    assert_eq!(
        effect.beat_grid_wave_parameters(),
        Some(BeatGridWaveParameters::default())
    );
    assert_eq!(
        BeatGridWaveOrigin::parse("centre"),
        Some(BeatGridWaveOrigin::Centre)
    );
    assert_eq!(
        BeatGridWaveOrigin::parse("left"),
        Some(BeatGridWaveOrigin::Left)
    );
    assert_eq!(BeatGridWaveOrigin::parse("corner"), None);
    effect.parameters = vec![100.0, 2.0, 9.0, 4.0, 999.0, 0.0];
    effect.normalize();
    assert_eq!(effect.parameters, vec![64.0, 1.0, 4.0, 4.0, 360.0, 0.1]);
    effect.enabled = false;
    assert_eq!(effect.beat_grid_wave_parameters(), None);
}

#[test]
fn beat_form_flash_has_bounded_persisted_spawn_controls() {
    let mut effect = EffectSlot::beat_form_flash();
    assert_eq!(
        effect.beat_form_flash_parameters(),
        Some(BeatFormFlashParameters::default())
    );
    effect.parameters = vec![9.0, 0.0, 12.0, 2.0];
    effect.normalize();
    assert_eq!(effect.parameters, vec![4.0, 0.1, 4.0, 1.0]);
    effect.enabled = false;
    assert_eq!(effect.beat_form_flash_parameters(), None);
}

#[test]
fn drawn_image_has_bounded_persisted_stylization_controls() {
    let mut effect = EffectSlot::drawn_image();
    assert_eq!(
        effect.drawn_image_parameters(),
        Some(DrawnImageParameters::default())
    );
    effect.parameters = vec![2.0, -1.0];
    effect.normalize();
    assert_eq!(effect.parameters, vec![1.0, 0.0]);
    effect.enabled = false;
    assert_eq!(effect.drawn_image_parameters(), None);
}

#[test]
fn authored_slice_typography_round_trips_without_runtime_glyph_pixels() {
    let mut effect = EffectSlot::beat_slice_grid();
    effect.text = Some("Legacy title".to_owned());
    effect.typography = Some(crate::slice_grid::SliceGridTypography {
        texts: vec!["Opening".to_owned(), "Encore".to_owned()],
        font: "Arial Black".to_owned(),
        text_color: [240, 10, 20],
        background_color: [12, 14, 16],
    });
    effect.text_atlas = Some(std::sync::Arc::new(crate::slice_grid::SliceTextAtlas {
        pixels: vec![255, 0, 128, 255],
        aspects: vec![1.0, 1.5],
    }));
    let stored = serde_json::to_value(&effect).unwrap();
    assert_eq!(stored["effectType"], crate::BEAT_SLICE_GRID_EFFECT);
    assert_eq!(stored["text"], "Legacy title");
    assert_eq!(stored["typography"]["texts"][1], "Encore");
    assert_eq!(
        stored["typography"]["textColor"],
        serde_json::json!([240, 10, 20])
    );
    assert!(stored.get("textAtlas").is_none());
    let restored: EffectSlot = serde_json::from_value(stored).unwrap();
    assert_eq!(restored.typography, effect.typography);
    assert_eq!(restored.text, effect.text);
    assert!(restored.text_atlas.is_none());
}
