use anyhow::Context;
use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

pub(super) const HELP: &str = "light-server [--data-dir PATH] [--fixture-package-dir PATH] [--bind ADDRESS] [--test-bench] [--osc-bind ADDRESS] [--output-bind-ip ADDRESS]";

#[derive(Debug, Eq, PartialEq)]
pub(super) enum StartupAction {
    Run(StartupOptions),
    ShowHelp,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct StartupOptions {
    pub(super) data_dir: PathBuf,
    pub(super) fixture_package_dir: Option<PathBuf>,
    pub(super) bind: SocketAddr,
    pub(super) test_bench: bool,
    pub(super) osc_bind_override: Option<SocketAddr>,
    pub(super) output_bind_override: Option<IpAddr>,
}

pub(super) fn from_process() -> anyhow::Result<StartupAction> {
    parse(
        env::args().skip(1),
        env::var_os("LIGHT_DATA_DIR").map(PathBuf::from),
    )
}

fn parse(
    args: impl IntoIterator<Item = String>,
    environment_data_dir: Option<PathBuf>,
) -> anyhow::Result<StartupAction> {
    let mut options = StartupOptions::with_data_dir(environment_data_dir);
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        if apply_argument(&mut options, &argument, &mut args)? {
            return Ok(StartupAction::ShowHelp);
        }
    }
    validate(&options)?;
    Ok(StartupAction::Run(options))
}

fn apply_argument(
    options: &mut StartupOptions,
    argument: &str,
    values: &mut impl Iterator<Item = String>,
) -> anyhow::Result<bool> {
    match argument {
        "--data-dir" => options.data_dir = required(values, "--data-dir requires a path")?.into(),
        "--fixture-package-dir" => {
            options.fixture_package_dir =
                Some(required(values, "--fixture-package-dir requires a path")?.into());
        }
        "--bind" => options.bind = required(values, "--bind requires an address")?.parse()?,
        "--test-bench" => options.test_bench = true,
        "--osc-bind" => {
            options.osc_bind_override =
                Some(required(values, "--osc-bind requires an address")?.parse()?);
        }
        "--output-bind-ip" => {
            options.output_bind_override =
                Some(required(values, "--output-bind-ip requires an address")?.parse()?);
        }
        "--help" => return Ok(true),
        _ => anyhow::bail!("unknown option: {argument}"),
    }
    Ok(false)
}

fn required(
    values: &mut impl Iterator<Item = String>,
    message: &'static str,
) -> anyhow::Result<String> {
    values.next().context(message)
}

fn validate(options: &StartupOptions) -> anyhow::Result<()> {
    if options.test_bench && !options.bind.ip().is_loopback() {
        anyhow::bail!("--test-bench requires a loopback HTTP bind");
    }
    Ok(())
}

impl StartupOptions {
    fn with_data_dir(environment_data_dir: Option<PathBuf>) -> Self {
        Self {
            data_dir: environment_data_dir.unwrap_or_else(|| PathBuf::from("light-data")),
            fixture_package_dir: None,
            bind: SocketAddr::from((Ipv4Addr::LOCALHOST, 5000)),
            test_bench: false,
            osc_bind_override: None,
            output_bind_override: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_existing_server_contract() {
        let options = run_options(&[], None);

        assert_eq!(options.data_dir, PathBuf::from("light-data"));
        assert_eq!(options.bind, SocketAddr::from((Ipv4Addr::LOCALHOST, 5000)));
        assert!(!options.test_bench);
        assert_eq!(options.fixture_package_dir, None);
    }

    #[test]
    fn command_line_values_override_the_environment_default() {
        let options = run_options(
            &[
                "--data-dir",
                "cli-data",
                "--fixture-package-dir",
                "fixtures",
                "--bind",
                "127.0.0.2:5100",
                "--test-bench",
                "--osc-bind",
                "127.0.0.1:9100",
                "--output-bind-ip",
                "127.0.0.2",
            ],
            Some("environment-data"),
        );

        assert_eq!(options.data_dir, PathBuf::from("cli-data"));
        assert_eq!(options.fixture_package_dir, Some(PathBuf::from("fixtures")));
        assert_eq!(options.bind, "127.0.0.2:5100".parse().unwrap());
        assert_eq!(
            options.osc_bind_override,
            Some("127.0.0.1:9100".parse().unwrap())
        );
        assert_eq!(
            options.output_bind_override,
            Some("127.0.0.2".parse().unwrap())
        );
        assert!(options.test_bench);
    }

    #[test]
    fn environment_data_directory_is_used_without_a_command_line_override() {
        let options = run_options(&[], Some("environment-data"));

        assert_eq!(options.data_dir, PathBuf::from("environment-data"));
    }

    #[test]
    fn help_stops_parsing_immediately() {
        let action = parse(strings(&["--help", "--unknown"]), None).unwrap();

        assert_eq!(action, StartupAction::ShowHelp);
    }

    #[test]
    fn value_options_report_their_existing_missing_value_errors() {
        for (option, message) in missing_value_cases() {
            let error = parse(strings(&[option]), None).unwrap_err();
            assert_eq!(error.to_string(), message);
        }
    }

    #[test]
    fn unknown_options_are_rejected() {
        let error = parse(strings(&["--unknown"]), None).unwrap_err();

        assert_eq!(error.to_string(), "unknown option: --unknown");
    }

    #[test]
    fn test_bench_requires_a_loopback_http_bind() {
        let error = parse(strings(&["--test-bench", "--bind", "0.0.0.0:5000"]), None).unwrap_err();

        assert_eq!(
            error.to_string(),
            "--test-bench requires a loopback HTTP bind"
        );
    }

    fn run_options(arguments: &[&str], environment_data_dir: Option<&str>) -> StartupOptions {
        let environment_data_dir = environment_data_dir.map(PathBuf::from);
        let StartupAction::Run(options) = parse(strings(arguments), environment_data_dir).unwrap()
        else {
            panic!("expected runnable startup options");
        };
        options
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn missing_value_cases() -> [(&'static str, &'static str); 5] {
        [
            ("--data-dir", "--data-dir requires a path"),
            (
                "--fixture-package-dir",
                "--fixture-package-dir requires a path",
            ),
            ("--bind", "--bind requires an address"),
            ("--osc-bind", "--osc-bind requires an address"),
            ("--output-bind-ip", "--output-bind-ip requires an address"),
        ]
    }
}
