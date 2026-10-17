use super::*;

// Frozen original formula: intentionally independent of PreparedLanePosition and lane_position.
fn original(speed: Rational, width: f32, pwm: bool, elapsed: u64, cycle: u64, phase: f32) -> f32 {
    let duration =
        (cycle as f64 / (f64::from(speed.numerator) / f64::from(speed.denominator))).max(1.0);
    let interval_position =
        ((elapsed as f64 / duration) + f64::from(phase) / 360.0).rem_euclid(1.0) as f32;
    let width = if pwm {
        1.0
    } else {
        width.clamp(f32::EPSILON, 1.0)
    };
    ((interval_position - (1.0 - width) * 0.5) / width).clamp(0.0, 1.0)
}
#[test]
fn prepared_position_is_bitwise_equal_to_frozen_formula_at_boundaries() {
    for speed in [
        Rational::ONE,
        Rational {
            numerator: 7,
            denominator: 3,
        },
        Rational {
            numerator: u32::MAX,
            denominator: 1,
        },
        Rational {
            numerator: 1,
            denominator: u32::MAX,
        },
    ] {
        for width in [
            0.0,
            f32::from_bits(f32::EPSILON.to_bits() - 1),
            f32::EPSILON,
            0.3,
            1.0,
            2.0,
        ] {
            for pwm in [false, true] {
                for elapsed in [
                    0,
                    1,
                    999,
                    2401,
                    (1u64 << 53) - 1,
                    (1u64 << 53) + 1,
                    u64::MAX,
                ] {
                    for cycle in [0, 1, 2400, u64::MAX] {
                        let prepared = PreparedLanePosition::new(speed, width, pwm, elapsed, cycle);
                        for phase in [
                            -1080.25,
                            -360.0,
                            -f32::EPSILON,
                            -0.0,
                            0.0,
                            f32::EPSILON,
                            90.0,
                            f32::from_bits(360.0f32.to_bits() - 1),
                            360.0,
                            1080.25,
                            f32::MAX,
                        ] {
                            assert_eq!(
                                prepared.at(phase).to_bits(),
                                original(speed, width, pwm, elapsed, cycle, phase).to_bits(),
                                "speed={speed:?} width={width} pwm={pwm} elapsed={elapsed} cycle={cycle} phase={phase}"
                            );
                        }
                    }
                }
            }
        }
    }
}
