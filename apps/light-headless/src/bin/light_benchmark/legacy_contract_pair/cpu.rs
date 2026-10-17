//! Whole-process CPU boundaries for measured loops, excluding oracle, setup and warmup.
//! Linux TGID stat sums all threads; USER_HZ is provided by the invoking Linux runtime.
use serde_json::{Value, json};
const PROCESS_STAT_MAX_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProcessCpuSnapshot {
    pid: u32,
    start_ticks: u64,
    user_ticks: u64,
    system_ticks: u64,
    thread_count: u64,
}

fn unsigned_stat_field(text: &str) -> Result<u64, &'static str> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("process stat field is not an unsigned decimal");
    }
    text.parse().map_err(|_| "process stat field overflow")
}

fn parse_process_stat(text: &str) -> Result<ProcessCpuSnapshot, &'static str> {
    if text.len() > PROCESS_STAT_MAX_BYTES {
        return Err("process stat byte cap exceeded");
    }
    // comm may contain spaces and parentheses. The rightmost ')' terminates it;
    // numeric tail fields cannot contain ')'. Never split the entire row on spaces.
    let open = text.find('(').ok_or("process stat comm opening absent")?;
    let close = text.rfind(')').ok_or("process stat comm closing absent")?;
    if close <= open {
        return Err("process stat comm delimiters invalid");
    }
    let pid = text[..open]
        .strip_suffix(' ')
        .ok_or("process stat PID delimiter invalid")?;
    let pid = u32::try_from(unsigned_stat_field(pid)?).map_err(|_| "process PID overflow")?;
    if pid == 0 {
        return Err("process PID must be positive");
    }
    let tail = text[close + 1..]
        .strip_prefix(' ')
        .ok_or("process stat tail delimiter invalid")?;
    let fields = tail.split_ascii_whitespace().collect::<Vec<_>>();
    if fields.len() < 20 || fields.len() > 128 {
        return Err("process stat tail field count invalid");
    }
    if fields[0].len() != 1 || !fields[0].as_bytes()[0].is_ascii_alphabetic() {
        return Err("process stat state invalid");
    }
    let thread_count = unsigned_stat_field(fields[17])?;
    if thread_count == 0 {
        return Err("process thread count must be positive");
    }
    Ok(ProcessCpuSnapshot {
        pid,
        start_ticks: unsigned_stat_field(fields[19])?,
        user_ticks: unsigned_stat_field(fields[11])?,
        system_ticks: unsigned_stat_field(fields[12])?,
        thread_count,
    })
}

fn parse_process_user_hz(text: &str) -> Result<u64, &'static str> {
    let hz = unsigned_stat_field(text)?;
    if !(1..=1_000_000).contains(&hz) {
        return Err("USER_HZ outside bounded range");
    }
    Ok(hz)
}

fn process_cpu_delta(
    before: ProcessCpuSnapshot,
    after: ProcessCpuSnapshot,
    user_hz: u64,
) -> Result<Value, &'static str> {
    if before.pid != after.pid || before.start_ticks != after.start_ticks {
        return Err("process CPU identity changed");
    }
    parse_process_user_hz(&user_hz.to_string())?;
    let user = after
        .user_ticks
        .checked_sub(before.user_ticks)
        .ok_or("user ticks regressed")?;
    let system = after
        .system_ticks
        .checked_sub(before.system_ticks)
        .ok_or("system ticks regressed")?;
    let total = user
        .checked_add(system)
        .ok_or("process CPU delta overflow")?;
    Ok(json!({
        "available": 1, "wall_only":0,"user_hz": user_hz,
        "user_ticks_delta": user, "system_ticks_delta": system, "total_ticks_delta": total,
        "user_seconds": user as f64 / user_hz as f64,
        "system_seconds": system as f64 / user_hz as f64,
        "total_seconds": total as f64 / user_hz as f64,
        "thread_count_before": before.thread_count, "thread_count_after": after.thread_count,
        "scope": "whole process/all threads; measured loop and its timing/assertion/vector/read-boundary bookkeeping; excludes oracle, warmup and fingerprints; excludes setup and child-process CPU; tick-quantized, not transaction-only or worker-specific CPU"
    }))
}

#[cfg(target_os = "linux")]
fn linux_process_cpu_snapshot() -> ProcessCpuSnapshot {
    use std::io::Read;
    let file = std::fs::File::open("/proc/self/stat").expect("open own Linux process stat");
    let mut text = String::new();
    file.take((PROCESS_STAT_MAX_BYTES + 1) as u64)
        .read_to_string(&mut text)
        .expect("read bounded UTF-8 process stat");
    let snapshot = parse_process_stat(&text).expect("parse bounded own Linux process stat");
    assert_eq!(snapshot.pid, std::process::id(), "own process PID changed");
    snapshot
}

