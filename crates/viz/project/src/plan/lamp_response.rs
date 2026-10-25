//! Explicit source metadata wins; lantern shape/name/wattage never identifies lamp technology.
use light_fixture::{FixtureProfile, InstalledFixtureAppearance, InstalledLightSource};
use viz_scene::LampResponseProfile;

pub(super) fn resolve(
    profile: &FixtureProfile,
    installed: &InstalledFixtureAppearance,
) -> LampResponseProfile {
    resolve_source(&installed.light_source, &profile.physical.light_source)
}

fn resolve_source(installed: &InstalledLightSource, declared: &str) -> LampResponseProfile {
    use InstalledLightSource::*;
    match installed {
        Tungsten | Halogen => LampResponseProfile::TungstenNominal,
        ProfileDefault => {
            let text = declared.to_ascii_lowercase();
            let words: Vec<_> = text.split(|c: char| !c.is_ascii_alphanumeric()).collect();
            if words.contains(&"led")
                || words.contains(&"discharge")
                || words.contains(&"fluorescent")
            {
                LampResponseProfile::Instant
            } else if words
                .iter()
                .any(|word| matches!(*word, "tungsten" | "halogen" | "incandescent"))
            {
                LampResponseProfile::TungstenNominal
            } else {
                LampResponseProfile::Instant
            }
        }
        _ => LampResponseProfile::Instant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_installed_source_overrides_profile_and_unknown_is_instant() {
        assert_eq!(
            resolve_source(&InstalledLightSource::Led, "tungsten"),
            LampResponseProfile::Instant
        );
        assert_eq!(
            resolve_source(&InstalledLightSource::Halogen, "LED"),
            LampResponseProfile::TungstenNominal
        );
        for text in [
            "Fresnel 2000W",
            "PAR64",
            "ACL 250W/28V",
            "",
            "LED tungsten emulation",
        ] {
            assert_eq!(
                resolve_source(&InstalledLightSource::ProfileDefault, text),
                LampResponseProfile::Instant
            );
        }
        for text in ["Tungsten-halogen 750W", "incandescent", "Halogen"] {
            assert_eq!(
                resolve_source(&InstalledLightSource::ProfileDefault, text),
                LampResponseProfile::TungstenNominal
            );
        }
    }
}
