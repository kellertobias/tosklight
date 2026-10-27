//! Logical test dimensions, independent of DPR, scene-resolution policy and HQ export size.
pub(super) fn parse(
    display: Option<&str>,
    size: Option<&str>,
) -> Result<Option<(u32, u32)>, String> {
    let Some(size) = size else {
        return Ok(None);
    };
    if display != Some("internal") {
        return Err(
            "TOSKLIGHT_VIZ_CAPTURE_SIZE requires TOSKLIGHT_VIZ_CAPTURE_DISPLAY=internal".into(),
        );
    }
    let invalid = || {
        "TOSKLIGHT_VIZ_CAPTURE_SIZE needs WIDTHxHEIGHT positive integers, at least640x480 logical pixels".to_owned()
    };
    let (width, height) = size.split_once('x').ok_or_else(invalid)?;
    let dimension = |value: &str| {
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid());
        }
        value.parse::<u32>().map_err(|_| invalid())
    };
    let (width, height) = (dimension(width)?, dimension(height)?);
    if width < 640 || height < 480 {
        return Err(invalid());
    }
    Ok(Some((width, height)))
}

pub(super) fn fit(
    requested: Option<(u32, u32)>,
    display: (f64, f64),
) -> Result<(f64, f64), String> {
    let available = (display.0 - 80.0, display.1 - 100.0);
    if !available.0.is_finite()
        || !available.1.is_finite()
        || available.0 < 640.0
        || available.1 < 480.0
    {
        return Err("Internal display cannot fit the capture".into());
    }
    if let Some((width, height)) = requested {
        if width < 640 || height < 480 {
            return Err("Capture size must be at least640x480 logical pixels".into());
        }
        let requested = (f64::from(width), f64::from(height));
        if requested.0 > available.0 || requested.1 > available.1 {
            return Err("Explicit capture size exceeds the internal display's available bounds; refusing to clamp".into());
        }
        return Ok(requested);
    }
    Ok((1440.0_f64.min(available.0), 900.0_f64.min(available.1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_fhd_logical_size_requires_the_internal_display() {
        assert_eq!(
            parse(Some("internal"), Some("960x540")).unwrap(),
            Some((960, 540))
        );
        assert!(parse(None, Some("960x540")).is_err());
        assert!(parse(Some("external"), Some("960x540")).is_err());
        assert_eq!(parse(None, None).unwrap(), None);
        assert_eq!(parse(Some("internal"), None).unwrap(), None);
    }

    #[test]
    fn noninteger_nonfinite_overflow_and_too_small_dimensions_are_rejected() {
        for value in [
            "",
            "960",
            "960x540x1",
            "960X540",
            "960.0x540",
            "NaNx540",
            "infx540",
            "-960x540",
            "+960x540",
            "960x 540",
            "0x540",
            "639x540",
            "960x479",
            "4294967296x540",
        ] {
            assert!(parse(Some("internal"), Some(value)).is_err(), "{value}");
        }
        assert_eq!(
            parse(Some("internal"), Some("640x480")).unwrap(),
            Some((640, 480))
        );
    }

    #[test]
    fn explicit_dimensions_fit_exactly_or_fail_without_silent_clamping() {
        assert_eq!(
            fit(Some((960, 540)), (1040.0, 640.0)).unwrap(),
            (960.0, 540.0)
        );
        assert!(fit(Some((960, 540)), (1039.0, 640.0)).is_err());
        assert!(fit(Some((960, 540)), (1040.0, 639.0)).is_err());
        for display in [
            (f64::NAN, 1000.0),
            (1500.0, f64::INFINITY),
            (719.0, 1000.0),
            (1500.0, 579.0),
        ] {
            assert!(fit(Some((960, 540)), display).is_err());
        }
    }

    #[test]
    fn omitted_size_preserves_the_existing_default_and_small_display_fit() {
        assert_eq!(fit(None, (1800.0, 1169.0)).unwrap(), (1440.0, 900.0));
        assert_eq!(fit(None, (1200.0, 900.0)).unwrap(), (1120.0, 800.0));
        assert_eq!(fit(None, (720.0, 580.0)).unwrap(), (640.0, 480.0));
    }
}
