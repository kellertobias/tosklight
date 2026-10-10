use super::*;
use crate::ActionSource;
use light_core::{NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Ports {
    snapshot: Arc<EngineSnapshot>,
    environment: DynamicFixAtEnvironment,
    captures: AtomicUsize,
    supported: u16,
    native: Option<Arc<NativeModel>>,
}
impl DynamicsPorts for Ports {
    fn authorize(&self, _: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }
    fn supported_programming_contract(&self) -> u16 {
        self.supported
    }
    fn snapshot(&self) -> Arc<EngineSnapshot> {
        self.snapshot.clone()
    }
    fn fix_at_environment(
        &self,
        _: &ActionContext,
        _: &[FixtureId],
        _: ProgrammingOwner,
    ) -> Result<DynamicFixAtEnvironment, ActionError> {
        self.captures.fetch_add(1, Ordering::SeqCst);
        Ok(self.environment.clone())
    }
    fn fix_at_native_model(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, ActionError> {
        self.native
            .clone()
            .filter(|model| &model.identity == source)
            .map(|model| model as Arc<dyn NativeColorEditModel + Send + Sync>)
            .ok_or_else(|| invalid("original source unavailable"))
    }
    fn now_millis(&self) -> u64 {
        10
    }
    fn runtime_controller_is_completed(&self, _: Uuid) -> bool {
        unreachable!()
    }
    fn runtime_controller_instance(&self, _: Uuid) -> Option<Uuid> {
        unreachable!()
    }
    fn reconcile_programmer_runtime(&self) {
        unreachable!()
    }
    fn start_runtime(&self, _: DynamicStartRequest) -> Result<Uuid, DynamicRuntimeError> {
        unreachable!()
    }
    fn off_runtime_controller(
        &self,
        _: Uuid,
        _: u64,
        _: u64,
        _: u64,
    ) -> Result<(Uuid, bool), DynamicRuntimeError> {
        unreachable!()
    }
    fn update_runtime_controller(
        &self,
        _: Uuid,
        _: Option<f32>,
        _: Option<f32>,
        _: Option<f32>,
    ) -> Result<(), DynamicRuntimeError> {
        unreachable!()
    }
    fn publish_runtime_change(&self, _: &ActionContext, _: crate::DynamicRuntimeChange) {
        unreachable!()
    }
}

struct Desk {
    service: DynamicsService,
    context: ActionContext,
    ports: Ports,
    targets: [FixtureId; 2],
}
impl Desk {
    fn new() -> Self {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        registry.start(session);
        let targets = [FixtureId::new(), FixtureId::new()];
        // Unpatched fixtures without physical Position/Color channels are still authorable.
        let fixtures = targets
            .iter()
            .map(|id| {
                serde_json::from_value(serde_json::json!({
                    "fixture_id": id,
                    "definition": {"schema_version":1,"id":id,"revision":1,"manufacturer":"Test",
                        "model":"Different destination", "mode":"None", "footprint":1,"heads":[],
                        "color_calibration":null,"hazardous":false,"safe_values":{}}
                }))
                .unwrap()
            })
            .collect::<Vec<_>>();
        Self {
            service: DynamicsService::new(registry),
            context: ActionContext::operator(Uuid::new_v4(), session.0, ActionSource::Keyboard),
            targets,
            ports: Ports {
                snapshot: Arc::new(EngineSnapshot {
                    fixtures: Arc::new(fixtures),
                    ..Default::default()
                }),
                environment: DynamicFixAtEnvironment::default(),
                captures: AtomicUsize::new(0),
                supported: 1,
                native: None,
            },
        }
    }
    fn session(&self) -> SessionId {
        SessionId(self.context.session_id.unwrap())
    }
    fn command(
        &self,
        owner: ProgrammingOwner,
        component: Option<ProgrammingComponent>,
    ) -> DynamicFixAtCaptureCommand {
        DynamicFixAtCaptureCommand {
            targets: self.targets.to_vec(),
            owner,
            component,
            edits: vec![],
            timing: Default::default(),
        }
    }
    fn capture(&self, command: DynamicFixAtCaptureCommand) -> Result<usize, ActionError> {
        self.service
            .fix_at_capture(&self.context, command, &self.ports)
    }
    fn batch(&self, values: Vec<DynamicFixAtValue>) -> Result<usize, ActionError> {
        self.service.fix_at_batch(
            &self.context,
            DynamicFixAtBatchCommand {
                values,
                timing: Default::default(),
            },
            &self.ports,
        )
    }
    fn masks(&self) -> Vec<ProgrammingFamilyFixAt> {
        self.service
            .programmers
            .get(self.session())
            .unwrap()
            .dynamic_values
            .iter()
            .map(|value| {
                let DynamicSemanticValue::ProgrammingFixAt { mask, .. } = &value.value else {
                    panic!("typed mask required")
                };
                mask.clone()
            })
            .collect()
    }
    fn depth(&self) -> usize {
        self.service.programmers.undo_depth(self.session()).unwrap()
    }
}
fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}

#[test]
fn capture_current_keeps_each_complete_pair_and_is_one_undo_without_static_assignments() {
    let mut desk = Desk::new();
    for (index, target) in desk.targets.iter().enumerate() {
        desk.ports
            .environment
            .families
            .insert(*target, angles(450.0, 10.0 + index as f32));
    }
    let command = desk.command(ProgrammingOwner::Position, Some(ProgrammingComponent::Pan));
    let before = desk.depth();
    assert_eq!(desk.capture(command.clone()).unwrap(), 2);
    assert_eq!(desk.depth(), before + 1);
    assert_eq!(desk.ports.captures.load(Ordering::SeqCst), 1);
    for (index, mask) in desk.masks().iter().enumerate() {
        assert_eq!(mask.family, angles(450.0, 10.0 + index as f32));
        assert_eq!(mask.address.component, Some(ProgrammingComponent::Pan));
    }
    assert!(
        desk.service
            .programmers
            .get(desk.session())
            .unwrap()
            .values
            .is_empty()
    );
    assert_eq!(desk.capture(command).unwrap(), 0);
    assert_eq!(desk.depth(), before + 1);
    assert!(desk.service.programmers.undo(desk.session()));
    assert!(desk.masks().is_empty());
}

#[test]
fn target_whole_hold_keeps_reference_but_pan_hold_adopts_solved_unwrapped_pair() {
    let mut desk = Desk::new();
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::new_v4(),
        },
        [1.0, 2.0, 3.0],
    )));
    for fixture in desk.targets {
        desk.ports
            .environment
            .families
            .insert(fixture, target.clone());
        desk.ports.environment.contexts.insert(
            fixture,
            OwnedFamilyEditContext {
                solved_angles: Some(JointAngles {
                    pan_degrees: 810.0,
                    tilt_degrees: -25.0,
                }),
                ..Default::default()
            },
        );
    }
    desk.capture(desk.command(ProgrammingOwner::Position, None))
        .unwrap();
    assert!(desk.masks().iter().all(|mask| mask.family == target));
    desk.service.programmers.undo(desk.session());
    let mut command = desk.command(ProgrammingOwner::Position, Some(ProgrammingComponent::Pan));
    command.edits.push(ComponentEdit::Scalar {
        component: ProgrammingComponent::Pan,
        operation: ScalarEdit::Set(ScalarIntent::Value(900.0)),
    });
    desk.capture(command).unwrap();
    assert!(
        desk.masks()
            .iter()
            .all(|mask| mask.family == angles(900.0, -25.0))
    );
}

