//! Explicit verification only; no new operator-facing command or ordinary-desk behavior.
use std::path::Path;

pub(super) const ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_CAPTURE";

pub(super) fn configured(
    value: Option<&str>,
    layout: Option<&str>,
    stage: bool,
    phases: Option<&str>,
    bind: Option<&str>,
    display: Option<&str>,
    output: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(value) = value else { return Ok(None) };
    if value != "live-ultra" {
        return Err(format!("{ENV} must be live-ultra"));
    }
    if layout != Some("dual")
        || !stage
        || phases.is_some()
        || bind != Some("127.0.0.1:5160")
        || display != Some("internal")
        || !output.is_some_and(|path| Path::new(path).is_absolute() && Path::new(path).is_dir())
    {
        return Err("Packaged capture requires fixed dual Stage layout, isolated 127.0.0.1:5160, explicit internal display and an existing absolute image directory; baseline phases are forbidden".into());
    }
    Ok(Some(value.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_desks_are_inert_and_capture_requires_every_boundary() {
        assert_eq!(
            configured(None, None, false, None, None, None, None),
            Ok(None)
        );
        let root = env!("CARGO_MANIFEST_DIR");
        let valid = || {
            configured(
                Some("live-ultra"),
                Some("dual"),
                true,
                None,
                Some("127.0.0.1:5160"),
                Some("internal"),
                Some(root),
            )
        };
        assert_eq!(valid().unwrap().as_deref(), Some("live-ultra"));
        for (layout, stage, phases, bind, display, output) in [
            (
                Some("single"),
                true,
                None,
                Some("127.0.0.1:5160"),
                Some("internal"),
                Some(root),
            ),
            (
                Some("dual"),
                false,
                None,
                Some("127.0.0.1:5160"),
                Some("internal"),
                Some(root),
            ),
            (
                Some("dual"),
                true,
                Some("static-camera-hidden"),
                Some("127.0.0.1:5160"),
                Some("internal"),
                Some(root),
            ),
            (
                Some("dual"),
                true,
                None,
                Some("127.0.0.1:5000"),
                Some("internal"),
                Some(root),
            ),
            (
                Some("dual"),
                true,
                None,
                Some("127.0.0.1:5160"),
                None,
                Some(root),
            ),
            (
                Some("dual"),
                true,
                None,
                Some("127.0.0.1:5160"),
                Some("internal"),
                Some("relative"),
            ),
        ] {
            assert!(
                configured(
                    Some("live-ultra"),
                    layout,
                    stage,
                    phases,
                    bind,
                    display,
                    output
                )
                .is_err()
            );
        }
        assert!(configured(Some("auto"), None, false, None, None, None, None).is_err());
    }
}
