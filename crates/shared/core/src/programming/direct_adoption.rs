//! Direct → Semantic Color adoption from a portable source estimate (TL-559).
//!
//! This is the single conversion used both by the first semantic edit of a Direct value (through
//! `FamilyEditContext::semantic_color_adoption`) and by incompatible Direct replay, which fits
//! the same derived intent through the destination resolver. It never consults a destination.
//!
//! Rules, from the portable estimate alone:
//! - Known visible XYZ is TOTAL output (brightness and known UV leakage included once, relative
//!   output 1). It becomes the base XYZ with the estimate's relative output and White Blend 0.
//!   It is never normalized: known black stays black, a half-output recipe stays half output.
//! - Unknown visible appearance is never invented: the caller must supply an explicit starting
//!   value, otherwise adoption fails. Known UV still replaces the starting value's UV.
//! - Known UV (including zero) is adopted independently of visible knowledge. Unknown UV
//!   (for example unequal independent banks) adopts UV off with an explicit limitation.
//! - The virtual recipe is always marked approximate: the Easy controls show an approximation,
//!   the base XYZ stays authoritative.
use super::*;
use crate::Xyz;

/// A derived semantic starting value and what it could not carry over losslessly.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticColorAdoption {
    pub intent: ColorIntent,
    /// Visible appearance came from the explicit starting value, not the Direct recipe.
    pub visible_from_start: bool,
    /// UV was unknown and adopted as off.
    pub uv_unknown: bool,
    pub limitations: Vec<String>,
}

/// Derive the semantic intent a Direct estimate stands for. `explicit_start` is required only
/// when visible appearance is unknown; it is otherwise ignored.
pub fn semantic_color_adoption(
    portable: &PortableColorEstimate,
    explicit_start: Option<&ColorIntent>,
) -> Result<SemanticColorAdoption, IntentError> {
    portable.validate()?;
    let mut limitations = portable.limitations.clone();
    let (mut intent, visible_from_start) = match portable.visible {
        Some(visible) => (adopt_visible(visible.xyz, visible.relative_output)?, false),
        None => {
            let start = explicit_start.ok_or_else(|| {
                IntentError(
                    "Direct appearance is unknown; semantic adoption requires an explicit starting Color"
                        .into(),
                )
            })?;
            limitations.push(
                "Direct appearance is unknown; the explicit starting Color was adopted.".into(),
            );
            (start.clone(), true)
        }
    };
    // Independent UV knowledge; never taken from the previous output or the starting value.
    let uv_unknown = portable.uv.is_none();
    intent.uv.amount = match portable.uv {
        Some(uv) => uv.amount,
        None => {
            limitations.push("Direct UV amount is unknown; UV is adopted off.".into());
            0.0
        }
    };
    intent.spreads.clear();
    intent.wheel_constraints.clear();
    intent.validate()?;
    Ok(SemanticColorAdoption {
        intent,
        visible_from_start,
        uv_unknown,
        limitations,
    })
}

fn adopt_visible(xyz: Xyz, relative_output: f32) -> Result<ColorIntent, IntentError> {
    require(
        valid_xyz(xyz) && relative_output.is_finite() && relative_output >= 0.0,
        "portable visible estimate must be finite and nonnegative",
    )?;
    let mut intent = ColorIntent {
        white_blend: 0.0,
        relative_output,
        ..ColorIntent::default()
    };
    VirtualColorAuthoringV1.set_coordinates(&mut intent, xyz)?;
    // The virtual recipe is a display approximation of total source output.
    intent.recipe.approximate = true;
    intent.base_xyz = xyz;
    Ok(intent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PhysicalDataQuality;

    fn estimate(visible: Option<Xyz>, uv: Option<f32>) -> PortableColorEstimate {
        PortableColorEstimate {
            model_revision: 1,
            visible: visible.map(|xyz| PortableVisibleColor {
                xyz,
                relative_output: 1.0,
            }),
            uv: uv.map(|amount| PortableUv {
                amount,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec!["recorded".into()],
        }
    }

    const DIM: Xyz = Xyz {
        x: 0.2,
        y: 0.1,
        z: 0.05,
    };
    const BLACK: Xyz = Xyz {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[test]
    fn known_visible_keeps_total_output_black_and_independent_uv() {
        let dim = semantic_color_adoption(&estimate(Some(DIM), Some(0.5)), None).unwrap();
        assert_eq!(dim.intent.base_xyz, DIM, "never normalized");
        assert_eq!(dim.intent.relative_output, 1.0);
        assert_eq!(dim.intent.white_blend, 0.0);
        assert_eq!(dim.intent.uv.amount, 0.5);
        assert!(dim.intent.recipe.approximate);
        assert!(!dim.visible_from_start && !dim.uv_unknown);

        let black = semantic_color_adoption(&estimate(Some(BLACK), Some(0.9)), None).unwrap();
        assert_eq!(black.intent.base_xyz, BLACK, "known black is not D65 white");
        assert_eq!(black.intent.uv.amount, 0.9, "UV-only black survives");
    }

    #[test]
    fn unknown_visible_requires_an_explicit_start_and_never_invents_white() {
        assert!(semantic_color_adoption(&estimate(None, Some(0.5)), None).is_err());
        let start = ColorIntent {
            relative_output: 0.3,
            uv: UvIntent { amount: 0.8 },
            ..ColorIntent::default()
        };
        let adopted = semantic_color_adoption(&estimate(None, Some(0.0)), Some(&start)).unwrap();
        assert!(adopted.visible_from_start);
        assert_eq!(adopted.intent.relative_output, 0.3);
        assert_eq!(
            adopted.intent.uv.amount, 0.0,
            "known UV zero replaces start UV"
        );
        assert!(adopted.limitations.len() > 1);
    }

    #[test]
    fn unknown_uv_adopts_off_with_a_limitation() {
        let adopted = semantic_color_adoption(&estimate(Some(DIM), None), None).unwrap();
        assert_eq!(adopted.intent.uv.amount, 0.0);
        assert!(adopted.uv_unknown);
        assert!(adopted.limitations.iter().any(|l| l.contains("UV")));
    }
}
