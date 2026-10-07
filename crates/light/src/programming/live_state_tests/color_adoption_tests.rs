//! TL-554: Color adoption of the first sample of a Color gesture, through the real values
//! service on both lanes.
//! - The first semantic edit of a Direct value adopts its modelled appearance and UV once and
//!   reports the starting value (approximate, or explicit). Known black stays black.
//! - An unknown visible appearance is never invented (never white): the action holds with
//!   `ExplicitColorStartRequired` until the operator supplies an explicit starting colour.
//! - A native edit without a pinned model and Direct seed holds (`NativeColorUnavailable`).
//!
//! Holds create no revision, no Undo step and no retained capture.
use super::*;
use crate::programming::semantic_intent_cases::{color, magenta};
use crate::{
    ProgrammingColorAdoption, ProgrammingColorAdoptionFixture, ProgrammingColorAdoptionStart,
    ProgrammingValuesHold,
};
use light_core::programming::{
    ColorIntent, ColorProgram, NativeColorComponentDescriptor, NativeColorEdit,
    NativeColorEditModel, NativeColorRecipe, PortableColorEstimate, PortableUv,
    PortableVisibleColor,
};
use light_core::{
    NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality, Xyz,
};

fn source() -> NativeColorIdentity {
    NativeColorIdentity {
        profile_id: Uuid::from_u128(0x554_01),
        profile_revision: 1,
        profile_digest: "tl554-digest".into(),
        mode_id: Uuid::from_u128(0x554_02),
        head_id: Uuid::from_u128(0x554_03),
        path_id: Uuid::from_u128(0x554_04),
        model_revision: 1,
        native_layout_signature: "tl554-layout".into(),
    }
}

fn binding() -> NativeColorBinding {
    NativeColorBinding {
        channel_id: Uuid::from_u128(0x554_10),
        function_id: Uuid::from_u128(0x554_20),
    }
}

fn direct(visible: Option<Xyz>, uv: Option<f32>, raw: u32) -> AttributeValue {
    let program = ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: source(),
            channels: vec![NativeColorValue {
                channel_id: binding().channel_id,
                function_id: binding().function_id,
                raw,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 1,
            visible: visible.map(|xyz| PortableVisibleColor {
                xyz,
                relative_output: 1.0,
            }),
            uv: uv.map(|amount| PortableUv {
                amount,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        },
    };
    program.validate().unwrap();
    AttributeValue::ColorProgram(Arc::new(program))
}

/// A pinned one-channel model: its forward estimate is `raw / 1000` grey, UV unknown.
struct Model;
impl NativeColorEditModel for Model {
    fn source(&self) -> &NativeColorIdentity {
        static SOURCE: std::sync::OnceLock<NativeColorIdentity> = std::sync::OnceLock::new();
        SOURCE.get_or_init(source)
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding == self::binding()).then_some(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: 65_535,
            continuous: true,
        })
    }
    fn predict(
        &self,
        recipe: &NativeColorRecipe,
    ) -> Result<PortableColorEstimate, light_core::programming::IntentError> {
        let level = recipe.channels[0].raw as f32 / 1000.0;
        Ok(PortableColorEstimate {
            model_revision: 1,
            visible: Some(PortableVisibleColor {
                xyz: Xyz {
                    x: level,
                    y: level,
                    z: level,
                },
                relative_output: 1.0,
            }),
            uv: None,
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        })
    }
}

/// The values ports plus a model catalogue and, optionally, a captured native seed.
struct ColorPorts<'a> {
    inner: &'a ValuesPorts,
    model: bool,
    seed: Option<AttributeValue>,
}

impl ProgrammingPorts for ColorPorts<'_> {
    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        _context: &ActionContext,
        _command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        panic!("Color adoption does not execute commands")
    }
    fn values_environment(
        &self,
        context: &ActionContext,
    ) -> Result<ProgrammingValuesEnvironment, crate::ActionError> {
        self.inner.values_environment(context)
    }
    fn prepare_family_edit_context(
        &self,
        _context: &ActionContext,
        _preload: bool,
        intent: &ProgrammingValueIntent,
        environment: &mut ProgrammingValuesEnvironment,
    ) -> Result<(), crate::ActionError> {
        if let Some(seed) = &self.seed {
            for fixture in &intent.fixture_ids {
                let context = environment.family_contexts.entry(*fixture).or_default();
                context.native_model = Some(Arc::new(Model));
                context.direct_color_seed = Some(seed.clone());
            }
        }
        Ok(())
    }
    fn native_color_model(
        &self,
        _context: &ActionContext,
        source: &NativeColorIdentity,
    ) -> Option<Arc<dyn NativeColorEditModel + Send + Sync>> {
        (self.model && *source == self::source())
            .then(|| Arc::new(Model) as Arc<dyn NativeColorEditModel + Send + Sync>)
    }
    fn persist(&self, context: &ActionContext, operation: &'static str) -> Option<String> {
        self.inner.persist(context, operation)
    }
    fn reconcile(&self, _context: &ActionContext, _reason: ProgrammingReconciliation) {}
    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        Ok(None)
    }
}

