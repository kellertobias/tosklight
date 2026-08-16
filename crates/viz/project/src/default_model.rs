//! Picking a body for a fixture whose profile carries no model of its own.
//!
//! Most shipped and imported profiles have no `model_asset`, and a rig full of identical
//! grey boxes tells an operator nothing. ToskLight ships a small default set (see
//! `assets/models`), and this chooses one from what the profile does say about itself:
//! its declared type first, and failing that the attributes it actually has channels for.
//!
//! The rules never look at a manufacturer or a product name. A fixture with pan, tilt and
//! a gobo wheel is drawn as a profile moving head whoever built it, and a fixture with
//! nothing but a dimmer is drawn as a Fresnel because that is the lantern a bare dimmer
//! channel is most often feeding.

/// One model shipped for this purpose, embedded so a packaged desk needs no asset path.
pub struct DefaultModel {
    /// The name it has in `assets/models`, which is what the catalogue and help call it.
    pub name: &'static str,
    pub bytes: &'static [u8],
}

macro_rules! shipped {
    ($constant:ident, $name:literal, $path:literal) => {
        pub static $constant: DefaultModel = DefaultModel {
            name: $name,
            bytes: include_bytes!(concat!("../../../../assets/models/", $path)),
        };
    };
}

shipped!(
    FRESNEL,
    "fresnel-barn-doors",
    "lamps/fresnel-barn-doors.glb"
);
shipped!(PROFILE_SPOT, "profile-spot", "lamps/profile-spot.glb");
shipped!(
    PAR_CAN,
    "par-64-short-nose-black",
    "lamps/par-64-short-nose-black.glb"
);
shipped!(
    MOVING_PROFILE,
    "moving-head-profile",
    "lamps/moving-head-profile.glb"
);
shipped!(
    MOVING_WASH,
    "moving-head-wash",
    "lamps/moving-head-wash.glb"
);
shipped!(
    MOVING_LED_WASH,
    "moving-head-led-wash-400",
    "lamps/moving-head-led-wash-400.glb"
);
shipped!(LED_PAR, "led-par-x-in-1", "lamps/led-par-x-in-1.glb");
shipped!(LED_STROBE, "led-strobe", "lamps/led-strobe.glb");
shipped!(BLINDER, "blinder-4-cell", "lamps/blinder-4-cell.glb");
shipped!(
    SCANNER,
    "scanner-mirror-spot",
    "lamps/scanner-mirror-spot.glb"
);
shipped!(
    LED_STRIP,
    "led-strip-rgbcct-1000",
    "lamps/led-strip-rgbcct-1000.glb"
);
shipped!(HAZER, "hazer", "lamps/hazer.glb");
shipped!(SHOW_LASER, "show-laser", "av/show-laser.glb");

/// Every renderer-owned fallback body, in stable gallery order.
pub fn all() -> [&'static DefaultModel; 13] {
    [
        &FRESNEL,
        &PROFILE_SPOT,
        &PAR_CAN,
        &MOVING_PROFILE,
        &MOVING_WASH,
        &MOVING_LED_WASH,
        &LED_PAR,
        &LED_STROBE,
        &BLINDER,
        &SCANNER,
        &LED_STRIP,
        &HAZER,
        &SHOW_LASER,
    ]
}

/// What a mode has channels for, which is all the rules need to know about it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FixtureTraits {
    pub dimmer: bool,
    pub pan: bool,
    pub tilt: bool,
    pub colour_wheel: bool,
    pub gobo: bool,
    pub rgb: bool,
    pub strobe: bool,
    pub fog: bool,
}

impl FixtureTraits {
    /// Fold one channel's canonical attribute into the traits.
    ///
    /// Attribute keys are canonical here — `color.red`, `gobo.1`, `shutter.strobe` — so
    /// this matches on their shape rather than on a fixed list of spellings.
    pub fn observe(&mut self, attribute: &str, is_intensity: bool) {
        let key = attribute.trim().to_ascii_lowercase();
        let base = key.rsplit('.').next().unwrap_or(key.as_str());
        if is_intensity || key == "dimmer" || base == "dimmer" {
            self.dimmer = true;
        }
        match key.as_str() {
            "pan" => self.pan = true,
            "tilt" => self.tilt = true,
            "fog" | "haze" | "smoke" => self.fog = true,
            _ => {}
        }
        if key.starts_with("gobo") {
            self.gobo = true;
        }
        if key.starts_with("color.wheel") || key == "color" {
            self.colour_wheel = true;
        }
        if key.starts_with("color") && matches!(base, "red" | "green" | "blue") {
            self.rgb = true;
        }
        // Subtractive colour mixing is still colour mixing: a CMY head is not a gobo spot.
        if key.starts_with("color") && matches!(base, "cyan" | "magenta" | "yellow") {
            self.rgb = true;
        }
        if key == "strobe" || base == "strobe" {
            self.strobe = true;
        }
    }

