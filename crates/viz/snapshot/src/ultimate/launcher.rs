//! Image-only resolution of Homebrew's non-exec wrapper; never parse shell or search versions.
use super::UltimateError;
use std::path::{Path, PathBuf};

pub(super) fn resolve_image_launcher(selected: &Path) -> Result<PathBuf, UltimateError> {
    let selected = selected.canonicalize()?;
    if selected
        .file_name()
        .is_none_or(|name| name != "blender.wrapper.sh")
    {
        return Ok(selected);
    }
    // Follow only the companion belonging to the exact selected wrapper installation. Its app
    // may itself be a symlink (the Homebrew layout); no /Applications or older-version fallback.
    let companion = selected
        .parent()
        .ok_or_else(broken_wrapper)?
        .join("Blender.app/Contents/MacOS/Blender");
    let executable = companion.canonicalize().map_err(|_| broken_wrapper())?;
    let metadata = executable.metadata().map_err(|_| broken_wrapper())?;
    if !metadata.is_file() {
        return Err(broken_wrapper());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(broken_wrapper());
        }
    }
    Ok(executable)
}

fn broken_wrapper() -> UltimateError {
    UltimateError::Failed("The selected Homebrew Blender launcher has no usable sibling Blender.app executable. Choose that Blender application bundle explicitly or choose Ultra; no other version was substituted".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let root = std::env::var_os("LIGHT_TMP_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp")
                });
            let path = root.join(format!(
                "image-launcher-{}-{} with spaces",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn executable(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"not executed in this test").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    #[test]
    fn direct_executable_and_unknown_shell_name_are_not_reinterpreted() {
        let scratch = Scratch::new();
        for name in ["Blender", "custom-launcher.sh"] {
            let path = scratch.0.join(name);
            executable(&path);
            assert_eq!(
                resolve_image_launcher(&path).unwrap(),
                path.canonicalize().unwrap()
            );
        }
    }
    #[test]
    fn missing_companion_refuses_without_using_another_available_application() {
        let scratch = Scratch::new();
        let wrapper = scratch.0.join("selected/5.1.0/blender.wrapper.sh");
        executable(&wrapper);
        executable(
            &scratch
                .0
                .join("other/4.5.0/Blender.app/Contents/MacOS/Blender"),
        );
        let error = resolve_image_launcher(&wrapper).unwrap_err().to_string();
        assert!(error.contains("application bundle explicitly"));
        assert!(error.contains("no other version"));
        let target = wrapper
            .parent()
            .unwrap()
            .join("Blender.app/Contents/MacOS/Blender");
        fs::create_dir_all(target).unwrap();
        assert!(resolve_image_launcher(&wrapper).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_search_path_wrapper_and_app_stay_in_the_selected_version_installation() {
        use std::os::unix::fs::symlink;
        let scratch = Scratch::new();
        let app = |version: &str| scratch.0.join(format!("Apps/Blender {version}.app"));
        for version in ["4.5.3", "5.1.0"] {
            let installation = scratch.0.join("Caskroom/blender").join(version);
            let wrapper = installation.join("blender.wrapper.sh");
            executable(&wrapper);
            // Deliberately not shell-parsed: only filename and sibling app determine resolution.
            fs::write(
                &wrapper,
                b"unparsed shell contents may mention another installation",
            )
            .unwrap();
            executable(&app(version).join("Contents/MacOS/Blender"));
            symlink(app(version), installation.join("Blender.app")).unwrap();
        }
        let path_entry = scratch.0.join("bin/blender");
        fs::create_dir_all(path_entry.parent().unwrap()).unwrap();
        symlink(
            scratch.0.join("Caskroom/blender/5.1.0/blender.wrapper.sh"),
            &path_entry,
        )
        .unwrap();
        assert_eq!(
            resolve_image_launcher(&path_entry).unwrap(),
            app("5.1.0")
                .join("Contents/MacOS/Blender")
                .canonicalize()
                .unwrap()
        );
        let explicitly_selected = scratch.0.join("Caskroom/blender/4.5.3/blender.wrapper.sh");
        assert_eq!(
            resolve_image_launcher(&explicitly_selected).unwrap(),
            app("4.5.3")
                .join("Contents/MacOS/Blender")
                .canonicalize()
                .unwrap()
        );
    }
    #[cfg(unix)]
    #[test]
    fn nonexecutable_companion_is_actionable_and_never_probed() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new();
        let wrapper = scratch.0.join("blender.wrapper.sh");
        executable(&wrapper);
        let target = scratch.0.join("Blender.app/Contents/MacOS/Blender");
        executable(&target);
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            resolve_image_launcher(&wrapper)
                .unwrap_err()
                .to_string()
                .contains("choose Ultra")
        );
    }
}
