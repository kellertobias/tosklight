use super::*;

#[derive(Clone)]
pub(in crate::runtime) struct IntegrationResource {
    matter_bridge: Arc<matter::MatterBridgeAdapter>,
    matter_transport: Option<Arc<matter::MatterTransport>>,
    osc_subscribers: Arc<Mutex<HashMap<String, OscSubscriber>>>,
    osc_cue_record_suppression: Arc<Mutex<osc_cue_record_suppression::OscCueRecordSuppression>>,
    osc_feedback: Option<Arc<std::net::UdpSocket>>,
    #[cfg(test)]
    osc_feedback_capture: Arc<Mutex<Vec<CapturedOscMessage>>>,
}

impl IntegrationResource {
    pub(in crate::runtime) fn new(
        matter_bridge: Arc<matter::MatterBridgeAdapter>,
        matter_transport: Option<Arc<matter::MatterTransport>>,
        osc_feedback: Option<Arc<std::net::UdpSocket>>,
    ) -> Self {
        Self {
            matter_bridge,
            matter_transport,
            osc_subscribers: Arc::default(),
            osc_cue_record_suppression: Arc::default(),
            osc_feedback,
            #[cfg(test)]
            osc_feedback_capture: Arc::default(),
        }
    }

    pub(in crate::runtime) fn matter_bridge(&self) -> &matter::MatterBridgeAdapter {
        &self.matter_bridge
    }

    pub(in crate::runtime) fn matter_transport(&self) -> Option<&matter::MatterTransport> {
        self.matter_transport.as_deref()
    }

    pub(in crate::runtime) fn hardware_connected(&self) -> bool {
        !self.osc_subscribers.lock().is_empty()
    }

    pub(in crate::runtime) fn osc_subscriber(&self, client_id: &str) -> Option<OscSubscriber> {
        self.osc_subscribers.lock().get(client_id).cloned()
    }

    #[cfg(test)]
    pub(in crate::runtime) fn set_osc_last_seen(&self, client_id: &str, last_seen: Instant) {
        if let Some(subscriber) = self.osc_subscribers.lock().get_mut(client_id) {
            subscriber.last_seen = last_seen;
        }
    }

    #[cfg(test)]
    pub(in crate::runtime) fn set_osc_record_started(&self, client_id: &str, started: Instant) {
        if let Some(subscriber) = self.osc_subscribers.lock().get_mut(client_id) {
            subscriber.update_record_started = Some(started);
        }
    }

    pub(in crate::runtime) fn osc_subscriber_for_source(
        &self,
        source: SocketAddr,
    ) -> Option<OscSubscriber> {
        self.osc_subscribers
            .lock()
            .values()
            .find(|subscriber| subscriber.command_source == source)
            .cloned()
    }

    pub(in crate::runtime) fn accept_highlight_action(
        &self,
        source: SocketAddr,
        desk_alias: &str,
        action: HighlightAction,
        now: Instant,
    ) -> Option<SessionId> {
        let mut subscribers = self.osc_subscribers.lock();
        let subscriber = subscribers.values_mut().find(|subscriber| {
            subscriber.command_source == source && subscriber.desk_alias == desk_alias
        })?;
        if is_duplicate_osc_action(
            subscriber
                .last_highlight_action
                .as_ref()
                .map(|(previous, received_at)| (previous.as_str(), *received_at)),
            action,
            now,
        ) {
            return None;
        }
        subscriber.last_highlight_action = Some((action.osc_dedupe_key().to_owned(), now));
        Some(subscriber.session_id)
    }

    pub(in crate::runtime) fn register_osc_subscriber(
        &self,
        client_id: String,
        subscriber: OscSubscriber,
    ) -> Option<OscSubscriber> {
        self.osc_subscribers.lock().insert(client_id, subscriber)
    }

    pub(in crate::runtime) fn unregister_osc_subscriber(
        &self,
        client_id: &str,
    ) -> Option<OscSubscriber> {
        self.osc_subscribers.lock().remove(client_id)
    }

    pub(in crate::runtime) fn clear_osc_subscribers(&self) {
        self.osc_subscribers.lock().clear();
    }

