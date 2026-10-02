//! Compiled native-output prediction shared by desk output and both Stage hosts.
//! Compilation owns UUID lookup, calibration validation and spectral resampling.
//! Evaluation never accepts the requested intent as a substitute for achieved output.
mod color;
mod optics;
mod position;
mod spectrum;

pub use color::*;
pub use optics::*;
pub use position::*;

pub(super) fn validate_native_domains(
    mode: &crate::FixtureMode,
) -> Result<(), crate::ProfileError> {
    for channel in &mode.channels {
        let mut functions = channel.functions.iter().collect::<Vec<_>>();
        functions.sort_unstable_by_key(|f| f.dmx_from);
        if functions
            .iter()
            .any(|f| f.dmx_from > f.dmx_to || f.dmx_to > channel.resolution.max_raw())
            || functions
                .windows(2)
                .any(|pair| pair[0].dmx_to >= pair[1].dmx_from)
        {
            return Err(crate::ProfileError::Invalid(
                "forward model: invalid or ambiguous native function range".into(),
            ));
        }
    }
    Ok(())
}

/// Configuration-time identity of ordered native channels. Portable compaction of other modes
/// does not change it. Transport consumers reject values compiled for a different layout.
pub fn native_output_identity(
    profile: &crate::FixtureProfile,
    mode: &crate::FixtureMode,
) -> String {
    use sha2::{Digest, Sha256};
    let channels: Vec<_> = mode
        .channels
        .iter()
        .map(|c| (c.id, c.resolution.max_raw()))
        .collect();
    let data = serde_json::to_vec(&(profile.id, profile.revision, mode.id, channels))
        .expect("validated native channel layout is serializable");
    format!("{:x}", Sha256::digest(data))
}

/// Adds native-output-relevant installed configuration, rejecting stale inversion/calibration
/// rows even when the selected mode has not changed.
pub fn native_instance_identity(
    profile: &crate::FixtureProfile,
    mode: &crate::FixtureMode,
    color: Option<&crate::InstalledColorCalibration>,
    position: PositionInstallation<'_>,
    appearance: &crate::InstalledFixtureAppearance,
    addresses: &[(u16, u16, u16)],
) -> String {
    use sha2::{Digest, Sha256};
    let mut addresses = addresses.to_vec();
    addresses.sort_unstable();
    let data = serde_json::to_vec(&(
        native_output_identity(profile, mode),
        addresses,
        color,
        position.calibration,
        position.invert_pan,
        position.invert_tilt,
        position.bracket_degrees,
        appearance,
    ))
    .expect("validated installed fixture data is serializable");
    format!("{:x}", Sha256::digest(data))
}