#[test]
fn failed_second_target_capture_leaves_first_target_selection_and_undo_untouched() {
    let mut desk = Desk::new();
    desk.ports
        .environment
        .families
        .insert(desk.targets[0], angles(30.0, 40.0));
    let before = desk.depth();
    let selection = desk.service.programmers.selection(desk.session()).unwrap();
    assert!(
        desk.capture(desk.command(ProgrammingOwner::Position, Some(ProgrammingComponent::Pan)))
            .is_err()
    );
    assert!(desk.masks().is_empty());
    assert_eq!(desk.depth(), before);
    assert_eq!(
        desk.service.programmers.selection(desk.session()).unwrap(),
        selection
    );
}

#[test]
fn empty_selection_is_quiet_before_contract_or_capture_and_focus_mask_is_gated() {
    let mut desk = Desk::new();
    desk.ports.supported = 0;
    let before = desk.depth();
    let mut command = desk.command(ProgrammingOwner::Focus, None);
    command.targets.clear();
    assert_eq!(desk.capture(command).unwrap(), 0);
    assert_eq!(desk.ports.captures.load(Ordering::SeqCst), 0);
    let mask = ProgrammingFamilyFixAt::from_family(
        ProgrammingOwner::Focus,
        None,
        AttributeValue::Normalized(0.4),
    )
    .unwrap();
    let value = DynamicFixAtValue::programming(desk.targets[0], mask);
    assert!(
        desk.batch(vec![value])
            .unwrap_err()
            .message
            .contains("programming contract 1")
    );
    assert!(
        desk.capture(desk.command(ProgrammingOwner::Focus, None))
            .unwrap_err()
            .message
            .contains("programming contract 1")
    );
    assert_eq!(desk.depth(), before);
    assert_eq!(desk.ports.captures.load(Ordering::SeqCst), 0);
}

