//! TL-596: options of the semantic scenarios that render through the production Live
//! transaction. They are parsed after the established options and change nothing without one
//! of `--semantic` or `--semantic-workload`.

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrackingScenario {
    StaticPoints,
    SmallSubset,
    AllPointsMove,
}

impl TrackingScenario {
    pub const fn key(self) -> &'static str {
        match self {
            Self::StaticPoints => "static-points",
            Self::SmallSubset => "small-subset",
            Self::AllPointsMove => "all-points-move",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticArguments {
    /// `--semantic`: the headless-stress or sustained-show capacity workload with typed lanes.
    pub typed_lanes: bool,
    /// `--semantic-workload DIR`: a TL-564 workload input directory.
    pub workload_dir: Option<String>,
    /// `--tracking-hz N`: Point stream rate; absent or 0 installs no tracking.
    pub tracking_hz: Option<u16>,
    pub tracking_scenario: TrackingScenario,
    /// `--readout-consumers N`: threads reading every published frame.
    pub readout_consumers: usize,
    /// `--slow-consumer-ms N`: how long each consumer holds a frame.
    pub slow_consumer_millis: u64,
    /// `--no-publish` disables the visualization publication step.
    pub publish: bool,
    /// `--static-bases-only`: install the workload's static bases but start none of its
    /// Dynamics, so unchanged targets must reuse their accepted solves.
    pub static_bases_only: bool,
    /// `--rig-height-mm N`: height of the TL-564 workload's fixture grid.
    pub rig_height_mm: i32,
    /// `--digest-ticks N` (TL-639): print per-tick output digests of N unpaced logical ticks
    /// instead of the timed report, to compare two builds frame by frame.
    pub digest_ticks: Option<u64>,
}

impl Default for SemanticArguments {
    fn default() -> Self {
        Self {
            typed_lanes: false,
            workload_dir: None,
            tracking_hz: None,
            tracking_scenario: TrackingScenario::StaticPoints,
            readout_consumers: 0,
            slow_consumer_millis: 0,
            publish: true,
            static_bases_only: false,
            rig_height_mm: 9_000,
            digest_ticks: None,
        }
    }
}

impl SemanticArguments {
    pub fn active(&self) -> bool {
        self.typed_lanes || self.workload_dir.is_some()
    }

    /// Consume `option` when it is a semantic option; `Ok(false)` leaves it to the caller.
    pub fn parse_option(
        &mut self,
        option: &str,
        arguments: &mut impl Iterator<Item = String>,
    ) -> Result<bool, String> {
        let mut value = || {
            arguments
                .next()
                .ok_or_else(|| format!("{option} requires a value"))
        };
        match option {
            "--semantic" => self.typed_lanes = true,
            "--semantic-workload" => {
                let path = value()?;
                if path.trim().is_empty() {
                    return Err("semantic workload directory must not be empty".into());
                }
                self.workload_dir = Some(path);
            }
            "--tracking-hz" => {
                self.tracking_hz = Some(bounded(&value()?, 0, 240, "tracking Hz")? as u16)
            }
            "--tracking-scenario" => {
                self.tracking_scenario = match value()?.as_str() {
                    "static-points" => TrackingScenario::StaticPoints,
                    "small-subset" => TrackingScenario::SmallSubset,
                    "all-points-move" => TrackingScenario::AllPointsMove,
                    other => return Err(format!("invalid tracking scenario: {other}")),
                }
            }
            "--readout-consumers" => {
                self.readout_consumers = bounded(&value()?, 0, 16, "readout consumers")? as usize
            }
            "--slow-consumer-ms" => {
                self.slow_consumer_millis = bounded(&value()?, 0, 1_000, "slow consumer ms")?
            }
            "--no-publish" => self.publish = false,
            "--static-bases-only" => self.static_bases_only = true,
            "--rig-height-mm" => {
                self.rig_height_mm = bounded(&value()?, 0, 30_000, "rig height mm")? as i32
            }
            "--digest-ticks" => {
                self.digest_ticks = Some(bounded(&value()?, 1, 100_000, "digest ticks")?)
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub fn validate(&self, stress_or_sustained: bool) -> Result<(), String> {
        if self.typed_lanes && self.workload_dir.is_some() {
            return Err("--semantic and --semantic-workload are mutually exclusive".into());
        }
        if self.typed_lanes && !stress_or_sustained {
            return Err(
                "--semantic requires --headless-stress-fixtures or --sustained-show".into(),
            );
        }
        if self.workload_dir.is_some() && stress_or_sustained {
            return Err("--semantic-workload replaces the capacity workloads".into());
        }
        if !self.active()
            && (self.tracking_hz.is_some()
                || self.readout_consumers > 0
                || self.slow_consumer_millis > 0
                || !self.publish)
        {
            return Err(
                "tracking, consumer and publication options need a semantic scenario".into(),
            );
        }
        if self.static_bases_only && self.workload_dir.is_none() {
            return Err("--static-bases-only needs a --semantic-workload".into());
        }
        if self.tracking_hz.is_some_and(|rate| rate > 0) && self.workload_dir.is_none() {
            return Err("--tracking-hz needs the Points of a --semantic-workload".into());
        }
        Ok(())
    }

    pub const HELP: &'static str = "\
  --semantic                   Typed semantic lanes through the production Live transaction\n\
  --semantic-workload DIR      Render a TL-564 workload directory (manifest, workload, patch)\n\
  --tracking-hz N              Inject the workload's Point stream at N Hz (0-240)\n\
  --tracking-scenario NAME     static-points|small-subset|all-points-move\n\
  --readout-consumers N        Threads reading each published frame (0-16)\n\
  --slow-consumer-ms N         How long each consumer holds a frame (0-1000)\n\
  --no-publish                 Skip the visualization publication step\n\
  --static-bases-only          Start none of the workload's Dynamics (memo-reuse gates)\n\
  --rig-height-mm N            Height of the workload's fixture grid (default 9000)\n\
  --digest-ticks N             Print per-tick output digests of N unpaced ticks (build equivalence)\n";
}

fn bounded(value: &str, min: u64, max: u64, label: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("invalid {label}: {value}"))?;
    if !(min..=max).contains(&parsed) {
        return Err(format!("{label} must be within {min}-{max}"));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(options: &[&str]) -> Result<SemanticArguments, String> {
        let mut parsed = SemanticArguments::default();
        let mut iter = options.iter().map(ToString::to_string);
        while let Some(option) = iter.next() {
            if !parsed.parse_option(&option, &mut iter)? {
                return Err(format!("unknown {option}"));
            }
        }
        Ok(parsed)
    }

    #[test]
    fn parses_and_validates_the_semantic_matrix() {
        let parsed = parse(&[
            "--semantic-workload",
            "dir",
            "--tracking-hz",
            "120",
            "--tracking-scenario",
            "small-subset",
            "--readout-consumers",
            "4",
            "--slow-consumer-ms",
            "50",
        ])
        .unwrap();
        assert_eq!(parsed.tracking_hz, Some(120));
        assert_eq!(parsed.tracking_scenario, TrackingScenario::SmallSubset);
        assert!(parsed.validate(false).is_ok());
        assert!(parsed.validate(true).is_err());
        assert!(parse(&["--tracking-scenario", "sometimes"]).is_err());
        assert!(parse(&["--readout-consumers", "17"]).is_err());
        let typed = parse(&["--semantic"]).unwrap();
        assert!(typed.validate(true).is_ok());
        assert!(typed.validate(false).is_err());
        assert!(
            parse(&["--semantic", "--tracking-hz", "60"])
                .unwrap()
                .validate(true)
                .is_err()
        );
        assert!(parse(&["--no-publish"]).unwrap().validate(true).is_err());
    }

    #[test]
    fn digest_ticks_are_bounded_and_off_by_default() {
        assert_eq!(parse(&[]).unwrap().digest_ticks, None);
        assert_eq!(
            parse(&["--digest-ticks", "40"]).unwrap().digest_ticks,
            Some(40)
        );
        assert!(parse(&["--digest-ticks", "0"]).is_err());
        assert!(parse(&["--digest-ticks", "100001"]).is_err());
    }
}
