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
    let debug = executable
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "target"))
        .map(|target| target.join("debug/bundle/macos/ToskLight Hardware Controls.app"));
    let installed = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Applications/ToskLight Hardware Controls.app"));
    let app = debug
        .filter(|path| path.exists())
        .or_else(|| installed.filter(|path| path.exists()))
        .ok_or("ToskLight Hardware Controls.app is not installed or built")?;
    Command::new("open")
        .arg(app)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
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
