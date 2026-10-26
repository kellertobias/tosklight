use super::*;
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let base = std::env::var_os("LIGHT_TMP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp")
            });
        let path = base.join(format!("architect-private-launch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn files(&self) -> (String, String, String) {
        let data = self.0.join("data");
        std::fs::create_dir(&data).unwrap();
        let show = data.join("Owned.show");
        let preferences = data.join("renderer.conf");
        std::fs::write(&show, b"not a valid show yet").unwrap();
        std::fs::write(&preferences, b"quality ultra\n").unwrap();
        (
            data.to_str().unwrap().into(),
            show.to_str().unwrap().into(),
            preferences.to_str().unwrap().into(),
        )
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn capture_launch_is_explicit_complete_and_preferences_alone_preserves_ordinary_startup() {
    assert!(parse(None, None, None, None).unwrap().is_none());
    assert!(
        parse(None, None, None, Some("/operator/preferences.conf"))
            .unwrap()
            .is_none()
    );
    let scratch = Scratch::new();
    let (data, show, prefs) = scratch.files();
    let config = parse(Some(&data), Some(&show), Some("internal"), Some(&prefs))
        .unwrap()
        .unwrap();
    assert_eq!(config.data, PathBuf::from(&data));
    assert_eq!(config.show, PathBuf::from(&show));
    for (d, s, m, p) in [
        (
            None,
            Some(show.as_str()),
            Some("internal"),
            Some(prefs.as_str()),
        ),
        (
            Some(data.as_str()),
            None,
            Some("internal"),
            Some(prefs.as_str()),
        ),
        (
            Some(data.as_str()),
            Some(show.as_str()),
            None,
            Some(prefs.as_str()),
        ),
        (
            Some(data.as_str()),
            Some(show.as_str()),
            Some("internal"),
            None,
        ),
        (
            Some(data.as_str()),
            Some(show.as_str()),
            Some("external"),
            Some(prefs.as_str()),
        ),
    ] {
        assert!(parse(d, s, m, p).is_err());
    }
}
#[test]
fn capture_launch_refuses_relative_missing_and_nonowned_files() {
    let scratch = Scratch::new();
    let (data, show, prefs) = scratch.files();
    assert!(
        parse(
            Some("relative"),
            Some(&show),
            Some("internal"),
            Some(&prefs)
        )
        .is_err()
    );
    let outside = scratch.0.join("Other.show");
    std::fs::write(&outside, b"untouched").unwrap();
    assert!(
        parse(
            Some(&data),
            Some(outside.to_str().unwrap()),
            Some("internal"),
            Some(&prefs)
        )
        .is_err()
    );
    assert!(
        parse(
            Some(&data),
            Some(&show),
            Some("internal"),
            Some(outside.to_str().unwrap())
        )
        .is_err()
    );
    assert!(parse(Some(&data), Some(&show), Some("internal"), Some(&show)).is_err());
    std::fs::remove_file(&show).unwrap();
    assert!(parse(Some(&data), Some(&show), Some("internal"), Some(&prefs)).is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), b"untouched");
}
#[test]
fn capture_launch_opens_only_explicit_private_show_and_records_recent_inside_private_data() {
    let scratch = Scratch::new();
    let (data, show, prefs) = scratch.files();
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/demo.show"),
        &show,
    )
    .unwrap();
    let config = parse(Some(&data), Some(&show), Some("internal"), Some(&prefs))
        .unwrap()
        .unwrap();
    let session = crate::session::Session::default();
    open_private(&session, &config).unwrap();
    assert!(session.document_name().is_some());
    let recent = crate::recent::RecentShow::at(config.data.join("recent-show"));
    assert_eq!(recent.read(), Some(config.show.clone()));
    assert!(
        session.sync_engine().is_none(),
        "explicit validation does not auto-reopen desk sync"
    );
}
#[test]
fn invalid_private_show_is_refused_without_loading_a_default_document() {
    let scratch = Scratch::new();
    let (data, show, prefs) = scratch.files();
    let config = parse(Some(&data), Some(&show), Some("internal"), Some(&prefs))
        .unwrap()
        .unwrap();
    let session = crate::session::Session::default();
    assert!(open_private(&session, &config).is_err());
    assert!(session.document_name().is_none());
    assert!(!config.data.join("recent-show").exists());
}

#[test]
fn capture_readiness_binds_exact_owned_identity_and_never_claims_frontend_or_display_frames() {
    let scratch = Scratch::new();
    let (data, show, prefs) = scratch.files();
    let config = parse(Some(&data), Some(&show), Some("internal"), Some(&prefs))
        .unwrap()
        .unwrap();
    write_readiness(&config, false).unwrap();
    let initial: serde_json::Value =
        serde_json::from_slice(&std::fs::read(config.data.join("capture-window.json")).unwrap())
            .unwrap();
    assert_eq!(initial["windowPlacedAndShown"], false);
    assert_eq!(initial["pid"], std::process::id());
    assert_eq!(initial["show"], config.show.to_str().unwrap());
    assert_eq!(initial["frontendReadinessClaimed"], false);
    write_readiness(&config, true).unwrap();
    let shown: serde_json::Value =
        serde_json::from_slice(&std::fs::read(config.data.join("capture-window.json")).unwrap())
            .unwrap();
    assert_eq!(shown["windowPlacedAndShown"], true);
    assert_eq!(shown["frontendReadinessClaimed"], false);
}