impl GestureDesk {
    fn seed_color(&mut self, fixture: FixtureId, value: AttributeValue) {
        self.setup
            .ports
            .environment
            .current_values
            .insert((fixture, ProgrammingOwner::Color.key()), value);
    }

    fn color_intent(&self, edits: Vec<ComponentEdit>, caller: &str) -> ProgrammingValueIntent {
        ProgrammingValueIntent {
            fixture_ids: vec![self.setup.fixtures[0]],
            group_id: None,
            attribute: ProgrammingOwner::Color.key(),
            operation: ProgrammingValueOperation::ComponentEdits(edits),
            undo_group: Some(caller.into()),
            timing: Default::default(),
            displayed_source: None,
            color_adoption: Default::default(),
        }
    }

    /// `(hold, adoption, revision)` of one Color action on this desk's lane.
    fn apply_color(
        &self,
        request: &str,
        intent: ProgrammingValueIntent,
        ports: &dyn ProgrammingPorts,
    ) -> (
        Option<ProgrammingValuesHold>,
        Option<ProgrammingColorAdoption>,
        u64,
    ) {
        let registry = &self.setup.registry;
        let capture = registry.capture_mode_revision();
        if self.preload {
            let result = self
                .setup
                .service
                .handle_preload_values(
                    ActionEnvelope {
                        context: self
                            .setup
                            .context
                            .clone()
                            .with_request_id(request)
                            .with_expected_revision(registry.preload_values_revision()),
                        command: ProgrammingPreloadValuesRequest {
                            expected_capture_mode_revision: capture,
                            command: ProgrammingPreloadValuesCommand::ApplyIntent { intent },
                        },
                    },
                    ports,
                )
                .unwrap();
            (
                result.hold,
                result.color_adoption,
                result.outcome.revision(),
            )
        } else {
            let result = self
                .setup
                .service
                .handle_values(
                    self.setup.action_with_capture(
                        request,
                        registry.normal_values_revision(),
                        capture,
                        ProgrammingValuesCommand::ApplyIntent { intent },
                    ),
                    ports,
                )
                .unwrap();
            (
                result.hold,
                result.color_adoption,
                result.outcome.revision(),
            )
        }
    }

    fn color_value(&self) -> Option<AttributeValue> {
        let state = self.setup.registry.get(self.setup.session).unwrap();
        let values = if self.preload {
            &state.preload_pending
        } else {
            state.values.as_ref()
        };
        values
            .iter()
            .find(|v| {
                v.fixture_id == self.setup.fixtures[0]
                    && v.attribute == ProgrammingOwner::Color.key()
            })
            .map(|v| v.value.clone())
    }
}

fn white_blend(value: f32) -> ComponentEdit {
    ComponentEdit::Scalar {
        component: ProgrammingComponent::Color(ColorComponent::WhiteBlend),
        operation: ScalarEdit::Set(ScalarIntent::Value(value)),
    }
}

fn semantic(value: &AttributeValue) -> &ColorIntent {
    match value {
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Semantic { intent } => intent,
            ColorProgram::Direct { .. } => panic!("expected Semantic"),
        },
        _ => panic!("expected a Color program"),
    }
}

const BLACK: Xyz = Xyz {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};
const D65: Xyz = light_core::color_intent::D65_WHITE;

