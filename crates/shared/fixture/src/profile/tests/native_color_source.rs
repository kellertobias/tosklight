//! TL-773: no-colour availability must not project the complete catalogue per mode.
use super::derived_color_physical::package;
use super::*;
use std::borrow::Cow;
use std::time::{Duration, Instant};

fn prior_source(profile: &FixtureProfile, mode_id: Uuid) -> Option<Cow<'_, FixtureProfile>> {
    let mode = profile.mode(mode_id)?;
    if mode.color_physical.is_some() {
        return Some(Cow::Borrowed(profile));
    }
    let mut projected = profile.clone();
    apply_runtime_profile_compatibility(&mut projected);
    projected.mode(mode_id)?.color_physical.as_ref()?;
    Some(Cow::Owned(projected))
}

fn assert_prior_source_equivalence(profile: &FixtureProfile) {
    let original = serde_json::to_value(profile).unwrap();
    for mode in &profile.modes {
        let expected = prior_source(profile, mode.id);
        let actual = profile.native_color_source(mode.id);
        assert_eq!(actual.is_some(), expected.is_some(), "{}", mode.name);
        if let (Some(actual), Some(expected)) = (actual, expected) {
            assert_eq!(
                serde_json::to_value(actual.as_ref()).unwrap(),
                serde_json::to_value(expected.as_ref()).unwrap()
            );
            assert_eq!(
                actual.native_color_identities(mode.id).unwrap(),
                expected.native_color_identities(mode.id).unwrap()
            );
            assert_eq!(
                matches!(actual, Cow::Borrowed(_)),
                matches!(expected, Cow::Borrowed(_))
            );
        }
    }
    assert_eq!(serde_json::to_value(profile).unwrap(), original);
    assert!(profile.native_color_source(Uuid::nil()).is_none());
}

#[test]
fn two_thousand_no_color_modes_fit_the_existing_single_patch_compile_budget() {
    let mut profile = FixtureProfile::blank();
    profile.revision = 7;
    let template = profile.modes[0].clone();
    profile.modes = (0..2000)
        .map(|index| {
            let mut mode = template.clone();
            mode.id = Uuid::from_u128(10_000 + index);
            mode.name = format!("Mode {index}");
            mode
        })
        .collect();
    let original = serde_json::to_value(&profile).unwrap();
    let started = Instant::now();
    let available = profile
        .modes
        .iter()
        .filter(|mode| profile.native_color_source(mode.id).is_some())
        .count();
    let elapsed = started.elapsed();
    assert_eq!(available, 0);
    assert!(
        elapsed < Duration::from_millis(250),
        "Availability alone exceeded the existing 250ms single-Patch budget: {elapsed:?}"
    );
    assert_eq!(serde_json::to_value(&profile).unwrap(), original);
}

#[test]
fn no_color_and_authored_or_derived_color_keep_exact_original_sources() {
    for name in ["generic--dimmer", "generic--rgb-led", "cameo--root-par-6"] {
        assert_prior_source_equivalence(&package(name));
    }
}

#[test]
fn media_and_explicitly_excluded_color_keep_their_existing_disposition() {
    assert_prior_source_equivalence(&package("tosklight--media-server"));
    let mut profile = package("etc--source-four-led-series-2-lustr");
    profile.modes.retain(|mode| mode.name == "HSI Plus 7");
    for channel in &mut profile.modes[0].channels {
        channel.functions.retain(|function| !matches!(&function.behavior, ChannelFunctionBehavior::Fixed { semantic_id, .. } if semantic_id == "plus_seven_on"));
    }
    assert!(matches!(
        profile.modes[0].derived_color_outcome(),
        DerivedColorOutcome::Excluded(_)
    ));
    assert_prior_source_equivalence(&profile);
}

#[test]
fn actual_generated_gdtf_source_remains_on_full_compatibility_identity_path() {
    let profile = package("generic--rgb-led");
    let archive = crate::gdtf::profile::package_profile(&profile).unwrap();
    let imported = crate::gdtf::read::import_profile(&archive).unwrap().profile;
    assert!(
        imported
            .source_gdtf
            .as_ref()
            .unwrap()
            .matches_profile(&imported)
            .unwrap()
    );
    assert_eq!(
        imported
            .source_gdtf
            .as_ref()
            .unwrap()
            .decoded_archive()
            .unwrap(),
        archive
    );
    assert_prior_source_equivalence(&imported);
}
