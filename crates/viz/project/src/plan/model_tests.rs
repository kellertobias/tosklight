use super::*;
use light_core::AttributeKey;
use light_fixture::{
    CanonicalTransform, ChannelBehavior, ChannelResolution, FixtureChannel, GelAssignment,
    ProfileLightSource,
};

/// One patched fixture of a named type, for the optics questions below.
fn patched(fixture_type: &str, optics: ProfileOptics) -> PatchedFixture {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Generic".into();
    profile.name = "Test".into();
    profile.fixture_type = fixture_type.into();
    profile.optics = optics;
    let mode_id = profile.modes[0].id;
    PatchedFixture {
        fixture_id: Uuid::new_v4(),
        name: "Test".into(),
        number: Some(1),
        profile: Arc::new(profile),
        mode_id,
        instances: vec![PhysicalInstance {
            instance_id: Uuid::new_v4(),
            name: "Test".into(),
            split_patches: vec![(1, Some((1, 1)))],
            position: Vec3::new(0.0, 5.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.0,
            shaper_angle: None,
            installed_appearance: InstalledFixtureAppearance::default(),
        }],
    }
}

/// A bank of lamps on one address is a bank of lamps, not one lamp and three dark ones.
///
/// Multi-patch is how an operator says "the same fixture, standing over there as well". The
/// instance has its own position and its own inversions and shares the programming, which the
/// patch expresses by giving it no address of its own. Read as an independent patch that
/// happens to be unaddressed, it decodes nothing and is drawn dark for ever — so four ACLs on
/// one address lit one and left three cold.
#[test]
fn a_multipatch_instance_reads_the_fixture_it_shares_its_programming_with() {
    let mut fixture = patched("par", ProfileOptics::default());
    // A dimmer, so there is something to read. Without a channel every binding is empty and
    // the question this test asks cannot be answered either way.
    let profile = Arc::get_mut(&mut fixture.profile).expect("sole owner");
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 1;
    let head_id = mode.heads[0].id;
    mode.channels = vec![FixtureChannel {
        id: Uuid::new_v4(),
        head_id,
        split: 1,
        fixture_attribute: AttributeKey("intensity".into()),
        attribute: AttributeKey("intensity".into()),
        canonical_transform: CanonicalTransform::Identity,
        resolution: ChannelResolution::U8,
        secondary_slots: Vec::new(),
        default_raw: 0,
        highlight_raw: 255,
        physical_min: Some(0.0),
        physical_max: Some(1.0),
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: false,
        reacts_to_group_master: false,
        reacts_to_grand_master: false,
        behavior: ChannelBehavior::Controlled,
        functions: Vec::new(),
    }];
    let root_addresses = fixture.instances[0].split_patches.clone();
    let mut standing_elsewhere = fixture.instances[0].clone();
    standing_elsewhere.instance_id = Uuid::new_v4();
    standing_elsewhere.name = "Second".into();
    standing_elsewhere.position = Vec3::new(3.0, 5.0, 0.0);
    // What the desk actually sends for a multi-patch instance: a split with no address in it.
    standing_elsewhere.split_patches = vec![(1, None)];
    fixture.instances.push(standing_elsewhere);

    let plan = compile(&[fixture]);

    assert_eq!(plan.scene.fixtures.len(), 2, "both instances are drawn");
    assert!(
        plan.scene.fixtures.iter().all(|fixture| fixture.patched),
        "an instance sharing the fixture's address is patched, not unpatched"
    );
    assert_eq!(plan.bindings.len(), plan.scene.emitters.len());
    assert!(
        plan.bindings
            .iter()
            .all(|binding| binding.universes == vec![1]),
        "every instance reads the universe the fixture is addressed in, got {:?}",
        plan.bindings
            .iter()
            .map(|binding| binding.universes.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        plan.bindings
            .iter()
            .all(|binding| binding.intensity.is_some()),
        "and decodes its dimmer"
    );
    // And the root's own address is untouched by any of this.
    assert_eq!(root_addresses, vec![(1, Some((1, 1)))]);
}

/// The mechanical angles are patch facts, so the projection has to carry them into the scene.
///
/// A rig where every clamp is set at 35 degrees and half the lanterns wear barn doors is drawn
/// hanging straight and square if this seam drops them.
#[test]
fn root_and_multipatch_keep_independent_bracket_and_installed_shaper_angles() {
    let mut fixture = patched("profile", ProfileOptics::default());
    fixture.instances[0].bracket_angle = -35.0;
    fixture.instances[0].shaper_angle = Some(22.5);
    fixture.instances[0]
        .installed_appearance
        .shaper_angles_degrees = [10.0, 20.0, 30.0, 40.0];
    let mut copy = fixture.instances[0].clone();
    copy.instance_id = Uuid::new_v4();
    copy.bracket_angle = 17.0;
    copy.shaper_angle = Some(-12.5);
    copy.installed_appearance.shaper_angles_degrees = [-11.0, -22.0, -33.0, -44.0];
    fixture.instances.push(copy);
    let plan = compile(&[fixture]);
    assert_eq!(plan.scene.fixtures.len(), 2);
    assert_eq!(plan.scene.fixtures[0].bracket_degrees, -35.0);
    assert_eq!(plan.scene.fixtures[0].shaper_degrees, Some(22.5));
    assert_eq!(
        plan.scene.fixtures[0].installed_shaper_angles_degrees,
        [10.0, 20.0, 30.0, 40.0]
    );
    assert_eq!(plan.scene.fixtures[1].bracket_degrees, 17.0);
    assert_eq!(plan.scene.fixtures[1].shaper_degrees, Some(-12.5));
    assert_eq!(
        plan.scene.fixtures[1].installed_shaper_angles_degrees,
        [-11.0, -22.0, -33.0, -44.0]
    );
}

#[test]
fn root_and_multipatch_keep_independent_installed_colours() {
    let mut fixture = patched("profile", ProfileOptics::default());
    fixture.instances[0]
        .installed_appearance
        .color_temperature_kelvin = Some(3_200);
    let mut copy = fixture.instances[0].clone();
    copy.instance_id = Uuid::new_v4();
    copy.installed_appearance.color_temperature_kelvin = Some(10_000);
    copy.installed_appearance.gel = GelAssignment::Custom {
        name: "Red".into(),
        color_srgb: "#FF0000".into(),
        note: None,
    };
    fixture.instances.push(copy);

    let plan = compile(&[fixture]);
    assert_eq!(plan.scene.fixtures.len(), 2);
    assert_ne!(
        plan.scene.fixtures[0].installed_colour,
        plan.scene.fixtures[1].installed_colour
    );
    assert_eq!(plan.scene.fixtures[1].installed_colour[1], 0.0);
    assert_eq!(plan.scene.fixtures[1].installed_colour[2], 0.0);
}

#[test]
fn canonical_cct_identity_aliases_bind_each_physical_channel_once() {
    let mut fixture = patched("wash", ProfileOptics::default());
    let profile = Arc::get_mut(&mut fixture.profile).expect("sole owner");
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 2;
    let head_id = mode.heads[0].id;
    mode.channels = [
        ("color.cold_white", "color.white"),
        ("color.warm_white", "color.amber"),
    ]
    .into_iter()
    .map(|(fixture_attribute, attribute)| FixtureChannel {
        id: Uuid::new_v4(),
        head_id,
        split: 1,
        fixture_attribute: AttributeKey(fixture_attribute.into()),
        attribute: AttributeKey(attribute.into()),
        canonical_transform: CanonicalTransform::Identity,
        resolution: ChannelResolution::U8,
        secondary_slots: Vec::new(),
        default_raw: 0,
        highlight_raw: 255,
        physical_min: Some(0.0),
        physical_max: Some(1.0),
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: false,
        reacts_to_group_master: false,
        reacts_to_grand_master: false,
        behavior: ChannelBehavior::Controlled,
        functions: Vec::new(),
    })
    .collect();

    let plan = compile(&[fixture]);
    let colour = &plan.bindings[0].colour;
    assert!(colour.white.is_some());
    assert!(colour.amber.is_some());
    assert!(colour.cold_white.is_none());
    assert!(colour.warm_white.is_none());
}

#[test]
fn canonical_softness_alias_binds_the_physical_frost_channel_once() {
    let mut fixture = patched("profile", ProfileOptics::default());
    let profile = Arc::get_mut(&mut fixture.profile).expect("sole owner");
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 1;
    let head_id = mode.heads[0].id;
    mode.channels = vec![FixtureChannel {
        id: Uuid::new_v4(),
        head_id,
        split: 1,
        fixture_attribute: AttributeKey("frost".into()),
        attribute: AttributeKey("softness".into()),
        canonical_transform: CanonicalTransform::Identity,
        resolution: ChannelResolution::U8,
        secondary_slots: Vec::new(),
        default_raw: 0,
        highlight_raw: 0,
        physical_min: Some(0.0),
        physical_max: Some(1.0),
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: false,
        reacts_to_group_master: false,
        reacts_to_grand_master: false,
        behavior: ChannelBehavior::Controlled,
        functions: Vec::new(),
    }];

    let plan = compile(&[fixture]);
    assert!(plan.bindings[0].frost.is_some());
}

fn optics_of(fixture: PatchedFixture) -> viz_scene::EmitterOptics {
    let plan = compile(&[fixture]);
    plan.scene
        .emitters
        .first()
        .expect("one emitter")
        .optics
        .clone()
}

/// A library that says nothing about its optics still has to render as the right sort of
/// lantern: the declared fixture type decides, and a profile and a flood differ.
#[test]
fn a_profile_that_declares_no_optics_falls_back_to_its_type() {
    let spot = optics_of(patched("profile", ProfileOptics::default()));
    let flood = optics_of(patched("cyc flood", ProfileOptics::default()));
    assert!(
        spot.sharpness > flood.sharpness + 0.4,
        "a profile cuts and a flood does not: {} against {}",
        spot.sharpness,
        flood.sharpness
    );
    assert!(spot.uniformity > 0.5 && flood.source.form == SourceForm::Rectangular);
}

/// And what a profile does declare wins. This is the point of the block: the library is the
/// authority on the fixture, not the renderer's guess from a type name.
#[test]
fn declared_optics_replace_the_fallback_for_that_fixture() {
    let optics = optics_of(patched(
        // A wash by name, deliberately declared as something else.
        "wash",
        ProfileOptics {
            output: Some(2.5),
            sharpness: Some(0.95),
            uniformity: Some(0.1),
            light_source: Some(ProfileLightSource {
                form: LightSourceForm::Rectangular,
                width_millimetres: 240.0,
                height_millimetres: 60.0,
            }),
        },
    ));
    assert_eq!(optics.output, 2.5);
    assert_eq!(optics.sharpness, 0.95);
    assert_eq!(optics.uniformity, 0.1);
    assert_eq!(optics.source.form, SourceForm::Rectangular);
    assert!((optics.source.width - 0.24).abs() < 1e-6);
    assert!((optics.source.height - 0.06).abs() < 1e-6);
}

/// A declared lens is stated, not guessed, so it is not second-guessed against the body — but
/// a lens the renderer had to invent is kept inside the lantern that carries it.
#[test]
fn an_invented_lens_stays_inside_the_body_and_a_declared_one_is_taken_as_read() {
    let mut small = patched("wash", ProfileOptics::default());
    let profile = Arc::get_mut(&mut small.profile).expect("sole owner");
    profile.physical.width_millimetres = Some(90.0);
    profile.physical.height_millimetres = Some(90.0);
    profile.physical.depth_millimetres = Some(90.0);
    let invented = optics_of(small.clone());
    assert!(
        invented.source.width <= 0.09,
        "an invented lens cannot be wider than the lantern: {}",
        invented.source.width
    );

    let profile = Arc::get_mut(&mut small.profile).expect("sole owner");
    profile.optics.light_source = Some(ProfileLightSource {
        form: LightSourceForm::Round,
        width_millimetres: 300.0,
        height_millimetres: 300.0,
    });
    assert!((optics_of(small).source.width - 0.3).abs() < 1e-6);
}
