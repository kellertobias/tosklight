//! The canonical demo rig, declared once in source.
//!
//! This is the inventory the product demo shows and the video is shot from, and it is deliberately
//! written as data rather than as a saved show file: the show that ships is generated from this,
//! so a fixture package that gains a model, a mode or a gobo wheel reaches the demo by being
//! rebuilt rather than by someone remembering to re-export it.
//!
//! The classes are chosen to cover what the renderer has to be able to draw — a Sunstrip, a
//! blinder, a strobe, a conventional lamp, a wash, a scanner, a laser and a moving fixture — so
//! that opening the demo is also the golden scene for fixture-model resolution.

/// One block of identical fixtures hung in a line.
#[derive(Clone, Copy, Debug)]
pub struct RigBlock {
    /// Profile manufacturer, exactly as the shipped package states it.
    pub manufacturer: &'static str,
    /// Profile name, exactly as the shipped package states it.
    pub profile: &'static str,
    /// Mode name, exactly as the profile states it.
    pub mode: &'static str,
    /// Operator-facing name; a block of more than one is numbered from 1.
    pub label: &'static str,
    pub count: u32,
    /// The logical universe every fixture in the block is patched into.
    pub universe: u16,
    /// The first fixture number of the block. Blocks never overlap.
    pub first_number: u32,
    /// Where the first fixture hangs, in desk storage millimetres: `x` across the stage, `y`
    /// upstage away from the audience, `z` up.
    pub origin: (i32, i32, i32),
    /// Added per fixture after the first, in the same millimetres.
    pub step: (i32, i32, i32),
    /// Degrees the whole block is rotated by, in desk storage axes.
    pub rotation: (f32, f32, f32),
}

/// Every fixture the demo show contains.
///
/// Ordering is the patch order an operator would see, front of house to back of stage, and the
/// fixture numbers are stated rather than derived so a block inserted later cannot renumber the
/// rig underneath a script that names a fixture.
pub const DEMO_RIG: &[RigBlock] = &[
    // Front of house: the conventional wash the rig is lit from.
    RigBlock {
        manufacturer: "ETC",
        profile: "Source Four LED Series 2 Lustr",
        mode: "Direct",
        label: "FOH",
        count: 6,
        universe: 1,
        first_number: 101,
        origin: (-5_000, -6_000, 7_000),
        step: (2_000, 0, 0),
        rotation: (-30.0, 0.0, 0.0),
    },
    RigBlock {
        manufacturer: "Generic",
        profile: "Dimmer PAR Can",
        mode: "8-bit",
        label: "PAR",
        count: 6,
        universe: 1,
        first_number: 121,
        origin: (-5_000, -3_000, 6_500),
        step: (2_000, 0, 0),
        rotation: (-35.0, 0.0, 0.0),
    },
    // Representative conventional faces for close visualizer evidence. These are deliberately
    // single fixtures: their purpose is to make the compact ACL lens and Fresnel glass inspectable,
    // not to add another repeated wash to the demo design.
    RigBlock {
        manufacturer: "Generic",
        profile: "ACL",
        mode: "16-bit",
        label: "ACL",
        count: 1,
        universe: 5,
        first_number: 501,
        origin: (-1_000, -4_500, 6_000),
        step: (0, 0, 0),
        rotation: (-35.0, 0.0, 0.0),
    },
    RigBlock {
        manufacturer: "Generic",
        profile: "Dimmer Fresnel",
        mode: "16-bit",
        label: "Fresnel",
        count: 1,
        universe: 5,
        first_number: 502,
        origin: (1_000, -4_500, 6_000),
        step: (0, 0, 0),
        rotation: (-35.0, 0.0, 0.0),
    },
    // Mid truss: the washes and the profile movers.
    RigBlock {
        manufacturer: "ROBE",
        profile: "Robin 600X LEDWash",
        mode: "Mode 1",
        label: "Wash",
        count: 8,
        universe: 2,
        first_number: 201,
        origin: (-7_000, 0, 7_500),
        step: (2_000, 0, 0),
        rotation: (0.0, 0.0, 0.0),
    },
    RigBlock {
        manufacturer: "Martin",
        profile: "MAC 250 Entour",
        mode: "16 Bit",
        label: "Profile",
        count: 6,
        universe: 2,
        first_number: 221,
        origin: (-5_000, 2_000, 7_500),
        step: (2_000, 0, 0),
        rotation: (0.0, 0.0, 0.0),
    },
    // Back truss: the beams and the strobes.
    RigBlock {
        manufacturer: "Claypaky",
        profile: "Sharpy",
        mode: "Standard",
        label: "Beam",
        count: 8,
        universe: 3,
        first_number: 301,
        origin: (-7_000, 5_000, 7_500),
        step: (2_000, 0, 0),
        rotation: (0.0, 0.0, 0.0),
    },
    RigBlock {
        manufacturer: "GLP",
        profile: "JDC1",
        mode: "Compressed Pro 14-channel",
        label: "JDC1",
        count: 4,
        universe: 3,
        first_number: 321,
        origin: (-4_500, 5_500, 6_000),
        step: (3_000, 0, 0),
        rotation: (0.0, 0.0, 0.0),
    },
    RigBlock {
        manufacturer: "High End Systems",
        profile: "Trackspot",
        mode: "DMX High Resolution",
        label: "Scanner",
        count: 4,
        universe: 3,
        first_number: 341,
        origin: (-4_500, 7_000, 7_000),
        step: (3_000, 0, 0),
        rotation: (0.0, 0.0, 0.0),
    },
    // Floor and deck.
    RigBlock {
        manufacturer: "Showtec",
        profile: "Sunstrip Active DMX",
        mode: "10 Channel",
        label: "Sunstrip",
        count: 6,
        universe: 4,
        first_number: 401,
        origin: (-5_000, 6_500, 200),
        step: (2_000, 0, 0),
        rotation: (-90.0, 0.0, 0.0),
    },
    RigBlock {
        manufacturer: "Generic",
        profile: "Blinder",
        mode: "Two channel, eight blind",
        label: "Blinder",
        count: 4,
        universe: 4,
        first_number: 421,
        origin: (-4_500, 3_000, 6_800),
        step: (3_000, 0, 0),
        rotation: (-15.0, 0.0, 0.0),
    },
    RigBlock {
        manufacturer: "Generic",
        profile: "Strobe",
        mode: "Dimmer, Strobe",
        label: "Strobe",
        count: 4,
        universe: 4,
        first_number: 441,
        origin: (-4_500, 4_500, 500),
        step: (3_000, 0, 0),
        rotation: (-80.0, 0.0, 0.0),
    },
    RigBlock {
        manufacturer: "ToskLight",
        profile: "Visualizer Laser",
        mode: "12 Channel",
        label: "Laser",
        count: 2,
        universe: 4,
        first_number: 461,
        origin: (-3_000, 7_500, 1_000),
        step: (6_000, 0, 0),
        rotation: (-10.0, 0.0, 0.0),
    },
    RigBlock {
        manufacturer: "Generic",
        profile: "Hazer",
        mode: "Fog, Fan",
        label: "Hazer",
        count: 1,
        universe: 4,
        first_number: 481,
        origin: (6_000, 7_500, 300),
        step: (0, 0, 0),
        rotation: (0.0, 0.0, 0.0),
    },
];

