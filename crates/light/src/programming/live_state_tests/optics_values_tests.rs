//! TL-558: Focus and Zoom through the shared values service used by HTTP/WS/command surfaces.
//! Each owner is edited, spread, aligned, released and undone independently.
use super::*;
use light_core::OpeningConvention;
use light_core::programming::*;

fn field(degrees: f32) -> AttributeValue {
    AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(degrees),
        convention: OpeningConvention::Field,
    }))
}

fn scalar(component: ProgrammingComponent, operation: ScalarEdit) -> ComponentEdit {
    ComponentEdit::Scalar {
        component,
        operation,
    }
}

struct Optics {
    setup: ValuesSetup,
}

impl Optics {
    fn new() -> Self {
        let mut setup = ValuesSetup::new();
        for fixture in setup.fixtures {
            let current = &mut setup.ports.environment.current_values;
            current.insert((fixture, ProgrammingOwner::Zoom.key()), field(20.));
            current.insert(
                (fixture, ProgrammingOwner::Focus.key()),
                AttributeValue::Normalized(0.2),
            );
        }
        Self { setup }
    }

    fn apply(
        &self,
        request: &str,
        fixtures: Vec<FixtureId>,
        group: Option<&str>,
        owner: ProgrammingOwner,
        operation: ProgrammingValueOperation,
    ) -> Result<ProgrammingValuesResult, crate::ActionError> {
        self.setup.service.handle_values(
            self.setup.action(
                request,
                self.setup.registry.normal_values_revision(),
                ProgrammingValuesCommand::ApplyIntent {
                    intent: ProgrammingValueIntent {
                        fixture_ids: fixtures,
                        group_id: group.map(Into::into),
                        attribute: owner.key(),
                        operation,
                        undo_group: None,
                        timing: Default::default(),
                        displayed_source: None,
                        color_adoption: Default::default(),
                    },
                },
            ),
            &self.setup.ports,
        )
    }

    fn edit(&self, request: &str, fixtures: Vec<FixtureId>, edit: ComponentEdit) {
        let owner = edit.owner();
        self.apply(
            request,
            fixtures,
            None,
            owner,
            ProgrammingValueOperation::ComponentEdits(vec![edit]),
        )
        .unwrap();
    }

    fn fixture(&self, fixture: FixtureId, owner: ProgrammingOwner) -> Option<AttributeValue> {
        let state = self.setup.registry.get(self.setup.session).unwrap();
        state
            .values
            .iter()
            .find(|v| v.fixture_id == fixture && v.attribute == owner.key())
            .map(|v| v.value.clone())
    }

    fn group(&self, fixture: FixtureId, owner: ProgrammingOwner) -> Option<AttributeValue> {
        let state = self.setup.registry.get(self.setup.session).unwrap();
        let value = &state.group_values.get("front")?.get(&owner.key())?.value;
        Some(match value {
            AttributeValue::GroupFamily(assignment) => assignment.for_member(fixture).clone(),
            value => value.clone(),
        })
    }
}

#[test]
fn focus_and_zoom_edit_spread_release_and_undo_as_independent_owners() {
    let desk = Optics::new();
    let [a, b, c] = desk.setup.fixtures;
    // Fixture edits: each owner writes only its own address.
    desk.edit(
        "zoom-a",
        vec![a],
        scalar(
            ProgrammingComponent::Zoom,
            ScalarEdit::Set(ScalarIntent::Value(30.)),
        ),
    );
    desk.edit(
        "focus-a",
        vec![a],
        scalar(
            ProgrammingComponent::Focus,
            ScalarEdit::Set(ScalarIntent::Value(0.6)),
        ),
    );
    assert_eq!(desk.fixture(a, ProgrammingOwner::Zoom), Some(field(30.)));
    assert_eq!(
        desk.fixture(a, ProgrammingOwner::Focus),
        Some(AttributeValue::Normalized(0.6))
    );
    // Spread over the ordered selection [c, a, b]: degrees per rank, Focus untouched.
    desk.edit(
        "zoom-spread",
        vec![c, a, b],
        scalar(
            ProgrammingComponent::Zoom,
            ScalarEdit::Set(ScalarIntent::Spread(vec![10., 40.])),
        ),
    );
    for (fixture, degrees) in [(c, 10.), (a, 25.), (b, 40.)] {
        assert_eq!(
            desk.fixture(fixture, ProgrammingOwner::Zoom),
            Some(field(degrees))
        );
    }
    assert_eq!(
        desk.fixture(a, ProgrammingOwner::Focus),
        Some(AttributeValue::Normalized(0.6))
    );
    assert_eq!(desk.fixture(b, ProgrammingOwner::Focus), None);

    // Per-owner release: Zoom goes, Focus stays; one undo restores exactly that Zoom.
    let depth = desk.setup.registry.undo_depth(desk.setup.session).unwrap();
    desk.setup.handle(
        "release-zoom-a",
        desk.setup.registry.normal_values_revision(),
        ProgrammingValuesCommand::ReleaseFixture {
            fixture_id: a,
            attribute: ProgrammingOwner::Zoom.key(),
        },
    );
    assert_eq!(desk.fixture(a, ProgrammingOwner::Zoom), None);
    assert_eq!(
        desk.fixture(a, ProgrammingOwner::Focus),
        Some(AttributeValue::Normalized(0.6))
    );
    assert_eq!(
        desk.setup.registry.undo_depth(desk.setup.session).unwrap(),
        depth + 1
    );
    assert!(desk.setup.registry.undo(desk.setup.session));
    assert_eq!(desk.fixture(a, ProgrammingOwner::Zoom), Some(field(25.)));
    assert_eq!(
        desk.fixture(a, ProgrammingOwner::Focus),
        Some(AttributeValue::Normalized(0.6))
    );
}

