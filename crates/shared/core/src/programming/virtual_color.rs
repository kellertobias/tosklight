//! Version 1 of the fixture-independent virtual RGB/Amber authoring engine. Its constants define
//! an authoring vocabulary, not spectral measurements or native drives of a physical lamp.
use super::*;
use crate::{PickerColor, Xyz, hsv_to_rgb, rgb_to_hsv, srgb_to_xyz, xyz_to_srgb};

#[derive(Clone, Copy, Debug, Default)]
pub struct VirtualColorAuthoringV1;

impl VirtualColorAuthoringV1 {
    /// Deliberately defined virtual amber: full sRGB red, half encoded green, zero blue.
    /// Its linear XYZ is added at the Amber level, independently of UV and White Blend.
    pub const AMBER_SRGB: [f32; 3] = [1.0, 0.5, 0.0];

    pub fn recipe_xyz(recipe: &VirtualColorRecipe) -> Result<Xyz, IntentError> {
        require(
            recipe.version == 1,
            "unsupported virtual Color recipe version",
        )?;
        require(
            recipe
                .rgb
                .into_iter()
                .chain([recipe.amber])
                .all(|value| ScalarDomain::UNIT.contains(value)),
            "virtual Color recipe is outside 0-1",
        )?;
        let [r, g, b] = recipe.rgb;
        let rgb = srgb_to_xyz(r, g, b);
        let [r, g, b] = Self::AMBER_SRGB;
        let amber = srgb_to_xyz(r, g, b);
        Ok(Xyz {
            x: rgb.x + recipe.amber * amber.x,
            y: rgb.y + recipe.amber * amber.y,
            z: rgb.z + recipe.amber * amber.z,
        })
    }
}
impl ColorAuthoringModel for VirtualColorAuthoringV1 {
    fn read_base_component(
        &self,
        intent: &ColorIntent,
        component: ColorComponent,
    ) -> Result<f32, IntentError> {
        require(
            intent.recipe.version == 1,
            "unsupported virtual Color recipe version",
        )?;
        let [r, g, b] = intent.recipe.rgb;
        let (hue, saturation, _) = rgb_to_hsv(r, g, b);
        match component {
            ColorComponent::Red => Ok(r),
            ColorComponent::Green => Ok(g),
            ColorComponent::Blue => Ok(b),
            ColorComponent::Amber => Ok(intent.recipe.amber),
            ColorComponent::Hue => Ok(hue * 360.0),
            ColorComponent::Saturation => Ok(saturation),
            _ => Err(IntentError(
                "component does not edit the virtual Color base".into(),
            )),
        }
    }
    fn set_base_component(
        &self,
        intent: &mut ColorIntent,
        component: ColorComponent,
        value: f32,
    ) -> Result<(), IntentError> {
        let domain = ProgrammingComponent::Color(component)
            .descriptor()
            .domain
            .unwrap();
        require(
            domain.contains(value),
            "virtual Color edit is outside its domain",
        )?;
        let mut recipe = intent.recipe.clone();
        require(
            recipe.version == 1,
            "unsupported virtual Color recipe version",
        )?;
        match component {
            ColorComponent::Red => recipe.rgb[0] = value,
            ColorComponent::Green => recipe.rgb[1] = value,
            ColorComponent::Blue => recipe.rgb[2] = value,
            ColorComponent::Amber => recipe.amber = value,
            ColorComponent::Hue | ColorComponent::Saturation => {
                let [r, g, b] = recipe.rgb;
                let (hue, saturation, brightness) = rgb_to_hsv(r, g, b);
                recipe.rgb = hsv_to_rgb(PickerColor {
                    hue: if component == ColorComponent::Hue {
                        value / 360.0
                    } else {
                        hue
                    },
                    saturation: if component == ColorComponent::Saturation {
                        value
                    } else {
                        saturation
                    },
                    brightness,
                });
            }
            _ => {
                return Err(IntentError(
                    "component does not edit the virtual Color base".into(),
                ));
            }
        }
        let xyz = Self::recipe_xyz(&recipe)?;
        recipe.approximate = false;
        intent.recipe = recipe;
        intent.base_xyz = xyz;
        Ok(())
    }
    fn set_coordinates(&self, intent: &mut ColorIntent, xyz: Xyz) -> Result<(), IntentError> {
        require(
            [xyz.x, xyz.y, xyz.z]
                .into_iter()
                .all(|value| value.is_finite() && value >= 0.0),
            "Color coordinates must be finite and non-negative",
        )?;
        let (r, g, b) = xyz_to_srgb(xyz);
        let recipe = VirtualColorRecipe {
            rgb: [r, g, b],
            amber: 0.0,
            ..Default::default()
        };
        let estimate = Self::recipe_xyz(&recipe)?;
        let error = (xyz.x - estimate.x)
            .abs()
            .max((xyz.y - estimate.y).abs())
            .max((xyz.z - estimate.z).abs());
        intent.recipe = VirtualColorRecipe {
            approximate: error > 0.0001,
            ..recipe
        };
        intent.base_xyz = xyz;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn virtual_edits_preserve_uv_white_and_output_and_black_stays_black() {
        let model = VirtualColorAuthoringV1;
        let mut intent = ColorIntent {
            uv: UvIntent { amount: 0.8 },
            white_blend: 0.5,
            relative_output: 0.0,
            ..Default::default()
        };
        for component in [
            ColorComponent::Red,
            ColorComponent::Green,
            ColorComponent::Blue,
        ] {
            model
                .set_base_component(&mut intent, component, 0.0)
                .unwrap();
        }
        assert_eq!(
            intent.base_xyz,
            Xyz {
                x: 0.0,
                y: 0.0,
                z: 0.0
            }
        );
        model
            .set_base_component(&mut intent, ColorComponent::Amber, 1.0)
            .unwrap();
        assert_eq!(intent.base_xyz, srgb_to_xyz(1.0, 0.5, 0.0));
        assert_eq!(intent.uv.amount, 0.8);
        assert_eq!(intent.relative_output, 0.0);
        assert_eq!(intent.white_blend, 0.5);
    }
    #[test]
    fn coordinate_adoption_retains_exact_xyz_until_an_easy_control_is_edited() {
        let model = VirtualColorAuthoringV1;
        let mut intent = ColorIntent::default();
        let xyz = Xyz {
            x: 0.0,
            y: 2.0,
            z: 0.0,
        };
        model.set_coordinates(&mut intent, xyz).unwrap();
        assert_eq!(intent.base_xyz, xyz);
        assert!(intent.recipe.approximate);
        model
            .read_base_component(&intent, ColorComponent::Hue)
            .unwrap();
        assert_eq!(intent.base_xyz, xyz);
        model
            .set_base_component(&mut intent, ColorComponent::Red, 0.5)
            .unwrap();
        assert!(!intent.recipe.approximate);
        assert_eq!(
            intent.base_xyz,
            VirtualColorAuthoringV1::recipe_xyz(&intent.recipe).unwrap()
        );
    }
    #[test]
    fn largest_finite_coordinate_has_a_finite_display_recipe() {
        let mut intent = ColorIntent::default();
        VirtualColorAuthoringV1
            .set_coordinates(
                &mut intent,
                Xyz {
                    x: f32::MAX,
                    y: f32::MAX,
                    z: f32::MAX,
                },
            )
            .unwrap();
        assert!(
            intent
                .recipe
                .rgb
                .into_iter()
                .all(|value| ScalarDomain::UNIT.contains(value))
        );
        assert!(intent.recipe.approximate);
    }
}

#[cfg(test)]
mod atomic_tests {
    use super::*;
    use crate::AttributeValue;
    use std::sync::Arc;
    #[test]
    fn hue_and_saturation_bundle_is_order_independent_from_white() {
        let make = |component, value| ComponentEdit::Scalar {
            component: ProgrammingComponent::Color(component),
            operation: ScalarEdit::Set(ScalarIntent::Value(value)),
        };
        let edits = [
            make(ColorComponent::Hue, 300.0),
            make(ColorComponent::Saturation, 1.0),
        ];
        let seed = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent::default(),
        }));
        let context = FamilyEditContext {
            color_model: Some(&VirtualColorAuthoringV1),
            ..Default::default()
        };
        let result = edit_family(&seed, &edits, &context).unwrap();
        assert_eq!(
            result,
            edit_family(&seed, &[edits[1].clone(), edits[0].clone()], &context).unwrap()
        );
        let AttributeValue::ColorProgram(program) = result else {
            panic!()
        };
        let ColorProgram::Semantic { intent } = program.as_ref() else {
            panic!()
        };
        assert_eq!(intent.recipe.rgb, [1.0, 0.0, 1.0]);
    }
    #[test]
    fn falsely_exact_recipe_is_rejected_while_coordinate_approximation_is_retained() {
        let mut intent = ColorIntent {
            base_xyz: Xyz {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            ..Default::default()
        };
        assert!(intent.validate().is_err());
        intent.recipe.approximate = true;
        assert!(intent.validate().is_ok());
        assert!(
            compile_programming_spread(&AttributeValue::Spread(vec![]), 0, &Default::default())
                .is_err()
        );
    }
}
