//! Participating-medium density, owned outright by the renderer.

/// Effective atmosphere applied by the renderer.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Atmosphere {
    /// Normalised density `0..=1`. `0` is clear air.
    pub density: f32,
}

/// Atmosphere the renderer starts with: enough haze to see beams at all.
pub const DEFAULT_DENSITY: f32 = 0.5;

/// Renderer-local haze setting, persisted independently from the show.
///
/// Haze is deliberately not taken from the show. A hazer's DMX output describes how hard a
/// machine is working, not the density the room ends up with, so following it swings the picture
/// between an invisible rig and a milky one on a value the operator is not looking at. The
/// renderer keeps its own amount and honours it exactly, hazers patched or not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmospherePreference {
    /// `0..=1`, presented to the operator as `0–100%`.
    pub amount: f32,
}

impl Default for AtmospherePreference {
    fn default() -> Self {
        Self {
            amount: DEFAULT_DENSITY,
        }
    }
}

impl AtmospherePreference {
    /// The atmosphere to render, which is the operator's amount and nothing else.
    pub fn resolve(&self) -> Atmosphere {
        Atmosphere {
            density: self.amount.clamp(0.0, 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_renderer_amount_is_the_atmosphere() {
        let preference = AtmospherePreference { amount: 0.4 };
        assert_eq!(preference.resolve().density, 0.4);
    }

    #[test]
    fn zero_is_clear_air_and_the_amount_stays_bounded() {
        assert_eq!(AtmospherePreference { amount: 0.0 }.resolve().density, 0.0);
        assert_eq!(AtmospherePreference { amount: 2.5 }.resolve().density, 1.0);
    }

    #[test]
    fn the_default_shows_beams_without_any_operator_input() {
        assert_eq!(
            AtmospherePreference::default().resolve().density,
            DEFAULT_DENSITY
        );
    }
}