#[test]
fn preset_batch_auto_wraps_rich_families_and_allows_distinct_component_masks() {
    let desk = Desk::new();
    let value = DynamicFixAtValue {
        fixture_id: desk.targets[0],
        attribute: ProgrammingOwner::Position.key(),
        value: angles(720.0, 15.0),
        programming_mask: None,
    };
    assert_eq!(desk.batch(vec![value.clone()]).unwrap(), 1);
    assert_eq!(desk.masks()[0].address.component, None);
    assert_eq!(desk.batch(vec![value]).unwrap(), 0);
    desk.service.programmers.undo(desk.session());
    let values = [ProgrammingComponent::Pan, ProgrammingComponent::Tilt].map(|component| {
        DynamicFixAtValue::programming(
            desk.targets[0],
            ProgrammingFamilyFixAt::from_family(
                ProgrammingOwner::Position,
                Some(component),
                angles(720.0, 15.0),
            )
            .unwrap(),
        )
    });
    assert_eq!(desk.batch(values.to_vec()).unwrap(), 2);
    assert_eq!(desk.masks().len(), 2);
    let before = desk.depth();
    assert!(
        desk.batch(vec![values[0].clone(), values[0].clone()])
            .is_err()
    );
    assert_eq!(desk.depth(), before);
}

struct NativeModel {
    identity: NativeColorIdentity,
    binding: NativeColorBinding,
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.identity
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding == self.binding).then_some(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: u32::MAX,
            continuous: true,
        })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        if recipe.source != self.identity
            || recipe.channels.len() != 1
            || recipe.channels[0].channel_id != self.binding.channel_id
            || recipe.channels[0].function_id != self.binding.function_id
        {
            return Err(IntentError("incomplete original recipe".into()));
        }
        // Source verification may predict, but it must not replace the captured estimate.
        Ok(PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
        })
    }
}
fn native() -> (Arc<NativeModel>, AttributeValue) {
    let model = Arc::new(NativeModel {
        identity: NativeColorIdentity {
            profile_id: Uuid::new_v4(),
            profile_revision: 1,
            profile_digest: "original-content".into(),
            mode_id: Uuid::new_v4(),
            head_id: Uuid::new_v4(),
            path_id: Uuid::new_v4(),
            model_revision: 1,
            native_layout_signature: "u32-original".into(),
        },
        binding: NativeColorBinding {
            channel_id: Uuid::new_v4(),
            function_id: Uuid::new_v4(),
        },
    });
    let family = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: model.identity.clone(),
            channels: vec![NativeColorValue {
                channel_id: model.binding.channel_id,
                function_id: model.binding.function_id,
                raw: u32::MAX - 1,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: Some(PortableUv {
                amount: 0.6,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec!["UV without visible estimate".into()],
        },
    }));
    (model, family)
}

#[test]
fn direct_batch_retains_original_u32_and_uv_and_rejects_missing_source_atomically() {
    let mut desk = Desk::new();
    let (model, family) = native();
    let direct = DynamicFixAtValue::programming(
        desk.targets[1],
        ProgrammingFamilyFixAt::from_family(
            ProgrammingOwner::Color,
            Some(ProgrammingComponent::NativeColor(model.binding)),
            family.clone(),
        )
        .unwrap(),
    );
    let position = DynamicFixAtValue::programming(
        desk.targets[0],
        ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Position, None, angles(10.0, 20.0))
            .unwrap(),
    );
    let before = desk.depth();
    assert!(desk.batch(vec![position.clone(), direct.clone()]).is_err());
    assert!(desk.masks().is_empty());
    assert_eq!(desk.depth(), before);
    desk.ports.native = Some(model);
    assert_eq!(desk.batch(vec![position, direct]).unwrap(), 2);
    assert_eq!(desk.masks()[1].family, family);
}

#[test]
fn capture_replay_does_not_resample_current_and_conflicting_replay_is_rejected() {
    let mut desk = Desk::new();
    desk.context.request_id = Some("one-capture".into());
    for target in desk.targets {
        desk.ports
            .environment
            .families
            .insert(target, angles(50.0, 60.0));
    }
    let command = desk.command(ProgrammingOwner::Position, Some(ProgrammingComponent::Tilt));
    assert_eq!(desk.capture(command.clone()).unwrap(), 2);
    desk.ports.environment.families.clear();
    assert_eq!(desk.capture(command).unwrap(), 2);
    assert_eq!(desk.ports.captures.load(Ordering::SeqCst), 1);
    assert!(
        desk.capture(desk.command(ProgrammingOwner::Position, None))
            .is_err()
    );
}