#[test]
fn unknown_direct_appearance_holds_for_an_explicit_start_and_is_never_white() {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let fixture = desk.setup.fixtures[0];
        // UV-only Direct recipe: visible appearance unknown, UV known.
        desk.seed_color(fixture, direct(None, Some(0.4), 9));
        let ports = ColorPorts {
            inner: &desk.setup.ports,
            model: false,
            seed: None,
        };
        let depth = desk.depth();
        let intent = desk.color_intent(vec![white_blend(0.25)], "touch");
        let (hold, adoption, revision) = desk.apply_color("held", intent.clone(), &ports);
        assert_eq!(
            hold,
            Some(ProgrammingValuesHold::ExplicitColorStartRequired)
        );
        assert_eq!(adoption, None);
        assert_eq!(desk.color_value(), None, "nothing was invented or written");
        assert_eq!(desk.depth(), depth, "a hold creates no Undo step");

        // The operator's explicit start (black) is adopted; known UV still replaces its UV.
        let mut explicit = intent.clone();
        explicit.color_adoption.explicit_start = Some(ColorIntent {
            base_xyz: BLACK,
            recipe: light_core::programming::VirtualColorRecipe {
                rgb: [0.0; 3],
                ..Default::default()
            },
            ..ColorIntent::default()
        });
        let (hold, adoption, after) = desk.apply_color("explicit", explicit, &ports);
        assert_eq!(hold, None);
        assert!(after > revision);
        let adoption = adoption.expect("the adoption is reported with its sample");
        assert_eq!(
            adoption.fixtures,
            vec![ProgrammingColorAdoptionFixture {
                fixture_id: fixture,
                start: ProgrammingColorAdoptionStart::Explicit,
                uv_unknown: false,
            }]
        );
        assert!(
            !adoption.limitations.is_empty(),
            "unknown appearance is reported"
        );
        let value = desk.color_value().unwrap();
        let adopted = semantic(&value);
        assert_eq!(adopted.base_xyz, BLACK, "never white");
        assert_ne!(adopted.base_xyz, D65);
        assert_eq!(adopted.uv.amount, 0.4, "known UV adopted independently");
        assert_eq!(adopted.white_blend, 0.25, "the edit applied once");
    }
}

#[test]
fn known_direct_appearance_adopts_the_modelled_estimate_once_and_reports_it_approximate() {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let fixture = desk.setup.fixtures[0];
        let dim = Xyz {
            x: 0.11,
            y: 0.07,
            z: 0.02,
        };
        desk.seed_color(fixture, direct(Some(dim), None, 250));
        // Without the original model the recorded estimate is used.
        let recorded = ColorPorts {
            inner: &desk.setup.ports,
            model: false,
            seed: None,
        };
        let (hold, adoption, _) = desk.apply_color(
            "recorded",
            desk.color_intent(vec![white_blend(0.5)], "a"),
            &recorded,
        );
        assert_eq!(hold, None);
        let adoption = adoption.unwrap();
        assert_eq!(
            adoption.fixtures[0].start,
            ProgrammingColorAdoptionStart::Approximate
        );
        assert!(adoption.fixtures[0].uv_unknown, "unknown UV is adopted off");
        let value = desk.color_value().unwrap();
        let adopted = semantic(&value);
        assert_eq!(
            adopted.base_xyz, dim,
            "the total estimate, never normalized"
        );
        assert!(adopted.recipe.approximate);
        assert_eq!((adopted.white_blend, adopted.uv.amount), (0.5, 0.0));
        // A second sample of the same gesture edits the adopted value; nothing re-adopts.
        let (_, again, _) = desk.apply_color(
            "second",
            desk.color_intent(vec![white_blend(0.6)], "a"),
            &recorded,
        );
        assert_eq!(again, None, "adoption is reported once");
        assert_eq!(semantic(&desk.color_value().unwrap()).base_xyz, dim);

        // With the original model the estimate is its forward evaluation of the recipe.
        let mut desk = GestureDesk::new(preload);
        desk.seed_color(fixture_of(&desk), direct(Some(dim), None, 250));
        let forward = ColorPorts {
            inner: &desk.setup.ports,
            model: true,
            seed: None,
        };
        desk.apply_color(
            "forward",
            desk.color_intent(vec![white_blend(0.0)], "b"),
            &forward,
        );
        assert_eq!(
            semantic(&desk.color_value().unwrap()).base_xyz,
            Xyz {
                x: 0.25,
                y: 0.25,
                z: 0.25
            }
        );
    }
}

fn fixture_of(desk: &GestureDesk) -> FixtureId {
    desk.setup.fixtures[0]
}

fn native(operation: NativeColorEdit) -> ComponentEdit {
    ComponentEdit::Native {
        binding: binding(),
        operation,
    }
}

