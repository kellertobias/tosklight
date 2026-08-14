use super::*;
use light_wire::v2::runtime::RuntimeSessionRole;

#[derive(Clone)]
pub(in crate::runtime) struct SessionResource {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    session_clients: Arc<RwLock<HashMap<SessionId, Uuid>>>,
    file_input_contexts: Arc<Mutex<HashMap<Uuid, file_manager::FileInputContext>>>,
    /// Sessions that hold a non-default role. Kept beside the session record so the historical
    /// operator session stays exactly as it was.
    roles: Arc<RwLock<HashMap<SessionId, RuntimeSessionRole>>>,
    visualizer_connections: Arc<RwLock<HashMap<SessionId, usize>>>,
}

pub(crate) enum SessionFileInputRoute {
    Unclaimed,
    Claimed,
    Dispatch(file_manager::FileInputContext),
}

impl SessionResource {
    pub(in crate::runtime) fn new() -> Self {
        Self {
            sessions: Arc::default(),
            session_clients: Arc::default(),
            file_input_contexts: Arc::default(),
            roles: Arc::default(),
            visualizer_connections: Arc::default(),
        }
    }

    /// Record the role a session was created with.
    pub(in crate::runtime) fn set_role(&self, id: SessionId, role: RuntimeSessionRole) {
        if role == RuntimeSessionRole::Operator {
            self.roles.write().remove(&id);
            return;
        }
        self.roles.write().insert(id, role);
    }

    /// The session's role, defaulting to the historical operator session.
    pub(in crate::runtime) fn role(&self, id: SessionId) -> RuntimeSessionRole {
        self.roles.read().get(&id).copied().unwrap_or_default()
    }

    pub(in crate::runtime) fn session(&self, id: SessionId) -> Option<Session> {
        self.sessions.read().get(&id).cloned()
    }

    pub(in crate::runtime) fn session_for_token(&self, token: &str) -> Option<Session> {
        self.sessions
            .read()
            .values()
            .find(|session| session.token == token)
            .cloned()
    }

    pub(in crate::runtime) fn session_token_matches(&self, id: SessionId, token: &str) -> bool {
        self.sessions
            .read()
            .get(&id)
            .is_some_and(|session| session.token == token)
    }

    pub(in crate::runtime) fn sessions(&self) -> Vec<Session> {
        self.sessions.read().values().cloned().collect()
    }

    pub(in crate::runtime) fn set_visualizer_connected(&self, id: SessionId, connected: bool) {
        if connected {
            *self.visualizer_connections.write().entry(id).or_default() += 1;
        } else {
            let mut connections = self.visualizer_connections.write();
            let Some(count) = connections.get_mut(&id) else {
                return;
            };
            *count -= 1;
            if *count == 0 {
                connections.remove(&id);
            }
        }
    }

    pub(in crate::runtime) fn has_visualizer_connection(&self) -> bool {
        // Visualizer views are installation-level presentation state, so a live renderer is
        // available to every desk surface attached to this server rather than one command-line
        // context.
        !self.visualizer_connections.read().is_empty()
    }

    pub(in crate::runtime) fn insert_session(&self, session: Session) -> Option<Session> {
        self.sessions.write().insert(session.id, session)
    }

