//! Scheduling policy for a newly owned offline Blender, never the desk/helper process.
use std::{
    path::{Path, PathBuf},
    thread,
};
use tokio::process::Command;

pub(super) struct BackgroundPolicy {
    pub threads: usize,
    nice: Option<PathBuf>,
}
impl BackgroundPolicy {
    pub fn discover() -> Self {
        Self {
            threads: thread_limit(thread::available_parallelism().map_or(1, usize::from)),
            nice: find_nice(),
        }
    }
    pub fn command(&self, executable: &Path) -> Command {
        let mut command = if let Some(nice) = &self.nice {
            let mut command = Command::new(nice);
            // nice execs the target in the same PID, preserving the capture parent guard.
            command.args(["-n", "10"]).arg(executable);
            command
        } else {
            Command::new(executable)
        };
        #[cfg(windows)]
        command.creation_flags(windows_flags(0));
        command
            .args(["--background", "--factory-startup", "--threads"])
            .arg(self.threads.to_string())
            .env("TOSKLIGHT_CAPTURE_THREADS", self.threads.to_string());
        command
    }
    pub fn description(&self) -> String {
        let priority = if self.nice.is_some() {
            "lower CPU priority (nice +10)"
        } else if cfg!(windows) {
            "below-normal process priority"
        } else {
            "priority lowering unavailable; CPU thread limit still applies"
        };
        format!(
            "background CPU limit {} threads; {priority}; image quality unchanged",
            self.threads
        )
    }
}

fn thread_limit(available: usize) -> usize {
    (available / 2).clamp(1, 8)
}

#[cfg(unix)]
fn find_nice() -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    ["/usr/bin/nice", "/bin/nice"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| {
            path.metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
}
#[cfg(not(unix))]
fn find_nice() -> Option<PathBuf> {
    None
}

/// Build flags once when constructing the owned child; preserve unrelated launch flags.
/// Never apply this to an existing live/helper Command or process.
#[cfg(any(windows, test))]
fn windows_flags(existing: u32) -> u32 {
    existing | 0x00004000
} // BELOW_NORMAL_PRIORITY_CLASS

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_budget_reserves_half_and_caps_at_eight_even_at_boundaries() {
        for (available, expected) in [
            (0, 1),
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 2),
            (15, 7),
            (16, 8),
            (18, 8),
            (64, 8),
            (usize::MAX, 8),
        ] {
            assert_eq!(thread_limit(available), expected);
        }
    }
    #[test]
    fn wrapped_and_direct_commands_keep_executable_paths_and_threads_as_distinct_arguments() {
        let executable = Path::new("/owned package/Blender");
        for nice in [None, Some(PathBuf::from("/usr/bin/nice"))] {
            let policy = BackgroundPolicy {
                threads: 3,
                nice: nice.clone(),
            };
            let command = policy.command(executable);
            let args = command
                .as_std()
                .get_args()
                .map(|arg| arg.to_str().unwrap())
                .collect::<Vec<_>>();
            let prefix = if nice.is_some() {
                vec!["-n", "10", "/owned package/Blender"]
            } else {
                vec![]
            };
            assert_eq!(&args[..prefix.len()], prefix);
            assert_eq!(
                &args[prefix.len()..],
                ["--background", "--factory-startup", "--threads", "3"]
            );
            assert_eq!(
                command.as_std().get_program(),
                nice.as_deref().unwrap_or(executable).as_os_str()
            );
            assert!(
                command
                    .as_std()
                    .get_envs()
                    .any(|(key, value)| key == "TOSKLIGHT_CAPTURE_THREADS"
                        && value == Some(std::ffi::OsStr::new("3")))
            );
            if nice.is_none() && !cfg!(windows) {
                assert!(
                    policy
                        .description()
                        .contains("priority lowering unavailable")
                );
            }
        }
    }
    #[test]
    fn windows_priority_combination_retains_unrelated_creation_flags() {
        let flags = 0x08000000 | 0x00000200; // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        assert_eq!(windows_flags(flags) & flags, flags);
        assert_ne!(windows_flags(flags) & 0x00004000, 0);
    }
}
