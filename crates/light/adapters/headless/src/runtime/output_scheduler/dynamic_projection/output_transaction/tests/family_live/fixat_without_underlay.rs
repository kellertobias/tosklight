//! TL-552: a whole-family FixAT is the operator's complete intent. It renders through the same
//! family output path whether or not a static value of the same owner sits underneath it
//! (`FIXAT COLOR PRESET 1` straight after Clear). Precedence and timing are unchanged.
use super::*;
use light_core::programming::ProgrammingComponent;
use light_dynamics::{DynamicValueTiming, ProgrammingFamilyFixAt};

fn fix_only(
    bench: &Bench,
    target: FixtureId,
    owner: ProgrammingOwner,
    component: Option<ProgrammingComponent>,
    value: AttributeValue,
    timing: DynamicValueTiming,
) {
    bench.clock.advance_millis(10);
    assert!(bench.programmers.apply_dynamic_values(
        bench.session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: target,
            attribute: owner.key(),
            value: DynamicSemanticValue::ProgrammingFixAt {
                mask: ProgrammingFamilyFixAt::from_family(owner, component, value).unwrap(),
                timing,
            },
        }],
        None,
    ));
}

/// The FixAT of each family with no underlay, and the same FixAT over a different underlay.
fn benches() -> (Show, Bench, Bench) {
    let (show, definition) = (Show::new(), definition());
    let bare = Bench::new(&show, &definition, PROGRAMMING_CONTRACT_VERSION, true);
    let over = Bench::new(&show, &definition, PROGRAMMING_CONTRACT_VERSION, true);
    let cyan = program(&intent([0., 1., 1.], 0.));
    let masks = [
        (
            show.mover,
            ProgrammingOwner::Position,
            angles(10., 20.),
            angles(450., 30.),
        ),
        (
            show.wash,
            ProgrammingOwner::Color,
            cyan,
            program(&magenta()),
        ),
        (
            show.optics,
            ProgrammingOwner::Focus,
            focus(0.2),
            focus(0.75),
        ),
        (show.optics, ProgrammingOwner::Zoom, field(30.), field(20.)),
    ];
    for (target, owner, underlay, value) in masks {
        fix_only(
            &bare,
            target,
            owner,
            None,
            value.clone(),
            Default::default(),
        );
        fix(
            &over.programmers,
            &over.clock,
            over.session,
            target,
            owner,
            underlay,
            value,
        );
    }
    (show, bare, over)
}

fn owners(bench: &Bench) -> Vec<(ProgrammingOwner, FixtureId)> {
    let published = bench.family.take_published().expect("a published frame");
    let mut owners = published
        .writes
        .iter()
        .map(|(owner, target, _)| (*owner, *target))
        .collect::<Vec<_>>();
    owners.sort_by_key(|(owner, target)| (owner.key().0.to_string(), target.0));
    owners.dedup();
    owners
}

#[test]
fn a_whole_family_fixat_without_an_underlay_renders_exactly_like_one_over_an_underlay() {
    let (show, bare, over) = benches();
    let (bare_frame, over_frame) = (bare.frame(), over.frame());
    assert!(bare_frame.hybrid && over_frame.hybrid);
    let mut expected = show.owners();
    expected.retain(|(_, target)| *target != show.layer);
    assert_eq!(
        owners(&bare),
        expected,
        "Position, Color, Focus and Zoom fitted"
    );
    assert_eq!(owners(&over), expected);
    // Mover, RGB wash and Focus/Zoom wash: identical bytes, so the mask (not the underlay or a
    // default) is what the family path rendered in both cases.
    for universe in [1, 2, 4] {
        assert_eq!(
            bare_frame.rendered.universes[&universe], over_frame.rendered.universes[&universe],
            "universe {universe}"
        );
    }
    assert_ne!(
        bare_frame.rendered.universes[&2][..5],
        [0; 5],
        "the wash is lit"
    );
    // The accepted-frame colour report has the wash's row, as it has over an underlay.
    let reported = |bench: &Bench| {
        let accepted = bench
            .family
            .latest_accepted_color()
            .expect("accepted Color");
        let row = accepted.heads.iter().find(|row| row.target == show.wash);
        row.map(|row| format!("{:?}", row.quality))
    };
    assert!(reported(&bare).is_some(), "the bare FixAT has a report row");
    assert_eq!(reported(&bare), reported(&over));
    // It stays rendered on the following frames.
    assert_eq!(
        bare.frame().rendered.universes[&2],
        over.frame().rendered.universes[&2]
    );
}

#[test]
fn a_bare_fixat_keeps_its_delay_and_a_component_mask_still_needs_an_underlay() {
    let (show, definition) = (Show::new(), definition());
    let bench = Bench::new(&show, &definition, PROGRAMMING_CONTRACT_VERSION, true);
    let delayed = DynamicValueTiming {
        delay_millis: Some(1_000),
        ..Default::default()
    };
    let magenta = program(&magenta());
    fix_only(
        &bench,
        show.wash,
        ProgrammingOwner::Color,
        None,
        magenta,
        delayed,
    );
    // Pan alone cannot complete a Position family: it stays a requirement, nothing is invented.
    fix_only(
        &bench,
        show.mover,
        ProgrammingOwner::Position,
        Some(ProgrammingComponent::Pan),
        angles(450., 30.),
        Default::default(),
    );
    let before = bench.frame();
    assert!(
        owners(&bench).is_empty(),
        "nothing is fitted during the delay"
    );
    assert_eq!(
        before.rendered.universes[&2][..5],
        [0; 5],
        "dark during the delay"
    );
    bench.clock.advance_millis(1_000);
    let after = bench.frame();
    assert_eq!(owners(&bench), vec![(ProgrammingOwner::Color, show.wash)]);
    assert_ne!(after.rendered.universes[&2][..5], [0; 5]);
}
