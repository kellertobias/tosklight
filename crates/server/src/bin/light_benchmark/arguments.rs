use light_output::Protocol;
use serde::Serialize;

const DEFAULT_SECONDS: u64 = 5;
const DEFAULT_WARMUP_SECONDS: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkProfile {
    HardFloor,
    Target,
    #[serde(rename = "low_power_4")]
    LowPower4,
    #[serde(rename = "low_power_8")]
    LowPower8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Expectation {
    RequiredFloor,
    TargetGoal,
    LowPowerGoal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileConfig {
    pub profile: BenchmarkProfile,
    pub expectation: Expectation,
    pub universes: u16,
    pub rate_hz: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolSelection {
    #[serde(rename = "artnet")]
    ArtNet,
    Sacn,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    EncodeOnly,
    Loopback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arguments {
    pub profiles: Vec<BenchmarkProfile>,
    pub protocol: ProtocolSelection,
    pub transport: Transport,
    pub seconds: u64,
    pub warmup_seconds: u64,
    pub hardware_label: Option<String>,
}

pub enum ParseOutcome {
    Run(Arguments),
    Help,
}

impl BenchmarkProfile {
    pub const ALL: [Self; 4] = [
        Self::HardFloor,
        Self::Target,
        Self::LowPower4,
        Self::LowPower8,
    ];

    pub const fn config(self) -> ProfileConfig {
        match self {
            Self::HardFloor => ProfileConfig {
                profile: self,
                expectation: Expectation::RequiredFloor,
                universes: 32,
                rate_hz: 100,
            },
            Self::Target => ProfileConfig {
                profile: self,
                expectation: Expectation::TargetGoal,
                universes: 64,
                rate_hz: 120,
            },
            Self::LowPower4 => ProfileConfig {
                profile: self,
                expectation: Expectation::LowPowerGoal,
                universes: 4,
                rate_hz: 40,
            },
            Self::LowPower8 => ProfileConfig {
                profile: self,
                expectation: Expectation::LowPowerGoal,
                universes: 8,
                rate_hz: 40,
            },
        }
    }
}

impl ProtocolSelection {
    pub fn protocols(self) -> &'static [Protocol] {
        match self {
            Self::ArtNet => &[Protocol::ArtNet],
            Self::Sacn => &[Protocol::Sacn],
            Self::Both => &[Protocol::ArtNet, Protocol::Sacn],
        }
    }
}

impl Default for Arguments {
    fn default() -> Self {
        Self {
            profiles: BenchmarkProfile::ALL.to_vec(),
            protocol: ProtocolSelection::ArtNet,
            transport: Transport::EncodeOnly,
            seconds: DEFAULT_SECONDS,
            warmup_seconds: DEFAULT_WARMUP_SECONDS,
            hardware_label: None,
        }
    }
}

impl Arguments {
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<ParseOutcome, String> {
        let mut parsed = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--profile" => {
                    parsed.profiles = parse_profiles(&required_value(&mut arguments, &argument)?)?
                }
                "--protocol" => {
                    parsed.protocol = parse_protocol(&required_value(&mut arguments, &argument)?)?
                }
                "--transport" => {
                    parsed.transport = parse_transport(&required_value(&mut arguments, &argument)?)?
                }
                "--seconds" => {
                    parsed.seconds = parse_bounded_u64(
                        &required_value(&mut arguments, &argument)?,
                        1,
                        300,
                        "seconds",
                    )?
                }
                "--warmup-seconds" => {
                    parsed.warmup_seconds = parse_bounded_u64(
                        &required_value(&mut arguments, &argument)?,
                        0,
                        60,
                        "warmup seconds",
                    )?
                }
                "--hardware-label" => {
                    let label = required_value(&mut arguments, &argument)?;
                    if label.trim().is_empty() {
                        return Err("hardware label must not be empty".into());
                    }
                    parsed.hardware_label = Some(label);
                }
                "--help" | "-h" => return Ok(ParseOutcome::Help),
                _ => return Err(format!("unknown argument: {argument}")),
            }
        }
        Ok(ParseOutcome::Run(parsed))
    }

    pub const fn help() -> &'static str {
        "Usage: light-benchmark [OPTIONS]\n\
         \n\
         Release-only render-through-protocol-encoding benchmark. JSON is written to stdout.\n\
         \n\
         Options:\n\
           --profile all|hard-floor|target|low-power-4|low-power-8\n\
           --protocol artnet|sacn|both\n\
           --transport encode-only|loopback\n\
           --seconds N                 Measurement duration, 1-300 (default: 5)\n\
           --warmup-seconds N          Unpaced warmup duration, 0-60 (default: 1)\n\
           --hardware-label TEXT       Reference-machine description included in JSON\n\
           -h, --help\n"
    }
}