#[test]
fn a_native_edit_adopts_its_captured_seed_once_and_holds_without_one() {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let fixture = fixture_of(&desk);
        desk.seed_color(fixture, color(magenta()));
        let without = ColorPorts {
            inner: &desk.setup.ports,
            model: false,
            seed: None,
        };
        let depth = desk.depth();
        let (hold, _, _) = desk.apply_color(
            "no-model",
            desk.color_intent(vec![native(NativeColorEdit::Relative(5))], "n0"),
            &without,
        );
        assert_eq!(hold, Some(ProgrammingValuesHold::NativeColorUnavailable));
        assert_eq!(desk.color_value(), None);
        assert_eq!(desk.depth(), depth);

        // The captured premaster seed (raw 100) is adopted once; later samples edit in place.
        let ports = ColorPorts {
            inner: &desk.setup.ports,
            model: true,
            seed: Some(direct(Some(BLACK), Some(0.0), 100)),
        };
        for (sample, expected) in [("first", 105), ("second", 110), ("third", 115)] {
            let (hold, _, _) = desk.apply_color(
                sample,
                desk.color_intent(vec![native(NativeColorEdit::Relative(5))], "turn"),
                &ports,
            );
            assert_eq!(hold, None);
            let AttributeValue::ColorProgram(program) = desk.color_value().unwrap() else {
                panic!("Color program")
            };
            let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
                panic!("the first native edit selects Direct")
            };
            assert_eq!(recipe.channels[0].raw, expected, "{sample}: seeded once");
            assert_eq!(recipe.source, source());
            let level = expected as f32 / 1000.0;
            assert_eq!(
                portable.visible.unwrap().xyz.y,
                level,
                "predicted by the pinned model"
            );
        }
        assert_eq!(desk.depth(), depth + 1, "one Undo group for the gesture");
        // A new gesture on the Direct value edits in place too: the seed is never reused.
        desk.apply_color(
            "new-gesture",
            desk.color_intent(vec![native(NativeColorEdit::Relative(-10))], "turn-2"),
            &ports,
        );
        let AttributeValue::ColorProgram(program) = desk.color_value().unwrap() else {
            unreachable!()
        };
        let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
            unreachable!()
        };
        assert_eq!(
            recipe.channels[0].raw, 105,
            "in place, never reseeded to 90"
        );
    }
}

impl GestureDesk {
    fn group_color_intent(
        &self,
        operation: ProgrammingValueOperation,
        caller: &str,
    ) -> ProgrammingValueIntent {
        ProgrammingValueIntent {
            fixture_ids: vec![],
            group_id: Some("front".into()),
            attribute: ProgrammingOwner::Color.key(),
            operation,
            undo_group: Some(caller.into()),
            timing: Default::default(),
            displayed_source: None,
            color_adoption: Default::default(),
        }
    }

    fn group_color(&self, fixture: FixtureId) -> AttributeValue {
        let state = self.setup.registry.get(self.setup.session).unwrap();
        let groups = if self.preload {
            &state.preload_group_pending
        } else {
            &state.group_values
        };
        match &groups["front"][&ProgrammingOwner::Color.key()].value {
            AttributeValue::GroupFamily(assignment) => assignment.for_member(fixture).clone(),
            value => value.clone(),
        }
    }
}

/// G5: a Direct value stored on the Group itself (not per fixture) is adopted by the first
/// semantic edit of that Group, on both lanes, exactly as a per-fixture Direct value is.
#[test]
fn a_group_stored_direct_value_is_adopted_by_the_first_semantic_group_edit() {
    for preload in [false, true] {
        let desk = GestureDesk::new(preload);
        let dim = Xyz {
            x: 0.11,
            y: 0.07,
            z: 0.02,
        };
        let ports = ColorPorts {
            inner: &desk.setup.ports,
            model: false,
            seed: None,
        };
        let (hold, _, _) = desk.apply_color(
            "store-direct",
            desk.group_color_intent(
                ProgrammingValueOperation::AbsoluteSet(direct(Some(dim), None, 250)),
                "store",
            ),
            &ports,
        );
        assert_eq!(hold, None);
        let (hold, adoption, _) = desk.apply_color(
            "semantic",
            desk.group_color_intent(
                ProgrammingValueOperation::ComponentEdits(vec![white_blend(0.5)]),
                "turn",
            ),
            &ports,
        );
        assert_eq!(hold, None, "preload={preload}");
        let adoption = adoption.expect("the Group's Direct value is adopted once");
        assert_eq!(adoption.fixtures.len(), desk.setup.fixtures.len());
        assert!(
            adoption
                .fixtures
                .iter()
                .all(|fixture| fixture.start == ProgrammingColorAdoptionStart::Approximate)
        );
        for fixture in desk.setup.fixtures {
            let value = desk.group_color(fixture);
            let adopted = semantic(&value);
            assert_eq!(adopted.base_xyz, dim, "preload={preload}");
            assert_eq!(adopted.white_blend, 0.5);
        }
    }
}
