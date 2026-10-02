//! Native integer arithmetic shared by typed Dynamic waveforms, size and transitions. Channel
//! values never pass through f32: only the authored factor does, interpreted as its exact rational.
use super::{IntentError, require};

pub fn scale_native_delta(
    base: u32,
    delta: i64,
    factor: f32,
    minimum: u32,
    maximum: u32,
) -> Result<u32, IntentError> {
    scale_native_delta_wide(base, delta, f64::from(factor), minimum, maximum)
}

/// Compound waveform factors retain their wide precision until integer rounding.
pub fn scale_native_delta_wide(
    base: u32,
    delta: i64,
    factor: f64,
    minimum: u32,
    maximum: u32,
) -> Result<u32, IntentError> {
    require(
        minimum <= maximum && (minimum..=maximum).contains(&base),
        "native base is outside its function",
    )?;
    require(
        delta.unsigned_abs() <= u64::from(u32::MAX),
        "native delta exceeds full channel width",
    )?;
    require(factor.is_finite(), "native factor must be finite")?;
    if delta == 0 || factor == 0.0 {
        return Ok(base);
    }
    let bits = factor.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = (bits & 0x000f_ffff_ffff_ffff) | if exponent == 0 { 0 } else { 1 << 52 };
    let shift = if exponent == 0 { 1074 } else { 1075 - exponent };
    let delta = if factor.is_sign_negative() {
        -i128::from(delta)
    } else {
        i128::from(delta)
    };
    let numerator = delta * i128::from(mantissa);
    let movement = if shift >= 96 {
        0 // numerator is strictly smaller than 2^85.
    } else if shift > 0 {
        let denominator = 1_i128 << shift;
        // Euclidean division gives ties towards the larger raw value in both directions.
        numerator.div_euclid(denominator)
            + i128::from(numerator.rem_euclid(denominator) >= denominator / 2)
    } else if shift <= -32 {
        return Ok(if numerator > 0 { maximum } else { minimum });
    } else {
        numerator << -shift
    };
    Ok((i128::from(base) + movement).clamp(i128::from(minimum), i128::from(maximum)) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_size_preserves_low_bits_and_exact_halfway_rounding() {
        assert_eq!(
            scale_native_delta(
                u32::MAX,
                -i64::from(u32::MAX),
                0.4999999403953552,
                0,
                u32::MAX
            )
            .unwrap(),
            2_147_483_903
        );
        assert_eq!(
            scale_native_delta(2_000_000_001, 1, 1.0, 0, u32::MAX).unwrap(),
            2_000_000_002
        );
        assert_eq!(scale_native_delta(100, -1, 0.5, 0, 255).unwrap(), 100);
        assert_eq!(scale_native_delta(100, 1, 0.5, 0, 255).unwrap(), 101);
        assert_eq!(scale_native_delta(100, 7, -0.5, 0, 255).unwrap(), 97);
        assert_eq!(scale_native_delta(100, 7, 2.0, 0, 255).unwrap(), 114);
    }
    #[test]
    fn native_size_clamps_after_exact_math_without_overflow() {
        assert_eq!(
            scale_native_delta(120, 200, f32::MAX, 100, 140).unwrap(),
            140
        );
        assert_eq!(
            scale_native_delta(120, -200, f32::MAX, 100, 140).unwrap(),
            100
        );
        assert_eq!(
            scale_native_delta(
                u32::MAX,
                -i64::from(u32::MAX),
                f32::from_bits(1),
                0,
                u32::MAX
            )
            .unwrap(),
            u32::MAX
        );
        assert!(scale_native_delta(120, 1, f32::NAN, 100, 140).is_err());
        assert!(scale_native_delta(90, 1, 0.5, 100, 140).is_err());
        assert!(scale_native_delta(120, i64::MAX, 0.5, 100, 140).is_err());
    }
}