impl RigBlock {
    /// Where the `index`-th fixture of this block hangs.
    pub const fn position(&self, index: u32) -> (i32, i32, i32) {
        let offset = index as i32;
        (
            self.origin.0 + self.step.0 * offset,
            self.origin.1 + self.step.1 * offset,
            self.origin.2 + self.step.2 * offset,
        )
    }

    /// What the operator sees in the fixture sheet. A block of one is not numbered.
    pub fn fixture_name(&self, index: u32) -> String {
        if self.count == 1 {
            self.label.to_owned()
        } else {
            format!("{} {}", self.label, index + 1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_fixture_number_in_the_rig_is_unique() {
        let mut seen = HashSet::new();
        for block in DEMO_RIG {
            for index in 0..block.count {
                let number = block.first_number + index;
                assert!(seen.insert(number), "fixture number {number} is used twice");
            }
        }
    }

    #[test]
    fn the_rig_covers_every_class_the_renderer_has_to_draw() {
        for required in [
            "Sunstrip Active DMX",
            "Blinder",
            "Strobe",
            "Dimmer PAR Can",
            "ACL",
            "Dimmer Fresnel",
            "Robin 600X LEDWash",
            "Trackspot",
            "Visualizer Laser",
            "Sharpy",
        ] {
            assert!(
                DEMO_RIG.iter().any(|block| block.profile == required),
                "the demo rig no longer contains a {required}"
            );
        }
    }

    #[test]
    fn a_block_of_one_is_not_numbered() {
        let hazer = DEMO_RIG
            .iter()
            .find(|block| block.profile == "Hazer")
            .expect("hazer");
        assert_eq!(hazer.fixture_name(0), "Hazer");
    }
}
