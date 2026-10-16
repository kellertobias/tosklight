//! A best-effort CPU preference owned only by a Linux embedded renderer's lifetime.
#![forbid(unsafe_code)]

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    UnknownInherited,
    Lowering,
    Verification,
}

impl Refusal {
    fn warning(self) -> &'static str {
        match self {
            Self::UnknownInherited => {
                "embedded Stage CPU priority: inherited nice value unavailable; continuing unchanged; check launcher scheduling restrictions"
            }
            Self::Lowering => {
                "embedded Stage CPU priority: OS refused lower CPU priority; continuing unchanged; check launcher scheduling restrictions"
            }
            Self::Verification => {
                "embedded Stage CPU priority: lowered nice value unverified; continuing rendering; check launcher scheduling restrictions"
            }
        }
    }
}

/// The caller supplies all helper startup, so this boundary precedes channel and graphics threads.
/// Only the `--embed` branch enters here; it never changes the launching desk or headless server.
pub(crate) fn run(start_helper: impl FnOnce() -> i32) -> i32 {
    run_with_policy(start_helper, lower_priority, |warning| {
        write_warning(&mut std::io::stderr().lock(), warning)
    })
}

fn write_warning(output: &mut impl std::io::Write, warning: &str) {
    // An optional scheduling hint must not stop drawing if its private stderr is unwritable.
    let _ = writeln!(output, "{warning}");
}

fn run_with_policy(
    start_helper: impl FnOnce() -> i32,
    policy: impl FnOnce() -> Result<(), Refusal>,
    warn: impl FnOnce(&str),
) -> i32 {
    if let Err(refusal) = policy() {
        warn(refusal.warning());
    }
    start_helper()
}

#[cfg(any(target_os = "linux", test))]
fn lower_with(
    inherited: impl FnOnce() -> Result<i32, ()>,
    add_nice: impl FnOnce(i32) -> Result<(), ()>,
    effective: impl FnOnce() -> Result<i32, ()>,
) -> Result<(), Refusal> {
    let before = inherited().map_err(|_| Refusal::UnknownInherited)?;
    if !(-20..=19).contains(&before) {
        return Err(Refusal::UnknownInherited);
    }
    // A positive increment can only lower priority; never set an absolute nice value that
    // could raise an already-demoted child. Each replacement applies this once after exec.
    add_nice(5).map_err(|_| Refusal::Lowering)?;
    let actual = effective().map_err(|_| Refusal::Verification)?;
    if actual != (before + 5).min(19) {
        return Err(Refusal::Verification);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn lower_priority() -> Result<(), Refusal> {
    lower_with(
        || rustix::process::getpriority_process(None).map_err(|_| ()),
        |increment| rustix::process::nice(increment).map(|_| ()).map_err(|_| ()),
        || rustix::process::getpriority_process(None).map_err(|_| ()),
    )
}

#[cfg(not(target_os = "linux"))]
fn lower_priority() -> Result<(), Refusal> {
    Ok(())
}

#[cfg(test)]
#[path = "embedded_priority/tests.rs"]
mod tests;
