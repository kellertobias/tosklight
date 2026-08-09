impl<P: ExtensionApplicationPorts> ExtensionHost<P> {
fn prepare_session(
    &self,
    attempt: u64,
    shared: &Arc<SharedHealth>,
    repair: &Arc<AtomicBool>,
) -> Result<ActiveSession, SessionResult> {
    let mut child = self
        .spawn(attempt)
        .map_err(|detail| SessionResult::failed(detail, false, false))?;
    {
        let mut health = shared.value.lock().expect("extension health mutex");
        health.launches += 1;
        health.state = ExtensionState::Handshaking;
    }
    let stdin = child.stdin.take().expect("piped child stdin");
    let stdout = child.stdout.take().expect("piped child stdout");
    let stderr = child.stderr.take().expect("piped child stderr");
    let (writer_tx, writer_rx) = std::sync::mpsc::sync_channel(self.limits.wire_queue.max(1));
    let (reader_tx, reader_rx) =
        std::sync::mpsc::sync_channel(self.limits.inbound_queue.max(1));
    let (log_tx, log_rx) = std::sync::mpsc::sync_channel(self.limits.stderr_queue.max(1));
    let writer = std::thread::spawn(move || writer_loop(stdin, writer_rx));
    let reader_drops = Arc::clone(&shared.inbound_drops);
    let inbound_drops_before = reader_drops.load(Ordering::Relaxed);
    let reader = std::thread::spawn(move || reader_loop(stdout, reader_tx, reader_drops));
    let stderr_drops = Arc::clone(&shared.stderr_transport_drops);
    let maximum_log_line = self.limits.log_bytes.max(1);
    let logger =
        std::thread::spawn(move || stderr_loop(stderr, log_tx, stderr_drops, maximum_log_line));
    let mut logs = BoundedLog::new(self.limits.log_bytes, self.limits.log_lines);

    let channel_challenge = fresh_channel_challenge();
    let hello = HostHello {
        host_name: "ToskLight".into(),
        host_instance_id: self.spec.desk_id.clone(),
        supported_versions: vec![DRAFT_PROTOCOL_V1],
        requested_capabilities: self.spec.requested_capabilities.clone(),
        channel_challenge: channel_challenge.clone(),
    };
    if writer_tx
        .send(WriterCommand::Message(Box::new(Message::HostHello(
            hello.clone(),
        ))))
        .is_err()
    {
        return Err(finish_failed(
            child,
            writer_tx,
            writer,
            reader,
            logger,
            "child channel closed before handshake".into(),
            (true, false),
        ));
    }
    let extension = match wait_for_hello(
        &mut child,
        &reader_rx,
        &log_rx,
        &mut logs,
        shared,
        self.limits.handshake_timeout,
    ) {
        Ok(extension) => extension,
        Err((detail, crashed, protocol)) => {
            return Err(finish_failed(
                child,
                writer_tx,
                writer,
                reader,
                logger,
                detail,
                (crashed, protocol),
            ));
        }
    };
    let context = self.context();
    let (capabilities, configure) = match self.negotiate_configuration(
        &hello,
        &extension,
        &channel_challenge,
        &context,
    ) {
        Ok(configure) => configure,
        Err(detail) => {
            return Err(finish_failed(
                child,
                writer_tx,
                writer,
                reader,
                logger,
                detail,
                (false, true),
            ));
        }
    };
    if writer_tx
        .send(WriterCommand::Message(Box::new(Message::Configure(
            configure,
        ))))
        .is_err()
    {
        return Err(finish_failed(
            child,
            writer_tx,
            writer,
            reader,
            logger,
            "child channel closed before configuration".into(),
            (true, false),
        ));
    }
    repair.store(false, Ordering::Release);
    set_state(shared, ExtensionState::Running);
    let mut health = shared.value.lock().expect("extension health mutex");
    for channel in health.telemetry.values_mut() {
        channel.last_sequence = None;
        channel.stale = false;
    }
    drop(health);
    Ok(ActiveSession {
        child,
        writer_tx,
        writer,
        reader,
        logger,
        reader_rx,
        log_rx,
        logs,
        inbound_drops_before,
        capabilities,
        context,
        last_input_id: None,
        held_controls: BTreeSet::new(),
        last_timecode_id: None,
        pending_device_actions: BTreeMap::new(),
    })
}

fn negotiate_configuration(
    &self,
    hello: &HostHello,
    extension: &ExtensionHello,
    channel_challenge: &str,
    context: &HostControlContext,
) -> Result<(BTreeSet<ExtensionCapability>, Configure), String> {
    let expected = HandshakeExpectations {
        extension_id: self.spec.extension_id.clone(),
        extension_instance_id: self.spec.extension_instance_id.clone(),
        approved_package_digest: self.spec.approved_package_digest.clone(),
        channel_response: crate::channel_response(
            &self.spec.channel_credential,
            channel_challenge,
        ),
    };
    let negotiated = negotiate(hello, extension, &expected).map_err(|error| error.to_string())?;
    let configure = Configure {
        enabled_capabilities: negotiated.capabilities.clone(),
        feedback: negotiated
            .capabilities
            .contains(&ExtensionCapability::ControlSurface)
            .then(|| {
                let mut snapshot = self
                    .ports
                    .feedback_snapshot(context, &self.spec.control_bindings);
                degrade_feedback_snapshot(&mut snapshot, &self.spec.feedback_features);
                snapshot
            }),
        telemetry_channels: self.spec.telemetry_channels.clone(),
        device_actions: self.spec.device_actions.clone(),
        control_bindings: self.spec.control_bindings.clone(),
        settings: self.spec.settings.clone(),
    };
    negotiated
        .validate_configure(&configure)
        .map_err(|error| error.to_string())?;
    Ok((negotiated.capabilities, configure))
}

fn repair_feedback(
    &self,
    session: &mut ActiveSession,
    commands: &Receiver<SupervisorCommand>,
    shared: &Arc<SharedHealth>,
    repair: &Arc<AtomicBool>,
) -> Result<(), RequestedSessionExit> {
    loop {
        match commands.try_recv() {
            Ok(SupervisorCommand::Feedback(_)) | Ok(SupervisorCommand::DeviceAction(_)) => {
                shared
                    .value
                    .lock()
                    .expect("extension health mutex")
                    .outbound_drops += 1;
            }
            Ok(SupervisorCommand::RefreshSnapshot) => repair.store(true, Ordering::Release),
            Ok(SupervisorCommand::Restart) => return Err(RequestedSessionExit::Restart),
            Ok(SupervisorCommand::Stop) | Err(TryRecvError::Disconnected) => {
                return Err(RequestedSessionExit::Stop);
            }
            Err(TryRecvError::Empty) => break,
        }
    }
    let mut snapshot = self
        .ports
        .feedback_snapshot(&session.context, &self.spec.control_bindings);
    degrade_feedback_snapshot(&mut snapshot, &self.spec.feedback_features);
    if let Err(error) = session.writer_tx.try_send(WriterCommand::Message(Box::new(
        Message::FeedbackSnapshot(snapshot),
    ))) {
        if matches!(error, TrySendError::Full(_)) {
            std::thread::yield_now();
        } else {
            return Err(RequestedSessionExit::WriterStopped);
        }
    } else {
        repair.store(false, Ordering::Release);
    }
    Ok(())
}

fn forward_command(
    &self,
    session: &mut ActiveSession,
    commands: &Receiver<SupervisorCommand>,
    shared: &Arc<SharedHealth>,
    repair: &Arc<AtomicBool>,
) -> Result<(), RequestedSessionExit> {
    match commands.try_recv() {
        Ok(SupervisorCommand::Feedback(mut delta)) => {
            degrade_feedback_delta(&mut delta, &self.spec.feedback_features);
            if repair.load(Ordering::Acquire)
                || session
                    .writer_tx
                    .try_send(WriterCommand::Message(Box::new(Message::FeedbackDelta(
                        delta,
                    ))))
                    .is_err()
            {
                repair.store(true, Ordering::Release);
                shared
                    .value
                    .lock()
                    .expect("extension health mutex")
                    .outbound_drops += 1;
            }
        }
        Ok(SupervisorCommand::DeviceAction(request)) => {
            if session
                .writer_tx
                .try_send(WriterCommand::Message(Box::new(
                    Message::DeviceActionRequest(request.clone()),
                )))
                .is_err()
            {
                shared
                    .value
                    .lock()
                    .expect("extension health mutex")
                    .outbound_drops += 1;
            } else {
                session
                    .pending_device_actions
                    .insert(request.request_id, request.action_id);
            }
        }
        Ok(SupervisorCommand::RefreshSnapshot) => repair.store(true, Ordering::Release),
        Ok(SupervisorCommand::Restart) => return Err(RequestedSessionExit::Restart),
        Ok(SupervisorCommand::Stop) | Err(TryRecvError::Disconnected) => {
            return Err(RequestedSessionExit::Stop);
        }
        Err(TryRecvError::Empty) => {}
    }
    Ok(())
}
}