pub(super) fn process_cpu_begin() -> Option<(ProcessCpuSnapshot, u64)> {
    #[cfg(target_os = "linux")]
    {
        // Obtain this value with getconf CLK_TCK INSIDE the same Linux runtime/container.
        // Missing or malformed values fail the explicit diagnostic CLI; no guessed 100.
        let text = std::env::var("LIGHT_MIXED576_USER_HZ")
            .expect("set LIGHT_MIXED576_USER_HZ from Linux getconf CLK_TCK");
        let hz = parse_process_user_hz(&text).expect("valid Linux USER_HZ receipt");
        Some((linux_process_cpu_snapshot(), hz))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

pub(super) fn process_cpu_finish(before: Option<(ProcessCpuSnapshot, u64)>) -> Value {
    #[cfg(target_os = "linux")]
    {
        let (before, hz) = before.expect("Linux CPU measurement must be available");
        process_cpu_delta(before, linux_process_cpu_snapshot(), hz)
            .expect("stable monotonic own process CPU counters")
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = before;
        json!({ "available": 0, "wall_only":1,"scope": "Linux process CPU unavailable on this host; wall-only timings" })
    }
}

#[test]
fn process_cpu_stat_parser_preserves_spaces_parentheses_and_field_positions() {
    fn row(comm: &str) -> String {
        format!("17 ({comm}) R 1 2 3 4 5 6 7 8 9 10 123 45 9000 8000 0 -1 5 0 777 0 0\n")
    }
    let expected = ProcessCpuSnapshot {
        pid: 17,
        start_ticks: 777,
        user_ticks: 123,
        system_ticks: 45,
        thread_count: 5,
    };
    for comm in [
        "bench",
        "bench pool",
        "bench (pool)",
        "bench) (pool))",
        "a ) R 9 ( b",
    ] {
        assert_eq!(parse_process_stat(&row(comm)), Ok(expected));
    }
    for malformed in [
        "17 bench R 1",
        "17 (bench R 1",
        "17 (bench)R 1",
        "0 (bench) R",
        "17 (bench) RR 1",
    ] {
        assert!(parse_process_stat(malformed).is_err());
    }
    assert!(parse_process_stat(&row("bench").replace("123 45", "-1 45")).is_err());
    assert!(
        parse_process_stat(&row("bench").replace("123 45", "18446744073709551616 45")).is_err()
    );
    assert!(parse_process_stat(&"x".repeat(PROCESS_STAT_MAX_BYTES + 1)).is_err());
}

#[test]
fn process_cpu_delta_excludes_child_fields_and_rejects_identity_or_counter_changes() {
    let before = ProcessCpuSnapshot {
        pid: 17,
        start_ticks: 777,
        user_ticks: 123,
        system_ticks: 45,
        thread_count: 5,
    };
    let after = ProcessCpuSnapshot {
        user_ticks: 153,
        system_ticks: 65,
        ..before
    };
    let observed = process_cpu_delta(before, after, 100).unwrap();
    assert_eq!(observed["user_ticks_delta"], 30);
    assert_eq!(observed["system_ticks_delta"], 20);
    assert_eq!(observed["total_ticks_delta"], 50);
    assert_eq!(observed["total_seconds"], 0.5);
    for changed in [
        ProcessCpuSnapshot { pid: 18, ..after },
        ProcessCpuSnapshot {
            start_ticks: 778,
            ..after
        },
        ProcessCpuSnapshot {
            user_ticks: 122,
            ..after
        },
        ProcessCpuSnapshot {
            system_ticks: 44,
            ..after
        },
    ] {
        assert!(process_cpu_delta(before, changed, 100).is_err());
    }
    assert!(process_cpu_delta(before, after, 0).is_err());
    let overflowing = ProcessCpuSnapshot {
        user_ticks: u64::MAX,
        system_ticks: u64::MAX,
        ..before
    };
    assert!(process_cpu_delta(before, overflowing, 100).is_err());
    assert_eq!(
        process_cpu_delta(before, after, 1000).unwrap()["total_seconds"],
        0.05
    );
    for bad in [
        "",
        "0",
        "-100",
        "+100",
        "100\n",
        "1000001",
        "18446744073709551616",
    ] {
        assert!(parse_process_user_hz(bad).is_err());
    }
    assert_eq!(parse_process_user_hz("100"), Ok(100));
}
