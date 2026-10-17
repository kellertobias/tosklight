//! A conservative startup budget for small Linux guests exposing SMT siblings.
//! Topology is guest-reported, not proof of physical-host cores. The first eligible automatic
//! observation is cached for this process; a later different available count uses the old cap.
//! This does not track later affinity changes that retain the same logical CPU count.
use super::MAX_DEFAULT_OUTPUT_WORKERS;

pub(super) fn automatic(available: usize) -> usize {
    let fallback = available.clamp(1, MAX_DEFAULT_OUTPUT_WORKERS);
    #[cfg(target_os = "linux")]
    {
        use std::sync::OnceLock;
        static STARTUP: OnceLock<Option<Budget>> = OnceLock::new();
        if !(2..=4).contains(&available) {
            return fallback;
        }
        let detected = STARTUP.get_or_init(|| {
            detect(available, |path, limit| {
                bounded_text(std::fs::File::open(path).ok()?, limit)
            })
        });
        apply(available, *detected)
    }
    #[cfg(not(target_os = "linux"))]
    {
        fallback
    }
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Budget {
    logical: usize,
    cores: usize,
}
#[cfg(any(target_os = "linux", test))]
fn apply(available: usize, detected: Option<Budget>) -> usize {
    match detected {
        Some(budget)
            if budget.logical == available && budget.cores > 0 && budget.cores < available =>
        {
            budget.cores
        }
        _ => available.clamp(1, MAX_DEFAULT_OUTPUT_WORKERS),
    }
}
#[cfg(any(target_os = "linux", test))]
fn bounded_text(mut reader: impl std::io::Read, limit: usize) -> Option<String> {
    use std::io::Read;
    let mut bytes = Vec::with_capacity(limit + 1);
    reader
        .by_ref()
        .take(u64::try_from(limit).ok()?.checked_add(1)?)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > limit {
        return None;
    }
    String::from_utf8(bytes).ok()
}
#[cfg(any(target_os = "linux", test))]
fn decimal(text: &str, max: usize) -> Option<usize> {
    if text.is_empty()
        || !text.bytes().all(|b| b.is_ascii_digit())
        || text.len() > 1 && text.starts_with('0')
    {
        return None;
    }
    text.parse::<usize>().ok().filter(|n| *n <= max)
}
#[cfg(any(target_os = "linux", test))]
fn cpu_list(text: &str) -> Option<Vec<usize>> {
    let mut cpus = Vec::new();
    for part in text.split(',') {
        let (first, last) = match part.split_once('-') {
            Some((first, last)) => (decimal(first, 4095)?, decimal(last, 4095)?),
            None => {
                let cpu = decimal(part, 4095)?;
                (cpu, cpu)
            }
        };
        if first > last || cpus.last().is_some_and(|previous| first <= *previous) {
            return None;
        }
        cpus.extend(first..=last);
        if cpus.len() > 4096 {
            return None;
        }
    }
    Some(cpus)
}
#[cfg(any(target_os = "linux", test))]
fn allowed(status: &str) -> Option<Vec<usize>> {
    let mut fields = status
        .lines()
        .filter_map(|line| line.strip_prefix("Cpus_allowed_list:"));
    let cpus = cpu_list(fields.next()?.trim_ascii())?;
    if fields.next().is_some() {
        return None;
    }
    Some(cpus)
}
#[cfg(any(target_os = "linux", test))]
fn detect(
    available: usize,
    mut read: impl FnMut(&std::path::Path, usize) -> Option<String>,
) -> Option<Budget> {
    if !(2..=4).contains(&available) {
        return None;
    }
    let process = allowed(&read(std::path::Path::new("/proc/self/status"), 65_536)?)?;
    if process.len() != available {
        return None;
    }
    let thread = allowed(&read(
        std::path::Path::new("/proc/thread-self/status"),
        65_536,
    )?)?;
    if thread != process {
        return None;
    }
    let mut cores = std::collections::BTreeSet::new();
    for cpu in process {
        let directory = format!("/sys/devices/system/cpu/cpu{cpu}/topology");
        let mut id = |name| {
            let value = read(&std::path::Path::new(&directory).join(name), 64)?;
            decimal(value.trim_ascii(), i32::MAX as usize)
        };
        cores.insert((id("physical_package_id")?, id("core_id")?));
    }
    (cores.len() < available).then_some(Budget {
        logical: available,
        cores: cores.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn fixture(mask: &str, topology: &[(usize, usize, usize)]) -> BTreeMap<String, String> {
        let mut files = BTreeMap::from([
            (
                "/proc/self/status".into(),
                format!("Name:\ttest\nCpus_allowed_list:\t{mask}\n"),
            ),
            (
                "/proc/thread-self/status".into(),
                format!("Cpus_allowed_list:\t{mask}\n"),
            ),
        ]);
        for &(cpu, package, core) in topology {
            let dir = format!("/sys/devices/system/cpu/cpu{cpu}/topology");
            files.insert(format!("{dir}/physical_package_id"), format!("{package}\n"));
            files.insert(format!("{dir}/core_id"), format!("{core}\n"));
        }
        files
    }
    fn inspect(available: usize, files: &BTreeMap<String, String>) -> Option<Budget> {
        detect(available, |path, limit| {
            bounded_text(
                std::io::Cursor::new(files.get(path.to_str()?)?.as_bytes()),
                limit,
            )
        })
    }
    #[test]
    fn affinity_list_accepts_disjoint_ranges_and_rejects_ambiguous_or_unbounded_input() {
        assert_eq!(cpu_list("0-1,4,7-8"), Some(vec![0, 1, 4, 7, 8]));
        assert_eq!(cpu_list("4095"), Some(vec![4095]));
        for invalid in [
            "",
            "-1",
            "0,0",
            "0-2,2-3",
            "2-1",
            "1,0",
            "0-4096",
            "4096",
            "1-2-3",
            "1,",
            "+1",
            "01",
            "0, 1",
            "1e3",
            "99999999999999999999999999999",
        ] {
            assert!(cpu_list(invalid).is_none(), "{invalid}");
        }
        assert!(allowed("Cpus_allowed_list: 0-1\nCpus_allowed_list: 0-1\n").is_none());
        assert!(allowed("Name: missing\n").is_none());
    }
    #[test]
    fn small_smt_guest_uses_distinct_package_core_pairs_only() {
        let files = fixture("0-3", &[(0, 0, 0), (1, 0, 1), (2, 0, 0), (3, 0, 1)]);
        let budget = inspect(4, &files);
        assert_eq!(
            budget,
            Some(Budget {
                logical: 4,
                cores: 2
            })
        );
        assert_eq!(apply(4, budget), 2);
        assert_eq!(apply(3, budget), 3); // A later different available count invalidates reuse.
        let packages = fixture("0-3", &[(0, 0, 0), (1, 1, 0), (2, 0, 0), (3, 1, 0)]);
        assert_eq!(inspect(4, &packages), budget); // Core IDs are not globally unique.
        assert!(
            inspect(
                4,
                &fixture("0-3", &[(0, 0, 0), (1, 0, 1), (2, 0, 2), (3, 0, 3)])
            )
            .is_none()
        );
    }
    #[test]
    fn incomplete_or_inconsistent_affinity_and_topology_keep_original_budget() {
        let original = fixture("0-3", &[(0, 0, 0), (1, 0, 1), (2, 0, 0), (3, 0, 1)]);
        assert!(inspect(2, &original).is_none()); // Cgroup availability differs from affinity.
        for (name, value) in [
            ("/proc/thread-self/status", Some("Cpus_allowed_list: 4-7\n")),
            ("/proc/thread-self/status", None),
            ("/sys/devices/system/cpu/cpu2/topology/core_id", None),
            (
                "/sys/devices/system/cpu/cpu2/topology/core_id",
                Some("-1\n"),
            ),
            (
                "/sys/devices/system/cpu/cpu2/topology/physical_package_id",
                Some("2147483648\n"),
            ),
            (
                "/sys/devices/system/cpu/cpu2/topology/core_id",
                Some("2 3\n"),
            ),
        ] {
            let mut files = original.clone();
            match value {
                Some(value) => {
                    files.insert(name.into(), value.into());
                }
                None => {
                    files.remove(name);
                }
            }
            assert!(inspect(4, &files).is_none(), "{name}");
        }
        assert_eq!(apply(4, None), 4);
        assert_eq!(apply(100, None), MAX_DEFAULT_OUTPUT_WORKERS);
        assert_eq!(apply(0, None), 1);
    }
    #[test]
    fn ineligible_counts_skip_io_and_reads_stop_at_the_hard_bound() {
        for count in [0, 1, 5, 4096] {
            assert!(detect(count, |_, _| panic!("ineligible count must skip reads")).is_none());
        }
        let mut input = std::io::Cursor::new(vec![b'0'; 100]);
        assert!(bounded_text(&mut input, 4).is_none());
        assert_eq!(input.position(), 5);
        assert_eq!(
            bounded_text(std::io::Cursor::new(b"1234"), 4).as_deref(),
            Some("1234")
        );
        assert!(bounded_text(std::io::Cursor::new([255]), 4).is_none());
        #[cfg(not(target_os = "linux"))]
        for count in [1, 2, 4, 5, 16] {
            assert_eq!(automatic(count), count.clamp(1, MAX_DEFAULT_OUTPUT_WORKERS));
        }
    }
}
