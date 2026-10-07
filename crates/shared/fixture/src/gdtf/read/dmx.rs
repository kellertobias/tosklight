//! Exact GDTF DMXValue conversion; never normalize native data through floating point.
//!
//! GDTF 1.2 specifies independent source width and byte mirroring (or `/ns` shifting):
//! https://gdtf-development.com/help/developers/gdtf_1_2/file-format-definition/index.html

use crate::{ChannelResolution, ProfileError};

pub(super) fn value(text: &str, target: ChannelResolution) -> Result<u32, ProfileError> {
    let invalid = || ProfileError::Invalid(format!("invalid GDTF DMXValue {text:?}"));
    let (number, width) = text.trim().split_once('/').ok_or_else(invalid)?;
    let shifted = width.ends_with('s');
    let width = width.strip_suffix('s').unwrap_or(width);
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    let source_bytes: usize = width.parse().map_err(|_| invalid())?;
    if !(1..=4).contains(&source_bytes) {
        return Err(invalid());
    }
    let raw: u32 = number.parse().map_err(|_| invalid())?;
    if u64::from(raw) >= (1_u64 << (source_bytes * 8)) {
        return Err(invalid());
    }
    let target_bytes = target.bytes();
    if shifted {
        return Ok(if target_bytes <= source_bytes {
            raw >> ((source_bytes - target_bytes) * 8)
        } else {
            raw << ((target_bytes - source_bytes) * 8)
        });
    }
    // libMVRgdtf converts mirroring by rounding the full-scale ratio. Repeating bytes
    // differs for non-integral width ratios; dropping low bytes differs when narrowing.
    // The largest product plus rounding term still fits u64 for four-byte values.
    let source_max = (1_u64 << (source_bytes * 8)) - 1;
    Ok(((u64::from(raw) * u64::from(target.max_raw()) + source_max / 2) / source_max) as u32)
}

pub(super) fn resolution(bytes: usize) -> Result<ChannelResolution, ProfileError> {
    match bytes {
        1 => Ok(ChannelResolution::U8),
        2 => Ok(ChannelResolution::U16),
        3 => Ok(ChannelResolution::U24),
        4 => Ok(ChannelResolution::U32),
        _ => Err(ProfileError::Invalid(
            "GDTF channels need one to four ordered offsets".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_width_and_conversion_operator_are_preserved() {
        assert_eq!(value("255/1", ChannelResolution::U16).unwrap(), 65535);
        assert_eq!(value("255/1s", ChannelResolution::U16).unwrap(), 65280);
        assert_eq!(value("4660/2", ChannelResolution::U24).unwrap(), 0x123412);
        assert_eq!(
            value("4660/2s", ChannelResolution::U32).unwrap(),
            0x12340000
        );
        assert_eq!(
            value("1193046/3", ChannelResolution::U32).unwrap(),
            0x12345612
        );
        assert_eq!(
            value("305419896/4", ChannelResolution::U16).unwrap(),
            0x1234
        );
        assert_eq!(value("129/2", ChannelResolution::U24).unwrap(), 33025);
        assert_eq!(value("255/2", ChannelResolution::U8).unwrap(), 1);
        assert_eq!(value("255/2s", ChannelResolution::U8).unwrap(), 0);
        assert_eq!(value("4294967295/4", ChannelResolution::U8).unwrap(), 255);
        assert_eq!(value("32768/2", ChannelResolution::U16).unwrap(), 32768);
        assert_eq!(
            value("4294967294/4", ChannelResolution::U32).unwrap(),
            u32::MAX - 1
        );
    }

    #[test]
    fn all_widths_preserve_exact_native_values_and_mirrored_full_output() {
        for bytes in 1..=4 {
            let width = resolution(bytes).unwrap();
            let max = width.max_raw();
            for raw in [0, 1, max / 2, max - 1, max] {
                assert_eq!(value(&format!("{raw}/{bytes}"), width).unwrap(), raw);
            }
            assert_eq!(value("255/1", width).unwrap(), max);
        }
    }

    #[test]
    fn malformed_or_out_of_source_width_values_are_not_clamped() {
        for text in [
            "",
            "255",
            "32768/1",
            "256/1",
            "-1/1",
            "+1/1",
            "1.5/1",
            "1/0",
            "1/5",
            "1/2ss",
            "4294967296/4",
            "1/1/1",
        ] {
            assert!(value(text, ChannelResolution::U32).is_err(), "{text}");
        }
    }
}
