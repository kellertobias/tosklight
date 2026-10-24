//! Validate the declared measurement condition; production now always uses its 60 Hz ceiling.
pub(super) fn validate_environment(benchmark: bool) -> Result<(), String> {
    validate(
        benchmark,
        std::env::var("LIGHT_STAGE_PACKAGED_BENCH_HELPER_HZ")
            .ok()
            .as_deref(),
    )
}

fn validate(benchmark: bool, value: Option<&str>) -> Result<(), String> {
    match (benchmark, value) {
        (true, Some(value)) if value != "60" => {
            Err("LIGHT_STAGE_PACKAGED_BENCH_HELPER_HZ must be 60 or absent".to_owned())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_supported_explicit_measurement_condition_is_accepted() {
        assert!(validate(true, Some("60")).is_ok());
        assert!(validate(true, None).is_ok());
        assert!(validate(true, Some("120")).is_err());
        assert!(validate(false, Some("120")).is_ok());
    }
}