    pub(in crate::runtime) fn has_osc_session(&self, session_id: SessionId) -> bool {
        self.osc_subscribers
            .lock()
            .values()
            .any(|subscriber| subscriber.session_id == session_id)
    }

    pub(in crate::runtime) fn osc_session_is_exclusive_to_client(
        &self,
        client_id: &str,
        session_id: SessionId,
    ) -> bool {
        self.osc_subscribers
            .lock()
            .iter()
            .all(|(id, subscriber)| id == client_id || subscriber.session_id != session_id)
    }

    pub(in crate::runtime) fn set_shift(&self, source: SocketAddr, pressed: bool) {
        let mut subscribers = self.osc_subscribers.lock();
        let Some(target) = subscribers
            .values_mut()
            .find(|candidate| candidate.command_source == source)
        else {
            return;
        };
        if pressed {
            target.shifted = !target.shifted;
            target.shift_held = true;
        } else {
            target.shift_held = false;
            if target.update_first_release.is_some() {
                target.shifted = false;
            }
        }
    }

    pub(in crate::runtime) fn clear_shift(&self, source: SocketAddr) {
        if let Some(target) = self
            .osc_subscribers
            .lock()
            .values_mut()
            .find(|candidate| candidate.command_source == source)
        {
            target.shifted = false;
        }
    }

    pub(in crate::runtime) fn record_gesture(
        &self,
        source: SocketAddr,
        pressed: bool,
    ) -> osc_highlight::OscRecordGesture {
        self.osc_subscribers
            .lock()
            .values_mut()
            .find(|candidate| candidate.command_source == source)
            .map(|target| osc_highlight::record_gesture(target, pressed))
            .unwrap_or(osc_highlight::OscRecordGesture::None)
    }

    pub(in crate::runtime) fn active_osc_subscribers(
        &self,
        now: Instant,
        timeout: Duration,
    ) -> (Vec<OscSubscriber>, Vec<(SessionId, SocketAddr)>, bool) {
        let mut subscribers = self.osc_subscribers.lock();
        let before = subscribers.len();
        let expired = subscribers
            .values()
            .filter(|subscriber| now.duration_since(subscriber.last_seen) >= timeout)
            .map(|subscriber| (subscriber.session_id, subscriber.command_source))
            .collect::<Vec<_>>();
        subscribers.retain(|_, subscriber| now.duration_since(subscriber.last_seen) < timeout);
        let changed = before != subscribers.len();
        let active = subscribers.values().cloned().collect();
        (active, expired, changed)
    }

    pub(in crate::runtime) fn remove_suppression_source(
        &self,
        session_id: SessionId,
        source: SocketAddr,
    ) {
        self.osc_cue_record_suppression
            .lock()
            .remove_source(session_id, source);
    }

    pub(in crate::runtime) fn remove_session_suppression(&self, session_id: SessionId) {
        self.osc_cue_record_suppression
            .lock()
            .remove_session(session_id);
    }

    pub(in crate::runtime) fn suppresses_osc_input(
        &self,
        input: osc_cue_record_suppression::OscSuppressionInput<'_>,
        now: Instant,
    ) -> bool {
        self.osc_cue_record_suppression
            .lock()
            .suppresses_input(input, now)
    }

    pub(in crate::runtime) fn remember_osc_intercept(
        &self,
        input: osc_cue_record_suppression::OscSuppressionInput<'_>,
        now: Instant,
    ) {
        self.osc_cue_record_suppression
            .lock()
            .remember_intercept(input, now);
    }

    pub(in crate::runtime) fn send_osc(
        &self,
        target: SocketAddr,
        address: String,
        arguments: Vec<OscArgument>,
    ) {
        #[cfg(test)]
        self.osc_feedback_capture
            .lock()
            .push((target, address.clone(), arguments.clone()));
        if let (Some(socket), Ok(packet)) =
            (&self.osc_feedback, encode_osc_message(&address, &arguments))
        {
            let _ = socket.send_to(&packet, target);
        }
    }

    #[cfg(test)]
    pub(in crate::runtime) fn captured_osc_feedback(&self) -> Vec<CapturedOscMessage> {
        self.osc_feedback_capture.lock().clone()
    }
}
