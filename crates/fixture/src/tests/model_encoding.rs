use super::*;

#[test]
fn reads_legacy_metre_locations_and_current_millimetre_locations() {
    let legacy: FixtureLocation =
        serde_json::from_str(r#"{"x":1.25,"y":-0.5,"z":0.00000001}"#).unwrap();
    assert_eq!(
        legacy,
        FixtureLocation {
            x: 1_250,
            y: -500,
            z: 0
        }
    );
    let current: FixtureLocation = serde_json::from_str(r#"{"x":1250,"y":-500,"z":0}"#).unwrap();
    assert_eq!(current, legacy);
    assert_eq!(
        serde_json::to_string(&current).unwrap(),
        r#"{"x":1250,"y":-500,"z":0}"#
    );
}

#[test]
fn encodes_16_bit_msb_first_at_one_based_address() {
    let p = Parameter {
        attribute: AttributeKey("pan".into()),
        components: vec![
            ChannelComponent {
                offset: 0,
                byte_order: ByteOrder::MsbFirst,
            },
            ChannelComponent {
                offset: 1,
                byte_order: ByteOrder::MsbFirst,
            },
        ],
        default: 0.0,
        virtual_dimmer: false,
        metadata: ParameterMetadata::default(),
        capabilities: vec![],
    };
    let mut frame = [0; 512];
    encode_parameter(&mut frame, 1, &p, 0.5).unwrap();
    assert_eq!(&frame[..2], &[128, 0]);
}
#[test]
fn encoder_applies_fixture_inversion_and_transfer_curve() {
    let parameter = Parameter {
        attribute: AttributeKey::intensity(),
        components: vec![ChannelComponent {
            offset: 0,
            byte_order: ByteOrder::MsbFirst,
        }],
        default: 0.0,
        virtual_dimmer: false,
        metadata: ParameterMetadata {
            invert: true,
            curve: DmxCurve::Square,
            ..ParameterMetadata::default()
        },
        capabilities: vec![],
    };
    let mut frame = [0; 512];
    encode_parameter(&mut frame, 1, &parameter, 0.25).unwrap();
    assert_eq!(frame[0], 143);
}

#[test]
fn virtual_dimmer_preserves_color_ratios() {
    let mut channels = [0.8, 0.4, 0.2, 1.0];
    apply_virtual_dimmer(&mut channels, &[0, 1, 2], 0.5);
    assert_eq!(channels, [0.4, 0.2, 0.1, 1.0]);
}
#[test]
fn calibrated_rgb_reconstructs_target_xyz() {
    let calibration = ColorCalibration {
        emitters: vec![
            EmitterCalibration {
                name: "R".into(),
                xyz: Xyz {
                    x: 0.412_456_4,
                    y: 0.212_672_9,
                    z: 0.019_333_9,
                },
                limit: 1.0,
            },
            EmitterCalibration {
                name: "G".into(),
                xyz: Xyz {
                    x: 0.357_576_1,
                    y: 0.715_152_2,
                    z: 0.119_192,
                },
                limit: 1.0,
            },
            EmitterCalibration {
                name: "B".into(),
                xyz: Xyz {
                    x: 0.180_437_5,
                    y: 0.072_175,
                    z: 0.950_304_1,
                },
                limit: 1.0,
            },
        ],
        correction_matrix: identity_matrix(),
    };
    let levels = mix_color(srgb_to_xyz(1.0, 0.0, 0.0), &calibration).unwrap();
    assert!(levels[0] > 0.98);
    assert!(levels[1] < 0.02);
    assert!(levels[2] < 0.02);
}
