//! Configuration validation.
//!
//! Everything here runs before a subsystem starts. A configuration that cannot be honored stops
//! startup with a message that names the output and the field, rather than letting the process
//! come up in a state the operator did not ask for.

use std::collections::HashSet;

use media_domain::PresentationMode;
use media_domain::personality::StartAddressError;

use super::{MediaConfiguration, OutputConfiguration, migration::MigrationError};

/// Why a configuration cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigurationError {
    #[error("the configuration document is not readable: {detail}")]
    Malformed { detail: String },
    #[error(transparent)]
    Migration(#[from] MigrationError),
    #[error("the configuration defines no outputs; a Media Server needs at least one")]
    NoOutputs,
    #[error("two outputs share the identity {id}")]
    DuplicateOutputId { id: String },
    #[error("output '{output}' has an invalid DMX start address: {source}")]
    StartAddress {
        output: String,
        #[source]
        source: StartAddressError,
    },
    #[error("output '{output}' has a zero {axis} resolution")]
    EmptyResolution { output: String, axis: &'static str },
    #[error("output '{output}' requests a fixed rate of zero frames per second")]
    ZeroFixedRate { output: String },
    #[error(
        "outputs '{first}' and '{second}' both consume universe {universe} at address {start_address}"
    )]
    OverlappingPatch {
        first: String,
        second: String,
        universe: u16,
        start_address: u16,
    },
}

pub(super) fn validate(configuration: &MediaConfiguration) -> Result<(), ConfigurationError> {
    if configuration.outputs.is_empty() {
        return Err(ConfigurationError::NoOutputs);
    }

    let mut seen = HashSet::new();
    for output in &configuration.outputs {
        if !seen.insert(output.id) {
            return Err(ConfigurationError::DuplicateOutputId {
                id: output.id.to_string(),
            });
        }
        validate_output(output)?;
    }

    validate_patch_overlap(&configuration.outputs)
}

fn validate_output(output: &OutputConfiguration) -> Result<(), ConfigurationError> {
    let name = output.name.to_string();

    output
        .personality
        .footprint()
        .validate_start_address(output.start_address)
        .map_err(|source| ConfigurationError::StartAddress {
            output: name.clone(),
            source,
        })?;

    if output.resolution.width == 0 {
        return Err(ConfigurationError::EmptyResolution {
            output: name,
            axis: "width",
        });
    }
    if output.resolution.height == 0 {
        return Err(ConfigurationError::EmptyResolution {
            output: name,
            axis: "height",
        });
    }

    if let PresentationMode::FixedFps {
        frames_per_second: 0,
    } = output.presentation
    {
        return Err(ConfigurationError::ZeroFixedRate { output: name });
    }

    Ok(())
}

/// Two enabled outputs on the same protocol and universe must not claim the same slots. Silently
/// letting them overlap would make one desk fader drive two outputs by accident.
fn validate_patch_overlap(outputs: &[OutputConfiguration]) -> Result<(), ConfigurationError> {
    let patched: Vec<&OutputConfiguration> =
        outputs.iter().filter(|output| output.enabled).collect();

    for (index, first) in patched.iter().enumerate() {
        for second in &patched[index + 1..] {
            if first.protocol != second.protocol || first.universe != second.universe {
                continue;
            }
            let first_span = span(first);
            let second_span = span(second);
            if first_span.0 <= second_span.1 && second_span.0 <= first_span.1 {
                return Err(ConfigurationError::OverlappingPatch {
                    first: first.name.to_string(),
                    second: second.name.to_string(),
                    universe: first.universe,
                    start_address: second.start_address,
                });
            }
        }
    }
    Ok(())
}

fn span(output: &OutputConfiguration) -> (u16, u16) {
    let total = output.personality.footprint().total();
    (output.start_address, output.start_address + total - 1)
}

#[cfg(test)]
mod tests {
    use media_domain::LayerPersonality;

    use super::super::{MediaConfiguration, OutputConfiguration};
    use super::*;

    fn configuration(outputs: Vec<OutputConfiguration>) -> MediaConfiguration {
        MediaConfiguration {
            outputs,
            ..Default::default()
        }
    }

    #[test]
    fn the_default_configuration_validates() {
        assert_eq!(validate(&MediaConfiguration::default()), Ok(()));
    }

    #[test]
    fn a_configuration_without_outputs_is_rejected() {
        assert_eq!(
            validate(&configuration(vec![])),
            Err(ConfigurationError::NoOutputs)
        );
    }

    #[test]
    fn an_eight_layer_output_may_not_span_universes() {
        let mut output = OutputConfiguration::new("Main");
        output.start_address = 300;
        let error = validate(&configuration(vec![output])).unwrap_err();
        assert!(
            matches!(error, ConfigurationError::StartAddress { .. }),
            "{error}"
        );
        assert!(error.to_string().contains("Main"), "{error}");
    }

    #[test]
    fn duplicate_output_identities_are_rejected() {
        let first = OutputConfiguration::new("Main");
        let mut second = OutputConfiguration::new("Second");
        second.id = first.id;
        second.start_address = 280;
        second.personality = LayerPersonality::TwoLayers;
        let error = validate(&configuration(vec![first, second])).unwrap_err();
        assert!(
            matches!(error, ConfigurationError::DuplicateOutputId { .. }),
            "{error}"
        );
    }

    #[test]
    fn two_outputs_may_not_claim_the_same_slots() {
        let first = OutputConfiguration::new("Main");
        let mut second = OutputConfiguration::new("Second");
        second.start_address = 100;
        let error = validate(&configuration(vec![first, second])).unwrap_err();
        assert!(
            matches!(error, ConfigurationError::OverlappingPatch { .. }),
            "{error}"
        );
    }

    #[test]
    fn disabled_outputs_do_not_conflict() {
        let first = OutputConfiguration::new("Main");
        let mut second = OutputConfiguration::new("Second");
        second.start_address = 100;
        second.enabled = false;
        assert_eq!(validate(&configuration(vec![first, second])), Ok(()));
    }

    #[test]
    fn outputs_on_different_universes_coexist() {
        let first = OutputConfiguration::new("Main");
        let mut second = OutputConfiguration::new("Second");
        second.universe = 1;
        assert_eq!(validate(&configuration(vec![first, second])), Ok(()));
    }

    #[test]
    fn an_empty_resolution_is_rejected() {
        let mut output = OutputConfiguration::new("Main");
        output.resolution.height = 0;
        let error = validate(&configuration(vec![output])).unwrap_err();
        assert_eq!(
            error,
            ConfigurationError::EmptyResolution {
                output: "Main".to_owned(),
                axis: "height"
            }
        );
    }

    #[test]
    fn a_fixed_rate_of_zero_is_rejected() {
        let mut output = OutputConfiguration::new("Main");
        output.presentation = PresentationMode::FixedFps {
            frames_per_second: 0,
        };
        let error = validate(&configuration(vec![output])).unwrap_err();
        assert_eq!(
            error,
            ConfigurationError::ZeroFixedRate {
                output: "Main".to_owned()
            }
        );
    }
}