    pub fn moving(self) -> bool {
        self.pan && self.tilt
    }
}

/// Choose a model from the profile's declared type, or `None` if the type says nothing
/// useful. A declared type is trusted over the channel set, because a profile that calls
/// itself a blinder is a blinder even if its author gave it an RGB mixer.
fn by_declared_type(fixture_type: &str, traits: FixtureTraits) -> Option<&'static DefaultModel> {
    let normalised = fixture_type
        .trim()
        .to_ascii_lowercase()
        .replace(['_', '-'], " ");
    let words: Vec<&str> = normalised.split_whitespace().collect();
    let has = |needle: &str| words.contains(&needle);
    let moving = traits.moving() || has("moving");

    if has("hazer") || has("haze") || has("fogger") || has("fog") || has("smoke") {
        return Some(&HAZER);
    }
    // Before the pattern-position channels every show laser has can be mistaken for a yoke: a
    // laser is a box with a window in it whatever its chart calls pan and tilt.
    if has("laser") || has("lasers") {
        return Some(&SHOW_LASER);
    }
    if has("scanner") || has("mirror") {
        return Some(&SCANNER);
    }
    if has("blinder") {
        return Some(&BLINDER);
    }
    if has("strobe") {
        return Some(&LED_STROBE);
    }
    if has("strip") || has("sunstrip") || has("pixel") || has("bar") {
        return Some(&LED_STRIP);
    }
    if has("fresnel") {
        return Some(&FRESNEL);
    }
    if has("par") || has("parcan") || has("acl") {
        return Some(if traits.rgb { &LED_PAR } else { &PAR_CAN });
    }
    if has("wash") {
        return Some(if moving { &MOVING_LED_WASH } else { &LED_PAR });
    }
    if has("profile") || has("spot") || has("ellipsoidal") || has("beam") {
        return Some(if moving {
            &MOVING_PROFILE
        } else {
            &PROFILE_SPOT
        });
    }
    None
}

/// Choose a model from the attributes the mode has channels for.
///
/// Read in order, because the tests below are not mutually exclusive: a moving head with
/// both a gobo wheel and a colour mixer is a profile, and a fixture with a strobe channel
/// and an RGB mixer is an LED PAR that happens to strobe.
fn by_attributes(traits: FixtureTraits) -> &'static DefaultModel {
    if traits.fog {
        return &HAZER;
    }
    if traits.moving() {
        if traits.gobo || traits.colour_wheel {
            return &MOVING_PROFILE;
        }
        if traits.rgb {
            return &MOVING_LED_WASH;
        }
        return &MOVING_WASH;
    }
    if traits.rgb {
        return &LED_PAR;
    }
    if traits.strobe {
        return &LED_STROBE;
    }
    // Nothing but a level. A bare dimmer channel is feeding a lantern nobody described,
    // and a Fresnel is the one that looks least wrong standing in for any of them.
    &FRESNEL
}

