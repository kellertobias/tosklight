//! Source-driven mutations keep the existing reconciliation result types while joining the
//! runtime's optional control journal. Automatic sampling cleanup does not use these adapters.
use light_dynamics::{
    ActivationPolicy, DynamicControl, DynamicControlOutcome, DynamicControllerSource,
    DynamicDefinition, DynamicLaneSelection, DynamicRuntime, DynamicRuntimeError,
    DynamicStartRequest, TimedDynamicControl,
};
use uuid::Uuid;

fn apply(
    runtime: &mut DynamicRuntime,
    at_millis: u64,
    control: DynamicControl,
) -> Result<DynamicControlOutcome, DynamicRuntimeError> {
    runtime.apply_recorded_control(TimedDynamicControl { at_millis, control })
}

fn check_instance(
    runtime: &DynamicRuntime,
    instance: Uuid,
    controller: Uuid,
) -> Result<(), DynamicRuntimeError> {
    if runtime.instance_definition(instance).is_none() {
        return Err(DynamicRuntimeError::MissingInstance);
    }
    if runtime
        .controller(controller)
        .is_none_or(|(id, _)| id != instance)
    {
        return Err(DynamicRuntimeError::MissingController);
    }
    Ok(())
}

pub(super) fn fallback_definition(
    runtime: &mut DynamicRuntime,
    definition: DynamicDefinition,
    at: u64,
) -> Result<(), DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.install_fallback_definition(definition);
    }
    apply(
        runtime,
        at,
        DynamicControl::InstallFallbackDefinition(Box::new(definition)),
    )
    .map(|_| ())
}

pub(super) fn start(
    runtime: &mut DynamicRuntime,
    observed_at: u64,
    request: DynamicStartRequest,
) -> Result<Uuid, DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.start(request);
    }
    // The observed time orders reconciliation. request.now_millis separately retains the
    // source's historical activation time, which may precede this captured frame.
    apply(
        runtime,
        observed_at,
        DynamicControl::Start(Box::new(request)),
    )
    .map(|outcome| outcome.instance_id.expect("Start selects its instance"))
}

pub(super) fn off(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    controller: Uuid,
    at: u64,
    delay: u64,
    duration: u64,
) -> Result<bool, DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.off_controller(instance, controller, at, delay, duration);
    }
    check_instance(runtime, instance, controller)?;
    apply(
        runtime,
        at,
        DynamicControl::Off {
            controller,
            delay,
            duration,
        },
    )
    .map(|outcome| outcome.instance_removed)
}

pub(super) fn cancel_release(
    runtime: &mut DynamicRuntime,
    controller: Uuid,
    at: u64,
) -> Result<(), DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.cancel_controller_release(controller);
    }
    apply(runtime, at, DynamicControl::CancelRelease { controller }).map(|_| ())
}

pub(super) fn update(
    runtime: &mut DynamicRuntime,
    controller: Uuid,
    size: Option<f32>,
    speed: Option<f32>,
    phase: Option<f32>,
    at: u64,
) -> Result<(), DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.update_controller(controller, size, speed, phase);
    }
    apply(
        runtime,
        at,
        DynamicControl::Update {
            controller,
            size,
            speed,
            phase,
        },
    )
    .map(|_| ())
}

pub(super) fn rank(
    runtime: &mut DynamicRuntime,
    controller: Uuid,
    priority: i16,
    authored_at: u64,
    at: u64,
) -> Result<(), DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.update_controller_rank(controller, priority, authored_at, at);
    }
    apply(
        runtime,
        at,
        DynamicControl::Rank {
            controller,
            priority,
            authored_at,
        },
    )
    .map(|_| ())
}

/// Refresh a surviving controller's current owner and priority. Recorded so retained
/// histories replay the same operational metadata change; the instance is never restarted.
pub(super) fn owner(
    runtime: &mut DynamicRuntime,
    controller: Uuid,
    source: DynamicControllerSource,
    priority: i16,
    at: u64,
) -> Result<bool, DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.update_controller_owner(controller, source, priority);
    }
    apply(
        runtime,
        at,
        DynamicControl::Owner {
            controller,
            source,
            priority,
        },
    )
    .map(|outcome| outcome.changed)
}

pub(super) fn pause(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    controller: Uuid,
    paused: bool,
    at: u64,
    resume: Option<ActivationPolicy>,
) -> Result<(), DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.set_controller_paused_with_resume(instance, controller, paused, at, resume);
    }
    check_instance(runtime, instance, controller)?;
    apply(
        runtime,
        at,
        DynamicControl::Pause {
            controller,
            paused,
            resume,
        },
    )
    .map(|_| ())
}

pub(super) fn lanes(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    controller: Uuid,
    selection: DynamicLaneSelection,
    at: u64,
) -> Result<bool, DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.set_controller_lane_selection(instance, controller, selection);
    }
    check_instance(runtime, instance, controller)?;
    apply(
        runtime,
        at,
        DynamicControl::Lanes {
            controller,
            selection,
        },
    )
    .map(|outcome| outcome.changed)
}

pub(super) fn output_gate(
    runtime: &mut DynamicRuntime,
    controller: Uuid,
    enabled: bool,
    at: u64,
    delay: u64,
    duration: u64,
) -> Result<bool, DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.set_controller_output_enabled(controller, enabled, at, delay, duration);
    }
    apply(
        runtime,
        at,
        DynamicControl::OutputGate {
            controller,
            enabled,
            delay,
            duration,
        },
    )
    .map(|outcome| outcome.changed)
}

pub(super) fn clear_output_gate(
    runtime: &mut DynamicRuntime,
    controller: Uuid,
    at: u64,
) -> Result<bool, DynamicRuntimeError> {
    if runtime.control_cursor().is_none() {
        return runtime.clear_controller_output_gate(controller);
    }
    apply(runtime, at, DynamicControl::ClearOutputGate { controller })
        .map(|outcome| outcome.changed)
}
