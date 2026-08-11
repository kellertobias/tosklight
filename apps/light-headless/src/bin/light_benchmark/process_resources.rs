use crate::light_benchmark::report::{MeasurementResourceReport, ProcessResourceReport};
#[cfg(target_os = "linux")]
use std::time::Instant;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct MeasurementSampler {
    stop: Sender<()>,
    worker: Option<thread::JoinHandle<MeasurementResourceReport>>,
}

impl MeasurementSampler {
    pub(super) fn start() -> Self {
        let (stop, receiver) = mpsc::channel();
        let ticks_per_second = clock_ticks_per_second();
        let worker = thread::spawn(move || sample_measurement(receiver, ticks_per_second));
        Self {
            stop,
            worker: Some(worker),
        }
    }

    pub(super) fn finish(mut self) -> MeasurementResourceReport {
        let _ = self.stop.send(());
        self.worker
            .take()
            .and_then(|worker| worker.join().ok())
            .unwrap_or_else(|| unavailable_measurement("resource sampler thread failed".into()))
    }
}

#[cfg(target_os = "linux")]
fn sample_measurement(
    stop: Receiver<()>,
    ticks_per_second: Option<u64>,
) -> MeasurementResourceReport {
    let Some(ticks_per_second) = ticks_per_second else {
        return unavailable_measurement("getconf CLK_TCK returned no clock rate".into());
    };
    let mut previous = match linux_sample() {
        Ok(sample) => sample,
        Err(reason) => return unavailable_measurement(reason),
    };
    let mut cpu_total = 0.0;
    let mut cpu_max = 0.0_f64;
    let mut cpu_samples = 0_u64;
    let mut peak_resident = previous.resident_bytes;
    loop {
        match stop.recv_timeout(SAMPLE_INTERVAL) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let current = match linux_sample() {
            Ok(sample) => sample,
            Err(reason) => return unavailable_measurement(reason),
        };
        let elapsed = current.at.duration_since(previous.at).as_secs_f64();
        if elapsed > 0.0 {
            let cpu = current.cpu_ticks.saturating_sub(previous.cpu_ticks) as f64
                / ticks_per_second as f64
                / elapsed
                * 100.0;
            cpu_total += cpu;
            cpu_max = cpu_max.max(cpu);
            cpu_samples += 1;
        }
        peak_resident = peak_resident.max(current.resident_bytes);
        previous = current;
    }
    MeasurementResourceReport {
        application_cpu_average_percent: (cpu_samples > 0)
            .then_some(cpu_total / cpu_samples as f64),
        application_cpu_max_percent: (cpu_samples > 0).then_some(cpu_max),
        application_peak_resident_bytes: Some(peak_resident),
        samples: cpu_samples,
        sample_interval_milliseconds: SAMPLE_INTERVAL.as_millis() as u64,
        measurement: "Light benchmark process only, sampled from /proc during the timed window",
        unavailable_reason: None,
    }
}

#[cfg(target_os = "linux")]
struct LinuxSample {
    at: Instant,
    cpu_ticks: u64,
    resident_bytes: u64,
}

#[cfg(target_os = "linux")]
fn linux_sample() -> Result<LinuxSample, String> {
    let stat = std::fs::read_to_string("/proc/self/stat")
        .map_err(|error| format!("read /proc/self/stat: {error}"))?;
    let fields = stat
        .rsplit_once(") ")
        .ok_or_else(|| "parse /proc/self/stat process name".to_owned())?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let user_ticks = fields
        .get(11)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| "parse /proc/self/stat user ticks".to_owned())?;
    let system_ticks = fields
        .get(12)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| "parse /proc/self/stat system ticks".to_owned())?;
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|error| format!("read /proc/self/status: {error}"))?;
    Ok(LinuxSample {
        at: Instant::now(),
        cpu_ticks: user_ticks + system_ticks,
        resident_bytes: status_kib(&status, "VmRSS:").unwrap_or(0),
    })
}

#[cfg(not(target_os = "linux"))]
fn sample_measurement(
    _stop: Receiver<()>,
    _ticks_per_second: Option<u64>,
) -> MeasurementResourceReport {
    unavailable_measurement("timed CPU and RAM sampling is currently available on Linux".into())
}

#[cfg(target_os = "linux")]
fn clock_ticks_per_second() -> Option<u64> {
    let output = std::process::Command::new("getconf")
        .arg("CLK_TCK")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().parse().ok())
        .flatten()
}

#[cfg(not(target_os = "linux"))]
const fn clock_ticks_per_second() -> Option<u64> {
    None
}

fn unavailable_measurement(reason: String) -> MeasurementResourceReport {
    MeasurementResourceReport {
        application_cpu_average_percent: None,
        application_cpu_max_percent: None,
        application_peak_resident_bytes: None,
        samples: 0,
        sample_interval_milliseconds: SAMPLE_INTERVAL.as_millis() as u64,
        measurement: "unavailable",
        unavailable_reason: Some(reason),
    }
}

pub(super) fn capture() -> ProcessResourceReport {
    #[cfg(target_os = "linux")]
    {
        return linux_status();
    }
    #[cfg(target_os = "macos")]
    {
        return macos_ps();
    }
    #[allow(unreachable_code)]
    ProcessResourceReport {
        resident_bytes: None,
        peak_resident_bytes: None,
        measurement: "unavailable",
        unavailable_reason: Some("process RSS sampling is not implemented on this platform".into()),
    }
}

#[cfg(target_os = "linux")]
fn linux_status() -> ProcessResourceReport {
    let status = match std::fs::read_to_string("/proc/self/status") {
        Ok(status) => status,
        Err(error) => return unavailable(format!("read /proc/self/status: {error}")),
    };
    ProcessResourceReport {
        resident_bytes: status_kib(&status, "VmRSS:"),
        peak_resident_bytes: status_kib(&status, "VmHWM:"),
        measurement: "post-run /proc/self/status; outside timed pipeline",
        unavailable_reason: None,
    }
}

#[cfg(target_os = "linux")]
fn status_kib(status: &str, key: &str) -> Option<u64> {
    status
        .lines()
        .find(|line| line.starts_with(key))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u64>().ok())
        .map(|kib| kib * 1_024)
}

#[cfg(target_os = "macos")]
fn macos_ps() -> ProcessResourceReport {
    let pid = std::process::id().to_string();
    let output = match std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
    {
        Ok(output) if output.status.success() => output,
        Ok(output) => return unavailable(format!("ps exited with {}", output.status)),
        Err(error) => return unavailable(format!("run ps: {error}")),
    };
    let resident_bytes = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .ok()
        .map(|kib| kib * 1_024);
    ProcessResourceReport {
        resident_bytes,
        peak_resident_bytes: None,
        measurement: "post-run ps resident set; outside timed pipeline",
        unavailable_reason: resident_bytes
            .is_none()
            .then(|| "ps returned no RSS value".into()),
    }
}

fn unavailable(reason: String) -> ProcessResourceReport {
    ProcessResourceReport {
        resident_bytes: None,
        peak_resident_bytes: None,
        measurement: "unavailable",
        unavailable_reason: Some(reason),
    }
}