/// The model a fixture is drawn with when its profile names none.
pub fn choose(fixture_type: &str, traits: FixtureTraits) -> &'static DefaultModel {
    by_declared_type(fixture_type, traits).unwrap_or_else(|| by_attributes(traits))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traits(attributes: &[&str]) -> FixtureTraits {
        let mut traits = FixtureTraits::default();
        for attribute in attributes {
            traits.observe(
                attribute,
                *attribute == "dimmer" || *attribute == "intensity",
            );
        }
        traits
    }

    /// Every model this module can hand back, so a new one cannot be added without meeting what
    /// the rest of these tests ask of a shipped body.
    /// The model's own bounding box, which is not centred on its origin: the origin is the rigging
    /// point and the body hangs below it.
    fn bounds(model: &viz_scene::FixtureModel) -> (glam::Vec3, glam::Vec3) {
        let mut min = glam::Vec3::splat(f32::INFINITY);
        let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
        for part in &model.parts {
            for position in &part.positions {
                let point = glam::Vec3::from_array(*position);
                min = min.min(point);
                max = max.max(point);
            }
        }
        (min, max)
    }

    #[test]
    fn every_shipped_default_model_is_readable() {
        for model in all() {
            let read = viz_scene::read_glb(model.bytes)
                .unwrap_or_else(|error| panic!("{}: {error}", model.name));
            assert!(!read.is_empty(), "{} has no triangles", model.name);
            assert!(
                read.warnings.is_empty(),
                "{}: {:?}",
                model.name,
                read.warnings
            );
        }
    }

    /// Every shipped body has to say where its light comes out, or the beam starts at the
    /// fixture's origin — which in this set is the rigging point, a lamp's length above the lens.
    #[test]
    fn every_shipped_default_model_says_where_its_light_leaves() {
        for model in all() {
            let read = viz_scene::read_glb(model.bytes).expect("the model reads");
            let anchor = read
                .emitter_anchor
                .unwrap_or_else(|| panic!("{} names no emitting surface", model.name));
            assert!(
                anchor.length() > 0.01,
                "{}: the light leaves from the model's own origin, which is what having no \
                 anchor already meant",
                model.name
            );
            let (min, max) = bounds(&read);
            assert!(
                anchor.cmpge(min - 0.001).all() && anchor.cmple(max + 0.001).all(),
                "{}: the light leaves from outside the body",
                model.name
            );
        }
    }

    /// Every shipped body has to say which way its emitting face looks as well as where it is, or
    /// the beam leaves out of the side of the box.
    #[test]
    fn every_shipped_default_model_says_which_way_its_light_goes() {
        for model in all() {
            let read = viz_scene::read_glb(model.bytes).expect("the model reads");
            let axis = read
                .emitter_axis
                .unwrap_or_else(|| panic!("{} names no emitting surface", model.name));
            assert!(
                (axis.length() - 1.0).abs() < 1e-3,
                "{}: the aim axis is not a direction",
                model.name
            );
        }
    }

    /// A hung lamp points down, so its lens is below the clamp it hangs from and it aims the way
    /// every emitter does at rest. The two units here that face forward instead — a hazer blowing
    /// out of its nozzle and a laser projecting out of its window — are why the aim is read off
    /// the body rather than assumed.
    #[test]
    fn a_hung_lamps_light_leaves_below_the_clamp() {
        for model in [
            &FRESNEL,
            &PROFILE_SPOT,
            &PAR_CAN,
            &MOVING_PROFILE,
            &MOVING_WASH,
            &MOVING_LED_WASH,
            &LED_PAR,
            &LED_STROBE,
            &BLINDER,
            &SCANNER,
            &LED_STRIP,
        ] {
            let read = viz_scene::read_glb(model.bytes).expect("the model reads");
            let anchor = read.emitter_anchor.expect("the lamp names a lens");
            assert!(
                anchor.y < -0.02,
                "{}: the light leaves at y={}, at or above the rigging point",
                model.name,
                anchor.y
            );
            assert_eq!(
                read.emitter_axis,
                Some(glam::Vec3::NEG_Y),
                "{}: a hung lamp aims anywhere but down",
                model.name
            );
        }
    }

    /// The laser is a box with its window in the front face, so its beam leaves forwards. Getting
    /// this wrong puts the beam through the side of the projector, which is what the shared aim of
    /// every other body would do to it.
    #[test]
    fn the_lasers_beam_leaves_the_window_in_its_front_face() {
        let read = viz_scene::read_glb(SHOW_LASER.bytes).expect("the model reads");
        assert_eq!(read.emitter_axis, Some(glam::Vec3::Z));
        let anchor = read.emitter_anchor.expect("the projector names its window");
        let (_, max) = bounds(&read);
        assert!(
            anchor.z >= max.z - 0.05,
            "the beam leaves at z={}, behind the front of the body at {}",
            anchor.z,
            max.z
        );
    }

    /// A moving head's beam has to start at the lens, not at the hanging point metres above it.
    #[test]
    fn a_moving_heads_light_leaves_from_its_head_not_its_base() {
        let read = viz_scene::read_glb(MOVING_PROFILE.bytes).expect("the model reads");
        let anchor = read.emitter_anchor.expect("the head names a lens");
        assert!(read.has_head, "a moving head has parts that tilt");
        // Below the trunnions it tilts about, because the lens is at the nose of the head.
        assert!(
            anchor.y < read.head_pivot.y,
            "the lens at y={} is not below the trunnions at y={}",
            anchor.y,
            read.head_pivot.y
        );
    }

    #[test]
    fn a_bare_dimmer_becomes_a_fresnel() {
        assert_eq!(choose("", traits(&["dimmer"])).name, FRESNEL.name);
    }

    #[test]
    fn pan_tilt_with_a_wheel_or_a_gobo_becomes_a_profile_moving_head() {
        let wheel = traits(&["dimmer", "pan", "tilt", "color.wheel.1"]);
        let gobo = traits(&["dimmer", "pan", "tilt", "gobo.1"]);
        assert_eq!(choose("", wheel).name, MOVING_PROFILE.name);
        assert_eq!(choose("", gobo).name, MOVING_PROFILE.name);
    }

    #[test]
    fn pan_tilt_with_colour_mixing_and_no_gobo_becomes_an_led_moving_head() {
        let mixed = traits(&[
            "dimmer",
            "pan",
            "tilt",
            "color.red",
            "color.green",
            "color.blue",
        ]);
        assert_eq!(choose("", mixed).name, MOVING_LED_WASH.name);
    }

    #[test]
    fn colour_mixing_without_movement_becomes_an_led_par() {
        let par = traits(&["dimmer", "color.red", "color.green", "color.blue"]);
        assert_eq!(choose("", par).name, LED_PAR.name);
    }

    #[test]
    fn a_strobe_channel_alone_becomes_a_strobe() {
        assert_eq!(
            choose("", traits(&["dimmer", "strobe"])).name,
            LED_STROBE.name
        );
    }

    #[test]
    fn a_fog_channel_becomes_a_hazer_whatever_else_it_has() {
        let hazer = traits(&["dimmer", "fog", "color.red", "color.green", "color.blue"]);
        assert_eq!(choose("", hazer).name, HAZER.name);
    }

    #[test]
    fn a_declared_type_wins_over_the_channel_set() {
        // An RGB blinder is still a blinder, and a static profile is not a moving head.
        let rgb = traits(&["dimmer", "color.red", "color.green", "color.blue"]);
        assert_eq!(choose("blinder", rgb).name, BLINDER.name);
        assert_eq!(
            choose("profile", traits(&["dimmer"])).name,
            PROFILE_SPOT.name
        );
        assert_eq!(
            choose("profile moving head", traits(&["dimmer", "pan", "tilt"])).name,
            MOVING_PROFILE.name
        );
    }

    /// The cases the desk's own copy of this rule pins, so the two languages are checked against
    /// one list rather than drifting apart quietly.
    ///
    /// The desk cannot call this function — it runs in a browser — so the rule is written twice.
    /// What keeps the two honest is that both test files assert these same answers.
    ///
    /// @see apps/light-desktop/src/windows/defaultFixtureModels.test.ts
    #[test]
    fn the_desk_s_copy_of_this_rule_is_pinned_to_the_same_answers() {
        let cases: [(&str, &[&str], &str); 11] = [
            (
                "blinder",
                &["dimmer", "color.red", "color.green", "color.blue"],
                BLINDER.name,
            ),
            ("profile", &["dimmer"], PROFILE_SPOT.name),
            (
                "profile moving head",
                &["dimmer", "pan", "tilt"],
                MOVING_PROFILE.name,
            ),
            (
                "something new",
                &["dimmer", "pan", "tilt", "gobo.1"],
                MOVING_PROFILE.name,
            ),
            (
                "something new",
                &[
                    "dimmer",
                    "pan",
                    "tilt",
                    "color.cyan",
                    "color.magenta",
                    "color.yellow",
                ],
                MOVING_LED_WASH.name,
            ),
            ("laser", &["dimmer", "pan", "tilt"], SHOW_LASER.name),
            ("hazer", &["fog"], HAZER.name),
            ("scanner", &["dimmer", "pan"], SCANNER.name),
            (
                "strip light",
                &["dimmer", "color.red", "color.green", "color.blue"],
                LED_STRIP.name,
            ),
            ("par", &["dimmer"], PAR_CAN.name),
            ("", &["dimmer"], FRESNEL.name),
        ];
        for (fixture_type, attributes, expected) in cases {
            assert_eq!(
                choose(fixture_type, traits(attributes)).name,
                expected,
                "{fixture_type} with {attributes:?}"
            );
        }
    }

    #[test]
    fn a_type_nobody_shipped_still_lands_on_the_channel_rules() {
        let moving = traits(&["dimmer", "pan", "tilt", "gobo.1"]);
        assert_eq!(choose("something new", moving).name, MOVING_PROFILE.name);
    }

    #[test]
    fn cmy_mixing_counts_as_colour_mixing() {
        let cmy = traits(&[
            "dimmer",
            "pan",
            "tilt",
            "color.cyan",
            "color.magenta",
            "color.yellow",
        ]);
        assert_eq!(choose("", cmy).name, MOVING_LED_WASH.name);
    }
}