#[test]
fn preload_capture_go_preserves_typed_mask_and_complete_family() {
    let mut desk = Desk::new();
    for target in desk.targets {
        desk.ports
            .environment
            .families
            .insert(target, angles(50.0, 60.0));
    }
    desk.service.programmers.arm_preload(desk.session(), true);
    desk.capture(desk.command(ProgrammingOwner::Position, Some(ProgrammingComponent::Pan)))
        .unwrap();
    let state = desk.service.programmers.get(desk.session()).unwrap();
    assert!(state.dynamic_values.is_empty());
    assert_eq!(state.preload_dynamic_pending.len(), 2);
    for value in state.preload_dynamic_pending.iter() {
        let DynamicSemanticValue::ProgrammingFixAt { mask, .. } = &value.value else {
            panic!()
        };
        assert_eq!(mask.family, angles(50.0, 60.0));
        assert_eq!(mask.address.component, Some(ProgrammingComponent::Pan));
    }
    assert!(desk.service.programmers.activate_preload(desk.session()));
    let state = desk.service.programmers.get(desk.session()).unwrap();
    assert!(state.preload_dynamic_pending.is_empty());
    assert_eq!(state.preload_dynamic_active.len(), 2);
    for value in state.preload_dynamic_active.iter() {
        let DynamicSemanticValue::ProgrammingFixAt { mask, .. } = &value.value else {
            panic!()
        };
        assert_eq!(mask.family, angles(50.0, 60.0));
        assert_eq!(mask.address.component, Some(ProgrammingComponent::Pan));
    }
}

#[test]
fn batch_rejects_unmaterialized_values_and_independent_component_mixtures() {
    let desk = Desk::new();
    let before = desk.depth();
    let valid = DynamicFixAtValue::programming(
        desk.targets[0],
        ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Position, None, angles(10.0, 20.0))
            .unwrap(),
    );
    for (attribute, value) in [
        (
            ProgrammingOwner::Focus.key(),
            AttributeValue::Spread(vec![0.2, 0.8]),
        ),
        (
            ProgrammingOwner::Position.key(),
            AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
                owner: ProgrammingOwner::Position,
                template: angles(20.0, 30.0),
                members: Default::default(),
            })),
        ),
        (AttributeKey("pan".into()), AttributeValue::Normalized(0.5)),
    ] {
        let invalid = DynamicFixAtValue {
            fixture_id: desk.targets[0],
            attribute,
            value,
            programming_mask: None,
        };
        assert!(desk.batch(vec![valid.clone(), invalid]).is_err());
        assert!(desk.masks().is_empty());
        assert_eq!(desk.depth(), before);
    }
}

#[test]
fn capture_with_explicit_angle_activation_is_valid_and_bad_empty_mask_is_not() {
    let mut desk = Desk::new();
    for fixture in desk.targets {
        desk.ports.environment.families.insert(
            fixture,
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Origin,
                [0.0, 2.0, 3.0],
            ))),
        );
        desk.ports.environment.contexts.insert(
            fixture,
            OwnedFamilyEditContext {
                solved_angles: Some(JointAngles {
                    pan_degrees: 800.0,
                    tilt_degrees: -30.0,
                }),
                ..Default::default()
            },
        );
    }
    let mut command = desk.command(ProgrammingOwner::Position, Some(ProgrammingComponent::Pan));
    command.edits = vec![
        ComponentEdit::ActivateAngles,
        ComponentEdit::Scalar {
            component: ProgrammingComponent::Pan,
            operation: ScalarEdit::Set(ScalarIntent::Value(900.0)),
        },
    ];
    assert_eq!(desk.capture(command).unwrap(), 2);
    assert!(
        desk.masks()
            .iter()
            .all(|mask| mask.family == angles(900.0, -30.0))
    );
    let mut command = desk.command(
        ProgrammingOwner::Color,
        Some(ProgrammingComponent::ColorWheel(0)),
    );
    command.targets.clear();
    assert!(desk.capture(command).is_err());
}

