use viz_helper::protocol::FrameTransport;

pub(super) const TRANSPORT_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_TRANSPORT";

/// Ignore benchmark environment on ordinary desks; a configured benchmark must be explicit.
pub(super) fn requested(
    absolute_report: bool,
    layout: Option<&str>,
    value: Option<&str>,
) -> Result<Option<FrameTransport>, String> {
    if !absolute_report {
        return Ok(None);
    }
    if value.is_some() && !matches!(layout, Some("single" | "dual")) {
        return Err(format!("{TRANSPORT_ENV} requires a fixed benchmark layout"));
    }
    match value {
        None | Some("auto") => Ok(None),
        Some("copy") => Ok(Some(FrameTransport::Copy)),
        Some("shared") => Ok(Some(FrameTransport::Shared)),
        Some(_) => Err(format!("{TRANSPORT_ENV} must be copy, shared or auto")),
    }
}

pub(super) fn filter(
    mut available: Vec<FrameTransport>,
    request: Option<FrameTransport>,
) -> Result<Vec<FrameTransport>, String> {
    if let Some(request) = request {
        available.retain(|transport| *transport == request);
        if available.is_empty() {
            return Err(format!(
                "requested benchmark transport {request:?} is unavailable on this desk"
            ));
        }
    }
    Ok(available)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_desks_ignore_override_and_fixed_benchmarks_validate_it() {
        assert_eq!(requested(false, None, Some("invalid")), Ok(None));
        assert!(requested(true, None, Some("copy")).is_err());
        assert!(requested(true, Some("other"), Some("auto")).is_err());
        assert!(requested(true, Some("single"), Some("invalid")).is_err());
        assert_eq!(
            requested(true, Some("dual"), Some("copy")),
            Ok(Some(FrameTransport::Copy))
        );
        assert_eq!(
            requested(true, Some("single"), Some("shared")),
            Ok(Some(FrameTransport::Shared))
        );
    }

    #[test]
    fn auto_preserves_capabilities_and_explicit_request_never_falls_back() {
        let both = vec![FrameTransport::Copy, FrameTransport::Shared];
        assert_eq!(filter(both.clone(), None), Ok(both.clone()));
        assert_eq!(
            filter(both, Some(FrameTransport::Copy)),
            Ok(vec![FrameTransport::Copy])
        );
        assert!(filter(vec![FrameTransport::Copy], Some(FrameTransport::Shared)).is_err());
        assert_eq!(
            filter(vec![FrameTransport::Copy], None),
            Ok(vec![FrameTransport::Copy])
        );
    }

    #[test]
    fn explicit_shared_cannot_negotiate_a_copy_only_helper() {
        let helper = viz_helper::handshake::HelperIdentity {
            protocol: (
                viz_helper::protocol::PROTOCOL_MAJOR,
                viz_helper::protocol::PROTOCOL_MINOR,
            ),
            renderer: "test".into(),
            transports: vec![FrameTransport::Copy],
        };
        let both = vec![FrameTransport::Copy, FrameTransport::Shared];
        assert_eq!(
            helper.embeddable_with(&filter(both.clone(), None).unwrap()),
            Some(FrameTransport::Copy)
        );
        assert_eq!(
            helper.embeddable_with(&filter(both, Some(FrameTransport::Shared)).unwrap()),
            None
        );
    }
}