fn required_value(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_profiles(value: &str) -> Result<Vec<BenchmarkProfile>, String> {
    Ok(match value {
        "all" => BenchmarkProfile::ALL.to_vec(),
        "hard-floor" => vec![BenchmarkProfile::HardFloor],
        "target" => vec![BenchmarkProfile::Target],
        "low-power-4" => vec![BenchmarkProfile::LowPower4],
        "low-power-8" => vec![BenchmarkProfile::LowPower8],
        _ => return Err(format!("invalid benchmark profile: {value}")),
    })
}

fn parse_protocol(value: &str) -> Result<ProtocolSelection, String> {
    match value {
        "artnet" => Ok(ProtocolSelection::ArtNet),
        "sacn" => Ok(ProtocolSelection::Sacn),
        "both" => Ok(ProtocolSelection::Both),
        _ => Err(format!("invalid protocol selection: {value}")),
    }
}

fn parse_transport(value: &str) -> Result<Transport, String> {
    match value {
        "encode-only" => Ok(Transport::EncodeOnly),
        "loopback" => Ok(Transport::Loopback),
        _ => Err(format!("invalid transport: {value}")),
    }
}

fn parse_bounded_u64(value: &str, min: u64, max: u64, label: &str) -> Result<u64, String> {
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

    fn parsed(arguments: &[&str]) -> Arguments {
        match Arguments::parse(arguments.iter().map(ToString::to_string)).unwrap() {
            ParseOutcome::Run(arguments) => arguments,
            ParseOutcome::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn parses_an_explicit_release_run() {
        let arguments = parsed(&[
            "--profile",
            "hard-floor",
            "--protocol",
            "both",
            "--transport",
            "loopback",
            "--seconds",
            "7",
            "--warmup-seconds",
            "2",
            "--hardware-label",
            "Test host",
        ]);
        assert_eq!(arguments.profiles, vec![BenchmarkProfile::HardFloor]);
        assert_eq!(arguments.protocol, ProtocolSelection::Both);
        assert_eq!(arguments.transport, Transport::Loopback);
        assert_eq!(arguments.seconds, 7);
        assert_eq!(arguments.warmup_seconds, 2);
        assert_eq!(arguments.hardware_label.as_deref(), Some("Test host"));
    }

    #[test]
    fn rejects_removed_codec_only_arguments_and_invalid_bounds() {
        assert!(Arguments::parse(["--universes".into(), "64".into()]).is_err());
        assert!(Arguments::parse(["--seconds".into(), "0".into()]).is_err());
        assert!(Arguments::parse(["--protocol".into(), "udp".into()]).is_err());
    }

    #[test]
    fn named_profiles_encode_the_acceptance_matrix() {
        assert_eq!(BenchmarkProfile::HardFloor.config().universes, 32);
        assert_eq!(BenchmarkProfile::HardFloor.config().rate_hz, 100);
        assert_eq!(BenchmarkProfile::Target.config().universes, 64);
        assert_eq!(BenchmarkProfile::Target.config().rate_hz, 120);
        assert_eq!(BenchmarkProfile::LowPower4.config().rate_hz, 40);
        assert_eq!(BenchmarkProfile::LowPower8.config().universes, 8);
        assert_eq!(
            BenchmarkProfile::HardFloor.config().expectation,
            Expectation::RequiredFloor
        );
    }
}