fn scalar_head_desk(virtual_dimmer: bool) -> (Desk, [FixtureId; 3]) {
    let mut desk = Desk::new();
    let mut profile = light_fixture::FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "FAT mixed heads".into();
    let mode = &mut profile.modes[0];
    mode.heads[0].master_shared = false;
    let first = mode.heads[0].id;
    let channels = if virtual_dimmer {
        vec![(first, "color.red", false)]
    } else {
        vec![(first, "intensity", false)]
    };
    let mut channels = channels;
    for (attribute, discrete) in [("intensity", true), ("pan", false)] {
        let head = Uuid::new_v4();
        mode.heads.push(light_fixture::FixtureHead {
            id: head,
            name: attribute.into(),
            master_shared: false,
        });
        channels.push((head, attribute, discrete));
    }
    mode.channels = channels.into_iter().map(|(head, attribute, discrete)| {
        let behavior = if discrete {
            serde_json::json!({"type":"fixed", "semantic_id":"off", "label":"Off", "raw_value":0})
        } else { serde_json::json!({"type":"continuous", "physical_min":0.0, "physical_max":1.0, "unit":null}) };
        serde_json::from_value(serde_json::json!({
            "id":Uuid::new_v4(), "head_id":head, "split":1, "attribute":attribute,
            "fixture_attribute":attribute, "resolution":"u8", "default_raw":0, "highlight_raw":255, "functions":[{
                "id":Uuid::new_v4(), "name":attribute, "attribute":attribute, "dmx_from":0,
                "dmx_to":255, "priority":0, "behavior":behavior
            }]
        })).unwrap()
    }).collect();
    mode.splits[0].footprint = mode.channels.len() as u16;
    mode.default_virtual_dimmer_reactions();
    let targets = std::array::from_fn(|_| FixtureId::new());
    let mut fixture = desk.ports.snapshot.fixtures[0].clone();
    fixture.definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    fixture.logical_heads = targets
        .iter()
        .enumerate()
        .map(|(index, target)| light_fixture::PatchedHead {
            fixture_id: *target,
            head_index: index as u16,
            profile_head_id: Some(profile.modes[0].heads[index].id),
        })
        .collect();
    let parameter = fixture.definition.heads[0]
        .parameters
        .iter()
        .find(|p| p.attribute.is_intensity())
        .unwrap();
    assert_eq!(parameter.virtual_dimmer, virtual_dimmer);
    assert_eq!(parameter.components.is_empty(), virtual_dimmer);
    desk.ports.snapshot = Arc::new(EngineSnapshot {
        fixtures: Arc::new(vec![fixture]),
        ..Default::default()
    });
    (desk, targets)
}

#[test]
fn scalar_fix_at_logical_head_supports_virtual_and_physical_intensity() {
    for virtual_dimmer in [true, false] {
        let (desk, targets) = scalar_head_desk(virtual_dimmer);
        assert_eq!(
            desk.service
                .fix_at(
                    &desk.context,
                    DynamicFixAtCommand {
                        targets: vec![targets[0]],
                        attribute: AttributeKey::intensity(),
                        value: 0.5,
                        timing: DynamicValueTiming {
                            fade_millis: Some(2000),
                            delay_millis: Some(1000)
                        },
                    },
                    &desk.ports
                )
                .unwrap(),
            1
        );
        let state = desk.service.programmers.get(desk.session()).unwrap();
        assert!(state.values.is_empty());
        assert_eq!(state.dynamic_values.len(), 1);
        assert_eq!(state.dynamic_values[0].fixture_id, targets[0]);
        assert!(matches!(
            state.dynamic_values[0].value,
            DynamicSemanticValue::FixAt {
                value: 0.5,
                timing: DynamicValueTiming {
                    fade_millis: Some(2000),
                    delay_millis: Some(1000)
                }
            }
        ));
    }
}

#[test]
fn scalar_fix_at_logical_head_rejects_discrete_unsupported_and_missing_atomically() {
    let (desk, targets) = scalar_head_desk(false);
    let root = desk.ports.snapshot.fixtures[0].fixture_id;
    for (selection, expected) in [
        (vec![targets[1]], "discrete"),
        (vec![targets[0], targets[1]], "discrete"),
        (vec![targets[2]], "unsupported"),
        (vec![targets[0], targets[2]], "unsupported"),
        (vec![targets[0], FixtureId::new()], "unsupported"),
        (vec![root], "discrete"),
    ] {
        let before =
            serde_json::to_value(desk.service.programmers.get(desk.session()).unwrap()).unwrap();
        let error = desk
            .service
            .fix_at(
                &desk.context,
                DynamicFixAtCommand {
                    targets: selection,
                    attribute: AttributeKey::intensity(),
                    value: 0.5,
                    timing: Default::default(),
                },
                &desk.ports,
            )
            .unwrap_err();
        assert!(error.message.contains(expected), "{}", error.message);
        assert_eq!(
            serde_json::to_value(desk.service.programmers.get(desk.session()).unwrap()).unwrap(),
            before
        );
    }
}
