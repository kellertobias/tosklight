//! Preserve the selected logical instance while replaying an authored start on an isolated
//! branch. Controls and history are evaluated locally; no Live sample maps are installed.
use super::*;

impl DynamicRuntime {
    pub fn start(&mut self, request: DynamicStartRequest) -> Result<Uuid, DynamicRuntimeError> {
        self.start_with_resolved_identity(request, None)
    }

    /// Apply a start using the identity selected by its authoritative source operation.
    /// The caller supplies the original request/time and orders this against the branch's own
    /// selected samples and cold changes. This does not acknowledge a journal cursor or make
    /// retries idempotent: the caller must roll back a failed enclosing replay transaction.
    ///
    /// Existing clocks are reused only when their bound/targetless scope allows it. Conflicting
    /// identities fail before mutation instead of selecting another clock or replacing one.
    /// A completed branch instance follows the ordinary start/restart policy, independently of
    /// whether Live was complete. All held Current values and Random progress remain branch-local.
    pub fn start_with_instance_identity(
        &mut self,
        request: DynamicStartRequest,
        instance_id: Uuid,
    ) -> Result<Uuid, DynamicRuntimeError> {
        self.start_with_resolved_identity(request, Some(instance_id))
    }

    fn start_with_resolved_identity(
        &mut self,
        request: DynamicStartRequest,
        resolved_id: Option<Uuid>,
    ) -> Result<Uuid, DynamicRuntimeError> {
        if resolved_id.is_some_and(|id| id.is_nil()) {
            return Err(DynamicRuntimeError::InvalidReplay(
                "Dynamic instance identity cannot be nil".into(),
            ));
        }
        validate_controller(&request.controller)?;
        if request.target_scope.ordered_targets.is_empty() {
            return Err(DynamicRuntimeError::EmptyTargets);
        }
        let definition = Arc::clone(
            self.definitions
                .get(&request.definition_id)
                .ok_or(DynamicRuntimeError::MissingDefinition)?,
        );
        let bound = !matches!(definition.target_binding, DynamicTargetBinding::Targetless);
        let reusable = |instance: &DynamicInstance| {
            instance.definition.id == definition.id
                && instance.targets == request.target_scope.ordered_targets
                && instance.controllers.values().any(|controller| {
                    controller
                        .source
                        .same_runtime_owner(&request.controller.source)
                })
        };
        let existing = if let Some(expected) = resolved_id {
            if bound {
                match self.bound_instances.get(&definition.id).copied() {
                    Some(actual) if actual != expected => {
                        return Err(DynamicRuntimeError::InvalidReplay(
                            "Bound Dynamic already has a different instance identity".into(),
                        ));
                    }
                    actual => actual,
                }
            } else if request.reuse_matching_targetless {
                if self.instances.get(&expected).is_some_and(reusable) {
                    Some(expected)
                } else if self.instances.values().any(reusable) {
                    return Err(DynamicRuntimeError::InvalidReplay(
                        "Targetless Dynamic reuse identity does not match its retained scope"
                            .into(),
                    ));
                } else {
                    None
                }
            } else {
                None
            }
        } else if bound {
            self.bound_instances.get(&definition.id).copied()
        } else if request.reuse_matching_targetless {
            self.instances
                .values()
                .find(|instance| reusable(instance))
                .map(|instance| instance.id)
        } else {
            None
        };
        if let Some(expected) = resolved_id
            && existing.is_none()
            && self.instances.contains_key(&expected)
        {
            return Err(DynamicRuntimeError::InvalidReplay(
                "Dynamic replay cannot replace an existing instance".into(),
            ));
        }
        if let Some(instance_id) = existing {
            self.journal_instance(instance_id);
            let instance = self
                .instances
                .get_mut(&instance_id)
                .expect("instance indices stay synchronized");
            if instance.completed {
                instance.completed = false;
                instance.started_at_millis = request.now_millis;
                instance.paused_at_millis = self.global_paused.then_some(request.now_millis);
                instance.paused_elapsed_millis = 0;
                instance.pending_until_millis = None;
                instance.speed_paused_at_millis = None;
                instance.speed_paused_elapsed_millis = 0;
                instance.random_streams.clear();
                instance.synchronized_hold_elapsed_millis = None;
                instance.synchronized_hold_captured = false;
                instance.last_synchronized_elapsed_millis = None;
                instance.synchronized_resume_transition = None;
                instance.last_sample_values.clear();
                instance.synchronized_hold_values.clear();
                instance.synchronized_hold_angle_sources.clear();
                instance.unavailable_samples.clear();
                instance.controllers.retain(|_, controller| {
                    !controller
                        .source
                        .same_runtime_owner(&request.controller.source)
                });
                instance
                    .controller_transitions
                    .retain(|controller_id, _| instance.controllers.contains_key(controller_id));
            }
            instance
                .controllers
                .insert(request.controller.id, request.controller.clone());
            instance.controller_transitions.insert(
                request.controller.id,
                DynamicControllerTransitionSnapshot {
                    controller_id: request.controller.id,
                    activation_started_at_millis: request.now_millis,
                    activation_delay_millis: request.activation_delay_millis,
                    activation_duration_millis: request.activation_duration_millis,
                    ..Default::default()
                },
            );
            reconcile_pause(instance, self.global_paused, request.now_millis);
            return Ok(instance_id);
        }

        let instance_id = resolved_id.unwrap_or_else(Uuid::new_v4);
        let phase_by_lane_target = project_instance_phases(
            &definition,
            &request.target_scope.ordered_targets,
            &request.stage_positions,
            request.inherited_spatial_mapping.as_ref(),
        )?;
        let mut controllers = HashMap::new();
        controllers.insert(request.controller.id, request.controller.clone());
        let controller_transitions = HashMap::from([(
            request.controller.id,
            DynamicControllerTransitionSnapshot {
                controller_id: request.controller.id,
                activation_started_at_millis: request.now_millis,
                activation_delay_millis: request.activation_delay_millis,
                activation_duration_millis: request.activation_duration_millis,
                ..Default::default()
            },
        )]);
        let activation_policy = request
            .activation_policy_override
            .unwrap_or(definition.default_activation);
        let instance = DynamicInstance {
            id: instance_id,
            programming_lanes: self.compiled_lanes[&definition.id].clone(),
            preset_values: Default::default(),
            preset_dependency_generation: Uuid::new_v4(),
            definition,
            targets: request.target_scope.ordered_targets,
            phase_by_lane_target,
            controllers,
            lane_selections: HashMap::new(),
            controller_transitions,
            started_at_millis: request.now_millis,
            paused_at_millis: self.global_paused.then_some(request.now_millis),
            paused_elapsed_millis: 0,
            activation_policy,
            pending_until_millis: None,
            speed_paused_at_millis: None,
            speed_paused_elapsed_millis: 0,
            random_streams: HashMap::new(),
            completed: false,
            synchronized_hold_elapsed_millis: None,
            synchronized_hold_captured: false,
            last_synchronized_elapsed_millis: None,
            synchronized_resume_transition: None,
            last_sample_values: HashMap::new(),
            synchronized_hold_values: HashMap::new(),
            unavailable_samples: HashMap::new(),
            synchronized_hold_angle_sources: Default::default(),
            frame_addresses: None,
        };
        if bound {
            if let Some(undo) = &mut self.output_frame_undo {
                undo.record_bound(
                    instance.definition.id,
                    self.bound_instances.get(&instance.definition.id).copied(),
                );
            }
            self.bound_instances
                .insert(instance.definition.id, instance_id);
        }
        self.journal_instance(instance_id);
        self.instances.insert(instance_id, instance);
        Ok(instance_id)
    }
}
