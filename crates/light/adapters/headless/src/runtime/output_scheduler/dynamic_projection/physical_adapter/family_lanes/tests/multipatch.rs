//! TL-548 C3: the optics blocker. A multipatched Focus/Zoom fixture must write its owning control
//! on the root AND every copy, or the engine's all-family native installation rejects the frame.
use super::*;

/// The mixed show with one copy of the Focus/Zoom wash on its own universe (5).
fn multipatched_rig() -> (Rig, FixtureId) {
    let rig = Rig::new();
    let copy = light_fixture::MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        universe: Some(5),
        address: Some(1),
        ..Default::default()
    };
    let copy_id = FixtureId(copy.id);
    let mut fixtures = rig.show.fixtures();
    let optics = fixtures
        .iter_mut()
        .find(|fixture| fixture.fixture_id == rig.show.optics)
        .unwrap();
    optics.multipatch = vec![copy];
    rig.engine
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: fixtures.into(),
            revision: 2,
            ..Default::default()
        })
        .unwrap();
    (rig, copy_id)
}

fn instance(
    published: &PublishedFamilyFrame,
    instance: FixtureId,
) -> &light_engine::PhysicalInstanceOutput {
    published
        .rendered
        .physical
        .instances
        .iter()
        .find(|output| output.instance_id == instance.0)
        .unwrap_or_else(|| panic!("physical instance {instance:?}"))
}

#[test]
fn a_multipatched_focus_zoom_fixture_writes_root_and_copy_through_each_instance_model() {
    let (rig, copy) = multipatched_rig();
    rig.program_all();
    let show = rig.show;
    let lanes = FamilyLanes::live();
    let published = Live::default().accept(&rig, &lanes);
    assert!(published.requirements.is_empty(), "every family is modeled");
    for owner in [ProgrammingOwner::Focus, ProgrammingOwner::Zoom] {
        let row = row(&published.results, show.optics, owner)
            .and_then(FamilySidecar::optics)
            .unwrap_or_else(|| panic!("{owner:?} sidecar"));
        assert_eq!(
            row.writes
                .iter()
                .map(|write| write.slot.destination)
                .collect::<Vec<_>>(),
            [show.optics, copy],
            "{owner:?}: root then copy"
        );
        let [root, copied] = [row.writes[0], row.writes[1]];
        assert_eq!(
            (root.slot.channel_index, root.channel_id, root.raw),
            (copied.slot.channel_index, copied.channel_id, copied.raw),
            "{owner:?}: the mode-level response gives every instance the same raw"
        );
        for write in &row.writes {
            let output = instance(&published, write.slot.destination);
            assert_eq!(
                output.native_raw[write.slot.channel_index as usize], write.raw,
                "{owner:?}: installed on {:?}",
                write.slot.destination
            );
            // The engine evaluates each instance through its own compiled optics model.
            let achieved = match owner {
                ProgrammingOwner::Focus => output.optics()[0].focus.map(|f| f.percent / 100.),
                _ => output.optics()[0].zoom.map(|z| z.degrees),
            };
            assert_eq!(
                achieved, row.achieved,
                "{owner:?} on {:?}",
                write.slot.destination
            );
        }
    }
    // The copy duplicates the root bytes: wash A is Intensity, U16 Zoom, U8 Focus at address 1.
    let (root_bytes, copy_bytes) = (
        &published.rendered.universes[&4],
        &published.rendered.universes[&5],
    );
    assert_eq!(root_bytes[0..4], copy_bytes[0..4]);
    // Every family's writes reach the native output of their own instance (C2 guidance).
    for result in &published.results {
        for write in result.writes() {
            assert_eq!(
                instance(&published, write.slot.destination).native_raw
                    [write.slot.channel_index as usize],
                write.raw,
                "{:?} on {:?}",
                result.owner(),
                result.target()
            );
        }
    }
    assert_eq!(lanes.last_accepted(), every(&published.token));
}
