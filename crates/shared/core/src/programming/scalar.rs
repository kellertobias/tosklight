use super::{IntentError, require};
use crate::AttributeBounds;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScalarDomain {
    /// Unknown fixture limits remain unknown. Signed angles and metres are not percentages.
    Finite,
    Bounded {
        bounds: AttributeBounds,
    },
    Cyclic {
        bounds: AttributeBounds,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarInterpolation {
    Linear,
    ShortestArc,
    Reciprocal,
}

impl ScalarDomain {
    pub const UNIT: Self = Self::Bounded {
        bounds: AttributeBounds { min: 0.0, max: 1.0 },
    };
    pub const DEGREES: Self = Self::Cyclic {
        bounds: AttributeBounds {
            min: 0.0,
            max: 360.0,
        },
    };
    pub const KELVIN: Self = Self::Bounded {
        bounds: AttributeBounds {
            min: 1000.0,
            max: 20000.0,
        },
    };
    pub const DUV: Self = Self::Bounded {
        bounds: AttributeBounds {
            min: -0.03,
            max: 0.03,
        },
    };

    pub fn validate(self) -> Result<(), IntentError> {
        match self {
            Self::Finite => Ok(()),
            Self::Bounded { bounds } | Self::Cyclic { bounds } => require(
                bounds.min.is_finite()
                    && bounds.max.is_finite()
                    && bounds.min < bounds.max
                    && (bounds.max - bounds.min).is_finite(),
                "scalar bounds must be finite, ordered and have a finite positive span",
            ),
        }
    }

    pub fn contains(self, value: f32) -> bool {
        if !value.is_finite() || self.validate().is_err() {
            return false;
        }
        match self {
            Self::Finite => true,
            Self::Bounded { bounds } | Self::Cyclic { bounds } => {
                (bounds.min..=bounds.max).contains(&value)
            }
        }
    }

    /// Relative edits clamp or wrap only when the component explicitly declares that behavior.
    pub fn constrain(self, value: f32) -> Result<f32, IntentError> {
        self.validate()?;
        require(value.is_finite(), "component value must be finite")?;
        Ok(match self {
            Self::Finite => value,
            Self::Bounded { bounds } => value.clamp(bounds.min, bounds.max),
            Self::Cyclic { bounds } => {
                bounds.min
                    + ((f64::from(value) - f64::from(bounds.min))
                        .rem_euclid(f64::from(bounds.max) - f64::from(bounds.min))
                        as f32)
            }
        })
    }
}

pub fn interpolate_scalar(from: f32, to: f32, progress: f32, rule: ScalarInterpolation) -> f32 {
    if progress <= 0.0 {
        return from;
    }
    if progress >= 1.0 {
        return to;
    }
    match rule {
        ScalarInterpolation::Linear => {
            (f64::from(from) * (1.0 - f64::from(progress)) + f64::from(to) * f64::from(progress))
                as f32
        }
        ScalarInterpolation::Reciprocal => 1.0 / ((1.0 - progress) / from + progress / to),
        ScalarInterpolation::ShortestArc => {
            let mut delta = (to - from).rem_euclid(360.0);
            if delta > 180.0 {
                delta -= 360.0;
            }
            (from + delta * progress).rem_euclid(360.0)
        }
    }
}

/// A spread belongs to an editable component inside its complete owner. Control points retain
/// operator order. Compilation resolves the same existing rank/anchor rules before fixture fit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ScalarIntent {
    Value(f32),
    Spread(Vec<f32>),
}

impl ScalarIntent {
    pub fn validate(&self, domain: ScalarDomain) -> Result<(), IntentError> {
        match self {
            Self::Value(value) => require(
                domain.contains(*value),
                "component value is outside its domain",
            ),
            Self::Spread(points) => {
                require(
                    (2..=4096).contains(&points.len()),
                    "a semantic spread requires 2-4096 control points",
                )?;
                require(
                    points.iter().all(|value| domain.contains(*value)),
                    "spread control point is outside its domain",
                )
            }
        }
    }

    /// Materialize only requested ranks while preserving the full Group's anchor layout.
    pub fn resolve_selected(
        &self,
        count: usize,
        ranks: &[usize],
        circular: bool,
    ) -> Result<Vec<f32>, IntentError> {
        require(
            ranks.iter().all(|rank| *rank < count),
            "spread rank is outside its selection",
        )?;
        self.validate(ScalarDomain::Finite)?;
        let Self::Spread(points) = self else {
            let Self::Value(value) = self else {
                unreachable!()
            };
            return Ok(vec![*value; ranks.len()]);
        };
        let mut points = points.clone();
        if circular {
            for index in 1..points.len() {
                let mut delta = (points[index] - points[index - 1]).rem_euclid(360.0);
                if delta > 180.0 {
                    delta -= 360.0;
                }
                points[index] = points[index - 1] + delta;
            }
        }
        let layout = super::ranks::SpreadRankLayout::new(points.len(), count);
        Ok(ranks
            .iter()
            .map(|rank| {
                let position = layout.scalar_position(*rank);
                let left = (position.floor() as usize).min(points.len() - 1);
                let right = (left + 1).min(points.len() - 1);
                let fraction = f64::from(position) - left as f64;
                let value = (f64::from(points[left]) * (1.0 - fraction)
                    + f64::from(points[right]) * fraction) as f32;
                if circular {
                    value.rem_euclid(360.0)
                } else {
                    value
                }
            })
            .collect())
    }

    /// Called when selection/group ranks change, not separately for each fixture each tick.
    pub fn resolve(&self, count: usize, circular: bool) -> Vec<f32> {
        match self {
            Self::Value(value) => vec![*value; count],
            Self::Spread(points) if circular => {
                let mut unwrapped = Vec::with_capacity(points.len());
                if let Some(first) = points.first() {
                    unwrapped.push(*first);
                }
                for next in points.iter().skip(1) {
                    let previous = *unwrapped.last().expect("spread has a first point");
                    let mut delta = (*next - previous).rem_euclid(360.0);
                    if delta > 180.0 {
                        delta -= 360.0;
                    }
                    unwrapped.push(previous + delta);
                }
                resolve_physical_spread(&unwrapped, count)
                    .into_iter()
                    .map(|v| v.rem_euclid(360.0))
                    .collect()
            }
            Self::Spread(points) => resolve_physical_spread(points, count),
        }
    }
}

/// Reuse the authoritative anchor/rank layout, then mix physical values with wider arithmetic.
/// The old normalized path and its exact byte behavior stay unchanged.
fn resolve_physical_spread(points: &[f32], count: usize) -> Vec<f32> {
    if points.len() < 2 {
        return vec![points.first().copied().unwrap_or(0.0); count];
    }
    let indices = (0..points.len())
        .map(|index| index as f32)
        .collect::<Vec<_>>();
    crate::attributes::resolve_spread(&indices, count)
        .into_iter()
        .map(|position| {
            let left = (position.floor() as usize).min(points.len() - 1);
            let right = (left + 1).min(points.len() - 1);
            let fraction = f64::from(position) - left as f64;
            (f64::from(points[left]) * (1.0 - fraction) + f64::from(points[right]) * fraction)
                as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_spreads_preserve_order_and_existing_middle_anchors() {
        let value = ScalarIntent::Spread(vec![720.0, -90.0, 720.0]);
        value.validate(ScalarDomain::Finite).unwrap();
        assert_eq!(
            value.resolve(6, false),
            vec![720.0, 315.0, -90.0, -90.0, 315.0, 720.0]
        );
        assert_eq!(ScalarDomain::Finite.constrain(-720.0).unwrap(), -720.0);
        assert_eq!(
            ScalarIntent::Spread(vec![2000.0, 10000.0]).resolve(3, false)[1],
            6000.0
        );
        assert!(
            (interpolate_scalar(2000.0, 10000.0, 0.5, ScalarInterpolation::Reciprocal) - 3333.3333)
                .abs()
                < 0.001
        );
    }
    #[test]
    fn hue_uses_shortest_arc_with_clockwise_half_turn_ties() {
        assert_eq!(
            ScalarIntent::Spread(vec![350.0, 10.0]).resolve(3, true),
            vec![350.0, 0.0, 10.0]
        );
        assert_eq!(
            interpolate_scalar(180.0, 0.0, 0.5, ScalarInterpolation::ShortestArc),
            270.0
        );
        assert_eq!(
            interpolate_scalar(0.0, 720.0, 0.5, ScalarInterpolation::Linear),
            360.0
        );
    }
}
