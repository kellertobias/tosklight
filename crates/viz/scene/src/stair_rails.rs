//! Stair handrails: which sides of a flight carry one, and how older snapshots read.

/// Which sides of a flight of stairs a handrail runs up, as seen climbing it.
///
/// A snapshot written before the sides could be chosen said only whether the flight had rails; it
/// reads as rails up both sides or none, which is what it drew. The binary channel to the helper is
/// not self-describing, so there the two sides are read as they were written.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct StairRails {
    pub left: bool,
    pub right: bool,
}

impl StairRails {
    pub const NONE: Self = Self {
        left: false,
        right: false,
    };
    pub const BOTH: Self = Self {
        left: true,
        right: true,
    };

    pub fn any(self) -> bool {
        self.left || self.right
    }
}

#[derive(serde::Deserialize)]
struct StairRailSides {
    left: bool,
    right: bool,
}

impl<'de> serde::Deserialize<'de> for StairRails {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if !deserializer.is_human_readable() {
            let StairRailSides { left, right } = StairRailSides::deserialize(deserializer)?;
            return Ok(Self { left, right });
        }
        StairRailsWire::deserialize(deserializer).map(Self::from)
    }
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum StairRailsWire {
    Legacy(bool),
    Sides {
        #[serde(default)]
        left: bool,
        #[serde(default)]
        right: bool,
    },
}

impl From<StairRailsWire> for StairRails {
    fn from(wire: StairRailsWire) -> Self {
        match wire {
            StairRailsWire::Legacy(railed) => Self {
                left: railed,
                right: railed,
            },
            StairRailsWire::Sides { left, right } => Self { left, right },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stair_rails_read_old_snapshots_as_both_sides_or_none() {
        let rails = |json: &str| serde_json::from_str::<StairRails>(json).unwrap();
        assert_eq!(rails("true"), StairRails::BOTH);
        assert_eq!(rails("false"), StairRails::NONE);
        assert_eq!(
            rails(r#"{"left":true}"#),
            StairRails {
                left: true,
                right: false
            }
        );
        let detail: crate::SceneryDetail = serde_json::from_str("{}").unwrap();
        assert_eq!(detail.handrails, StairRails::NONE);
        let written = serde_json::to_string(&StairRails {
            left: false,
            right: true,
        })
        .unwrap();
        assert_eq!(
            rails(&written),
            StairRails {
                left: false,
                right: true
            }
        );
    }
}