#[test]
fn group_relative_zoom_uses_degrees_and_a_normalized_step_is_rejected() {
    let desk = Optics::new();
    let [a, b, c] = desk.setup.fixtures;
    desk.apply(
        "group-zoom",
        vec![],
        Some("front"),
        ProgrammingOwner::Zoom,
        ProgrammingValueOperation::ComponentEdits(vec![scalar(
            ProgrammingComponent::Zoom,
            ScalarEdit::Relative(5.),
        )]),
    )
    .unwrap();
    for fixture in [a, b, c] {
        assert_eq!(
            desk.group(fixture, ProgrammingOwner::Zoom),
            Some(field(25.))
        );
        assert_eq!(desk.group(fixture, ProgrammingOwner::Focus), None);
    }
    desk.apply(
        "group-focus",
        vec![],
        Some("front"),
        ProgrammingOwner::Focus,
        ProgrammingValueOperation::ComponentEdits(vec![scalar(
            ProgrammingComponent::Focus,
            ScalarEdit::Relative(0.1),
        )]),
    )
    .unwrap();
    let focus = desk.group(a, ProgrammingOwner::Focus).unwrap();
    assert!(
        (focus.normalized().unwrap() - 0.3).abs() < 1e-6,
        "{focus:?}"
    );
    assert_eq!(desk.group(a, ProgrammingOwner::Zoom), Some(field(25.)));
    // A unitless normalized step has no degree meaning: rejected, nothing changes.
    let revision = desk.setup.registry.normal_values_revision();
    let rejected = desk
        .apply(
            "group-zoom-normalized",
            vec![],
            Some("front"),
            ProgrammingOwner::Zoom,
            ProgrammingValueOperation::RelativeStep(0.1),
        )
        .unwrap_err();
    assert_eq!(rejected.kind, ActionErrorKind::Invalid);
    assert_eq!(desk.setup.registry.normal_values_revision(), revision);
    assert_eq!(desk.group(b, ProgrammingOwner::Zoom), Some(field(25.)));
}

#[test]
fn align_ramps_a_relative_zoom_step_without_touching_focus() {
    use light_programmer::ProgrammerAlignmentMode::Left;
    let desk = Optics::new();
    let [a, b, c] = desk.setup.fixtures;
    desk.setup.registry.select(desk.setup.session, [a, b, c]);
    desk.setup
        .service
        .set_alignment(&desk.setup.context, &desk.setup.ports, Some(Left))
        .unwrap();
    desk.edit(
        "align-zoom",
        vec![a, b, c],
        scalar(ProgrammingComponent::Zoom, ScalarEdit::Relative(10.)),
    );
    let zooms: Vec<_> = [a, b, c]
        .map(|fixture| desk.fixture(fixture, ProgrammingOwner::Zoom))
        .into();
    assert_eq!(
        zooms,
        [Some(field(20.)), Some(field(25.)), Some(field(30.))]
    );
    for fixture in [a, b, c] {
        assert_eq!(desk.fixture(fixture, ProgrammingOwner::Focus), None);
    }
}

/// TL-637 follow-up: a Zoom edit needs a seed in degrees for every target. A legacy percentage
/// (no measurable opening was adopted) holds the whole selection quietly: no partial edit, no
/// revision, no Undo step and no error. A target with a degree seed is edited normally.
#[test]
fn a_zoom_edit_without_a_degree_seed_holds_the_whole_selection_quietly() {
    let mut desk = Optics::new();
    let [a, b, _] = desk.setup.fixtures;
    desk.setup.ports.environment.current_values.insert(
        (b, ProgrammingOwner::Zoom.key()),
        AttributeValue::Normalized(0.5),
    );
    let revision = desk.setup.registry.normal_values_revision();
    let held = desk
        .apply(
            "zoom-unseeded",
            vec![a, b],
            None,
            ProgrammingOwner::Zoom,
            ProgrammingValueOperation::ComponentEdits(vec![scalar(
                ProgrammingComponent::Zoom,
                ScalarEdit::Relative(1.),
            )]),
        )
        .expect("a quiet hold, never an error");
    assert_eq!(held.hold, Some(ProgrammingValuesHold::ZoomUnavailable));
    assert_eq!(desk.setup.registry.normal_values_revision(), revision);
    assert_eq!(desk.fixture(a, ProgrammingOwner::Zoom), None);
    assert_eq!(desk.fixture(b, ProgrammingOwner::Zoom), None);

    desk.edit(
        "zoom-seeded",
        vec![a],
        scalar(ProgrammingComponent::Zoom, ScalarEdit::Relative(1.)),
    );
    assert_eq!(desk.fixture(a, ProgrammingOwner::Zoom), Some(field(21.)));
}
