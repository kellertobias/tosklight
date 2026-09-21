use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) fn open() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    open_from(&executable)
}

#[cfg(target_os = "macos")]
fn open_from(executable: &Path) -> Result<(), String> {
    let bundled = bundled_macos_app(executable);
    let installed = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Applications/ToskLight Hardware Controls.app"));
    let app = select_macos_app(bundled, installed, Path::exists)
        .ok_or("ToskLight Hardware Controls.app is not installed or built")?;
    Command::new("open")
        .arg(app)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn bundled_macos_app(executable: &Path) -> Option<PathBuf> {
    executable
        .ancestors()
        .find(|path| {
            path.file_name().is_some_and(|name| name == "macos")
                && path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "bundle")
        })
        .map(|macos| macos.join("ToskLight Hardware Controls.app"))
}

#[cfg(target_os = "macos")]
fn select_macos_app(
    bundled: Option<PathBuf>,
    installed: Option<PathBuf>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    bundled
        .filter(|path| exists(path))
        .or_else(|| installed.filter(|path| exists(path)))
}

#[cfg(not(target_os = "macos"))]
fn open_from(executable: &Path) -> Result<(), String> {
    Command::new(executable.with_file_name(sibling_binary_name()))
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(windows)]
const fn sibling_binary_name() -> &'static str {
    "light-hardware-controls.exe"
}

#[cfg(all(not(windows), not(target_os = "macos")))]
const fn sibling_binary_name() -> &'static str {
    "light-hardware-controls"
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_finds_the_hardware_app_beside_the_current_bundle() {
        let executable = std::path::Path::new(
            "/repo/.artifacts/build/cargo/debug/bundle/macos/ToskLight.app/Contents/MacOS/light-desktop",
        );

        assert_eq!(
            super::bundled_macos_app(executable),
            Some(std::path::PathBuf::from(
                "/repo/.artifacts/build/cargo/debug/bundle/macos/ToskLight Hardware Controls.app"
            ))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_falls_back_to_the_installed_hardware_app() {
        let bundled = std::path::PathBuf::from(
            "/repo/.artifacts/build/cargo/debug/bundle/macos/ToskLight Hardware Controls.app",
        );
        let installed = std::path::PathBuf::from(
            "/Users/operator/Applications/ToskLight Hardware Controls.app",
        );

        assert_eq!(
            super::select_macos_app(
                Some(bundled),
                Some(installed.clone()),
                |candidate| candidate == installed,
            ),
            Some(installed)
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_uses_the_packaged_executable_name() {
        assert_eq!(super::sibling_binary_name(), "light-hardware-controls.exe");
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    #[test]
    fn unix_uses_the_packaged_executable_name() {
        assert_eq!(super::sibling_binary_name(), "light-hardware-controls");
    }
}