    pub(in crate::runtime) fn remove_session(&self, id: SessionId) -> Option<Session> {
        self.roles.write().remove(&id);
        self.sessions.write().remove(&id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn contains_session(&self, id: SessionId) -> bool {
        self.sessions.read().contains_key(&id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    pub(in crate::runtime) fn update_desk_sessions(&self, desk: &ControlDesk) {
        for session in self
            .sessions
            .write()
            .values_mut()
            .filter(|session| session.desk.id == desk.id)
        {
            session.desk = desk.clone();
        }
    }

    #[cfg(test)]
    pub(in crate::runtime) fn update_session_desk(
        &self,
        session_id: SessionId,
        desk: ControlDesk,
    ) -> bool {
        let mut sessions = self.sessions.write();
        let Some(session) = sessions.get_mut(&session_id) else {
            return false;
        };
        session.desk = desk;
        true
    }

    pub(in crate::runtime) fn bind_client(&self, session_id: SessionId, client_id: Uuid) {
        self.session_clients.write().insert(session_id, client_id);
    }

    pub(in crate::runtime) fn unbind_client(&self, session_id: SessionId) -> Option<Uuid> {
        self.session_clients.write().remove(&session_id)
    }

    pub(in crate::runtime) fn client_id(&self, session_id: SessionId) -> Option<Uuid> {
        self.session_clients.read().get(&session_id).copied()
    }

    pub(in crate::runtime) fn has_bound_client(&self, session_id: SessionId) -> bool {
        self.session_clients.read().contains_key(&session_id)
    }

    pub(in crate::runtime) fn client_connected(&self, client_id: Uuid) -> bool {
        let sessions = self.sessions.read();
        let clients = self.session_clients.read();
        sessions
            .keys()
            .any(|session_id| clients.get(session_id) == Some(&client_id))
    }

    pub(in crate::runtime) fn desk_in_use(&self, desk_id: Uuid) -> bool {
        self.sessions
            .read()
            .values()
            .any(|session| session.desk.id == desk_id)
    }

    pub(in crate::runtime) fn client_or_desk_in_use(&self, client_id: Uuid, desk_id: Uuid) -> bool {
        self.client_connected(client_id) || self.desk_in_use(desk_id)
    }

    pub(in crate::runtime) fn same_context_connected(&self, session: &Session) -> bool {
        self.sessions.read().values().any(|candidate| {
            candidate.user.id == session.user.id && candidate.desk.id == session.desk.id
        })
    }

    pub(crate) fn prune_file_input_contexts(&self, now: std::time::Instant) {
        self.file_input_contexts
            .lock()
            .retain(|_, context| context.expires_at > now);
    }

    pub(crate) fn try_claim_file_input_context(
        &self,
        context: file_manager::FileInputContext,
        prepare: impl FnOnce() -> Result<(), ApiError>,
    ) -> Result<(), ApiError> {
        let mut contexts = self.file_input_contexts.lock();
        contexts.retain(|_, current| current.expires_at > std::time::Instant::now());
        if let Some(existing) = contexts.get(&context.desk_id)
            && existing.instance_id != context.instance_id
        {
            return Err(ApiError::conflict(
                "another File Manager instance owns this session's file input context",
            ));
        }
        prepare()?;
        contexts.insert(context.desk_id, context);
        Ok(())
    }

    pub(crate) fn file_input_context(
        &self,
        desk_id: Uuid,
    ) -> Option<file_manager::FileInputContext> {
        self.file_input_contexts.lock().get(&desk_id).cloned()
    }

    pub(crate) fn release_file_input_context(
        &self,
        desk_id: Uuid,
        instance_id: Option<&str>,
    ) -> Option<file_manager::FileInputContext> {
        let mut contexts = self.file_input_contexts.lock();
        let matches = contexts.get(&desk_id).is_some_and(|context| {
            instance_id.is_none_or(|instance_id| instance_id == context.instance_id)
        });
        matches.then(|| contexts.remove(&desk_id)).flatten()
    }

    pub(crate) fn route_file_input(
        &self,
        desk_id: Uuid,
        action: &str,
        expires_at: std::time::Instant,
    ) -> SessionFileInputRoute {
        let mut contexts = self.file_input_contexts.lock();
        let Some(context) = contexts.get_mut(&desk_id) else {
            return SessionFileInputRoute::Unclaimed;
        };
        if context.desk_id != desk_id {
            return SessionFileInputRoute::Unclaimed;
        }
        context.expires_at = expires_at;
        if context.action == file_manager::FileInputAction::MacroEdit {
            return SessionFileInputRoute::Dispatch(context.clone());
        }
        if !matches!(action, "enter" | "escape" | "esc") {
            return SessionFileInputRoute::Claimed;
        }
        let context = context.clone();
        if matches!(action, "escape" | "esc") {
            contexts.remove(&desk_id);
        }
        SessionFileInputRoute::Dispatch(context)
    }

    pub(crate) fn release_session_file_input(
        &self,
        session: &Session,
    ) -> Option<file_manager::FileInputContext> {
        let mut contexts = self.file_input_contexts.lock();
        let owned = contexts
            .get(&session.desk.id)
            .is_some_and(|context| context.session_id == session.id);
        owned.then(|| contexts.remove(&session.desk.id)).flatten()
    }

    #[cfg(test)]
    pub(in crate::runtime) fn file_input_context_count(&self) -> usize {
        self.file_input_contexts.lock().len()
    }
}
