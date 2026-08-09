impl<P: ExtensionApplicationPorts> ExtensionHost<P> {
fn handle_control_input(
    &self,
    input: light_extensions_contract::ControlInputEvent,
    context: &HostControlContext,
    writer: &SyncSender<WriterCommand>,
    last_input_id: &mut Option<u64>,
    held_controls: &mut BTreeSet<String>,
) -> Result<(), String> {
    if last_input_id.is_some_and(|previous| input.input_id <= previous) {
        return Err(format!(
            "control input_id {} is stale or duplicated",
            input.input_id
        ));
    }
    validate_control_input(&input, &self.spec.control_bindings)
        .map_err(|error| error.to_string())?;
    if let light_extensions_contract::ControlInput::Button {
        control_id,
        pressed,
    } = &input.control
    {
        if *pressed && !held_controls.insert(control_id.clone()) {
            return Err(format!(
                "button `{control_id}` was pressed while already held"
            ));
        }
        if !*pressed && !held_controls.remove(control_id) {
            return Err(format!(
                "button `{control_id}` was released without a preceding press"
            ));
        }
    }
    *last_input_id = Some(input.input_id);
    let control_id = match &input.control {
        light_extensions_contract::ControlInput::Button { control_id, .. }
        | light_extensions_contract::ControlInput::Absolute { control_id, .. }
        | light_extensions_contract::ControlInput::Relative { control_id, .. } => control_id,
    };
    let intent = self.spec.control_bindings[control_id].clone();
    if let Some(mut delta) = self
        .ports
        .apply_control(context, BoundControlInput { input, intent })
        .map_err(|error| error.to_string())?
    {
        degrade_feedback_delta(&mut delta, &self.spec.feedback_features);
        writer
            .try_send(WriterCommand::Message(Box::new(Message::FeedbackDelta(
                delta,
            ))))
            .map_err(|_| "outbound feedback queue is full".to_owned())?;
    }
    Ok(())
}

fn handle_telemetry_sample(
    &self,
    sample: light_extensions_contract::TelemetrySample,
    shared: &Arc<SharedHealth>,
) -> Result<(), String> {
    if let Err(error) = validate_telemetry_sample(&sample, &self.spec.telemetry_channels) {
        if matches!(
            error,
            HandshakeError::TelemetryValueOutOfRange(_)
                | HandshakeError::UnsupportedTelemetryQuality(_)
                | HandshakeError::MissingTelemetryTimestamp(_)
        ) {
            if let Some(channel) = shared
                .value
                .lock()
                .expect("extension health mutex")
                .telemetry
                .get_mut(&sample.channel_id)
            {
                channel.invalid_samples += 1;
                if channel
                    .last_sequence
                    .is_none_or(|last| sample.sample_id > last)
                {
                    if let Some(last) = channel.last_sequence {
                        channel.lost_samples += sample.sample_id.saturating_sub(last + 1);
                    }
                    channel.last_sequence = Some(sample.sample_id);
                }
            }
            return Ok(());
        }
        return Err(error.to_string());
    }
    let received_at_micros = unix_time_micros();
    {
        let mut health = shared.value.lock().expect("extension health mutex");
        let channel = health
            .telemetry
            .get_mut(&sample.channel_id)
            .expect("validated telemetry channel exists");
        if channel
            .last_sequence
            .is_some_and(|last| sample.sample_id <= last)
            || channel
                .latest
                .as_ref()
                .is_some_and(|latest| sample.observed_at_micros <= latest.sample.observed_at_micros)
        {
            channel.invalid_samples += 1;
            return Ok(());
        }
        if let Some(last) = channel.last_sequence {
            channel.lost_samples += sample.sample_id.saturating_sub(last + 1);
        }
        if sample.quality == TelemetryQuality::Stale {
            channel.stale_samples += 1;
        }
        let minimum_interval = (self.spec.maximum_telemetry_rate_hz > 0)
            .then(|| 1_000_000_u64 / u64::from(self.spec.maximum_telemetry_rate_hz));
        if channel
            .latest
            .as_ref()
            .zip(minimum_interval)
            .is_some_and(|(latest, minimum)| {
                received_at_micros.saturating_sub(latest.received_at_micros) < minimum
            })
        {
            channel.excess_rate_samples += 1;
        }
    }
    let envelope = TelemetryEnvelope {
        extension_id: self.spec.extension_id.clone(),
        extension_instance_id: self.spec.extension_instance_id.clone(),
        received_at_micros,
        sample,
    };
    self.ports
        .publish_telemetry(envelope.clone())
        .map_err(|error| error.to_string())?;
    let mut health = shared.value.lock().expect("extension health mutex");
    let channel = health
        .telemetry
        .get_mut(&envelope.sample.channel_id)
        .expect("validated telemetry channel exists");
    channel.last_sequence = Some(envelope.sample.sample_id);
    channel.accepted_samples += 1;
    channel.latest = Some(envelope.clone());
    channel.history.push(envelope);
    let overflow = channel
        .history
        .len()
        .saturating_sub(self.limits.telemetry_history_samples.max(1));
    if overflow > 0 {
        channel.history.drain(..overflow);
    }
    Ok(())
}

fn handle_device_action_result(
    &self,
    result: DeviceActionResult,
    pending_device_actions: &mut BTreeMap<u64, String>,
    shared: &Arc<SharedHealth>,
) -> Result<(), String> {
    let Some(action_id) = pending_device_actions.remove(&result.request_id) else {
        return Err(format!(
            "device action result references unknown request {}",
            result.request_id
        ));
    };
    if action_id != result.action_id {
        return Err("device action result action_id does not match request".into());
    }
    let declaration = self
        .spec
        .device_actions
        .iter()
        .find(|declaration| declaration.action_id == result.action_id)
        .ok_or_else(|| "device action result is no longer declared".to_owned())?;
    validate_device_action_result(&result, declaration).map_err(|error| error.to_string())?;
    let mut health = shared.value.lock().expect("extension health mutex");
    health.device_action_results.push(result);
    let overflow = health
        .device_action_results
        .len()
        .saturating_sub(self.limits.telemetry_history_samples.max(1));
    if overflow > 0 {
        health.device_action_results.drain(..overflow);
    }
    Ok(())
}
}
