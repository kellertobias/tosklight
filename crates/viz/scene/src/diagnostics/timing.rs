//! Diagnostic admission only; this never schedules a picture or blocks GPU completion.
use std::time::{Duration, Instant};

pub const CADENCE_ENV: &str = "LIGHT_STAGE_GPU_TIMING_CADENCE";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TimingCadence {
    OneHz,
    Full,
}

impl TimingCadence {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "1hz" => Ok(Self::OneHz),
            "full" => Ok(Self::Full),
            _ => Err("expected full or 1hz"),
        }
    }
}

/// Cumulative owner-lifetime coverage, not a count of displayed frames or GPU utilization.
#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingCoverage {
    pub diagnostic_cadence: TimingCadence,
    pub benchmark_override: bool,
    pub elapsed_millis: u64,
    pub frames_considered: u64,
    pub frames_admitted: u64,
    pub busy_skips: u64,
    pub cadence_skips: u64,
    pub completed_readbacks: u64,
    pub valid_samples: u64,
}

pub struct TimingAdmission {
    clock: Instant,
    cadence: TimingCadence,
    last_diagnostic: Option<Duration>,
    coverage: TimingCoverage,
}

impl TimingAdmission {
    pub fn from_environment(owner: &str) -> Result<Self, String> {
        let benchmark = std::env::var_os("LIGHT_STAGE_PACKAGED_BENCH_REPORT")
            .is_some_and(|path| std::path::Path::new(&path).is_absolute());
        let value = std::env::var_os(CADENCE_ENV);
        let requested = configured_cadence(benchmark, value.as_deref())?;
        let cadence = requested.unwrap_or(TimingCadence::OneHz);
        let admission = Self::new(cadence, requested.is_some());
        if benchmark {
            eprintln!(
                "Stage GPU timing policy owner={owner} diagnostic={cadence:?} explicit={} otherQualities=Full",
                requested.is_some()
            );
        }
        Ok(admission)
    }

    pub fn new(cadence: TimingCadence, benchmark_override: bool) -> Self {
        Self {
            clock: Instant::now(),
            cadence,
            last_diagnostic: None,
            coverage: TimingCoverage {
                diagnostic_cadence: cadence,
                benchmark_override,
                elapsed_millis: 0,
                frames_considered: 0,
                frames_admitted: 0,
                busy_skips: 0,
                cadence_skips: 0,
                completed_readbacks: 0,
                valid_samples: 0,
            },
        }
    }

    /// Call exactly once before encoding a frame, then retain the returned decision for all
    /// its passes. Busy frames never consume the next due opportunity. Non-diagnostic views
    /// retain their previous every-available-frame policy (including Ultra adaptation).
    pub fn begin_frame(&mut self, diagnostic: bool, busy: bool) -> bool {
        self.begin_at(self.clock.elapsed(), diagnostic, busy)
    }

    fn begin_at(&mut self, now: Duration, diagnostic: bool, busy: bool) -> bool {
        self.coverage.frames_considered = self.coverage.frames_considered.saturating_add(1);
        if busy {
            self.coverage.busy_skips = self.coverage.busy_skips.saturating_add(1);
            return false;
        }
        if diagnostic && self.cadence == TimingCadence::OneHz {
            if self
                .last_diagnostic
                .is_some_and(|last| now.saturating_sub(last) < Duration::from_secs(1))
            {
                self.coverage.cadence_skips = self.coverage.cadence_skips.saturating_add(1);
                return false;
            }
            self.last_diagnostic = Some(now);
        }
        self.coverage.frames_admitted = self.coverage.frames_admitted.saturating_add(1);
        true
    }

    pub fn coverage(&self, completed_readbacks: u64, valid_samples: u64) -> TimingCoverage {
        TimingCoverage {
            elapsed_millis: self.clock.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            completed_readbacks,
            valid_samples,
            ..self.coverage
        }
    }
}

fn configured_cadence(
    benchmark: bool,
    value: Option<&std::ffi::OsStr>,
) -> Result<Option<TimingCadence>, String> {
    if !benchmark {
        return Ok(None);
    }
    value
        .map(|value| {
            let value = value
                .to_str()
                .ok_or_else(|| format!("{CADENCE_ENV} must contain Unicode full or 1hz"))?;
            TimingCadence::parse(value).map_err(|reason| format!("invalid {CADENCE_ENV}: {reason}"))
        })
        .transpose()
}

#[cfg(test)]
mod tests;
