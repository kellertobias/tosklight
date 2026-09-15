//! The shipped Box, Cylinder and Ball compile to scenery of their own kind, at the size they are
//! placed at, held to what their profile allows, in the colour chosen for them.
use super::model_tests::shipped_venue;
use super::*;

const PRIMITIVES: [(&str, viz_scene::SceneryKind); 3] = [
    ("venue--box", viz_scene::SceneryKind::Box),
    ("venue--cylinder", viz_scene::SceneryKind::Cylinder),
    ("venue--ball", viz_scene::SceneryKind::Sphere),
];

#[test]
fn a_placed_primitive_is_scenery_of_its_kind_at_its_size_and_colour() {
    for (name, kind) in PRIMITIVES {
        let mut fixture = shipped_venue(name);
        let declared = fixture.profile.scenery.expect(name);
        let unplaced = compile(std::slice::from_ref(&fixture));
        let object = &unplaced.scene.scenery[0];
        assert_eq!(object.kind, kind, "{name}");
        assert!(unplaced.scene.models.is_empty(), "{name} is generated");
        assert_eq!(
            object.size,
            compile_instances::vector(declared.default_size_metres),
            "{name}"
        );
        // Neutral grey until an operator chooses.
        assert!(
            object
                .colour
                .iter()
                .all(|channel| (channel - 0.3).abs() < 1e-6),
            "{name} {:?}",
            object.colour
        );

        fixture.instances[0].scenery_size_metres = Some(Vec3::new(2.0, 3.0, 4.0));
        fixture.instances[0].scenery_options = light_fixture::SceneryOptions {
            colour_srgb: Some("#FF0000".into()),
            ..Default::default()
        };
        let placed = &compile(std::slice::from_ref(&fixture)).scene.scenery[0];
        assert_eq!(placed.size, Vec3::new(2.0, 3.0, 4.0), "{name}");
        assert_eq!(placed.colour, [1.0, 0.0, 0.0], "{name}");

        // Every axis is the operator's, within 0.05 m to 30 m.
        fixture.instances[0].scenery_size_metres = Some(Vec3::new(40.0, 0.01, 12.0));
        let clamped = compile(std::slice::from_ref(&fixture)).scene.scenery[0].size;
        assert_eq!(clamped, Vec3::new(30.0, 0.05, 12.0), "{name}");
    }
}
