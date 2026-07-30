use super::*;

impl ProgrammingResource {
    pub(in crate::runtime) fn remember_selective_import(
        &self,
        context: &light_application::ActionContext,
        target: light_application::SelectiveShowImportUndoTarget,
    ) -> Result<(), light_application::ActionError> {
        self.service.remember_selective_import(context, target)
    }

    pub(in crate::runtime) fn record_command_history(
        &self,
        entry: CommandHistoryEntry,
        limit: usize,
    ) {
        let mut histories = self.command_history.lock();
        let history = histories.entry(entry.desk_id).or_default();
        history.push_front(entry);
        history.truncate(limit);
    }

    pub(in crate::runtime) fn command_history(&self, desk_id: Uuid) -> Vec<CommandHistoryEntry> {
        self.command_history
            .lock()
            .get(&desk_id)
            .map(|history| history.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub(in crate::runtime) fn run_desk_operation<T>(
        &self,
        desk_id: Uuid,
        operation: impl FnOnce() -> T,
    ) -> T {
        self.service.run_desk_operation(desk_id, operation)
    }

    pub(in crate::runtime) fn with_staged_command<T, E>(
        &self,
        session_id: SessionId,
        operation: impl FnOnce(&Self) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<String>,
    {
        self.programmers
            .with_staged_command(session_id, |staged_programmers| {
                let staged = Self {
                    programmers: staged_programmers.clone(),
                    service: self.service.clone(),
                    command_history: Arc::clone(&self.command_history),
                };
                operation(&staged)
            })
    }

    pub(in crate::runtime) fn with_transaction<T, E>(
        &self,
        session_id: SessionId,
        operation: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        self.programmers.with_transaction(session_id, operation)
    }

    pub(in crate::runtime) fn start(
        &self,
        session_id: SessionId,
        user_id: light_core::UserId,
    ) -> light_programmer::ProgrammerState {
        self.programmers.start(session_id, user_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn restore(&self, state: light_programmer::ProgrammerState) {
        self.programmers.restore(state);
    }

    pub(in crate::runtime) fn disconnect(&self, session_id: SessionId) {
        self.programmers.disconnect(session_id);
    }

    pub(in crate::runtime) fn reset_all(&self) {
        self.programmers.reset_all();
    }

    pub(in crate::runtime) fn get(
        &self,
        session_id: SessionId,
    ) -> Option<light_programmer::ProgrammerState> {
        self.programmers.get(session_id)
    }

    pub(in crate::runtime) fn selection(
        &self,
        session_id: SessionId,
    ) -> Option<light_programmer::ProgrammerSelection> {
        self.programmers.selection(session_id)
    }

    pub(in crate::runtime) fn active(&self) -> Vec<light_programmer::ProgrammerState> {
        self.programmers.active()
    }

    #[cfg(test)]
    pub(in crate::runtime) fn active_for_sessions(&self) -> Vec<light_programmer::ProgrammerState> {
        self.programmers.active_for_sessions()
    }

    pub(in crate::runtime) fn active_for_user_sessions(
        &self,
        user_id: light_core::UserId,
    ) -> Vec<light_programmer::ProgrammerState> {
        self.programmers.active_for_user_sessions(user_id)
    }

    pub(in crate::runtime) fn select(
        &self,
        session_id: SessionId,
        fixtures: impl IntoIterator<Item = light_core::FixtureId>,
    ) -> u64 {
        self.programmers.select(session_id, fixtures)
    }

    pub(in crate::runtime) fn select_expression(
        &self,
        session_id: SessionId,
        fixtures: Vec<light_core::FixtureId>,
        expression: light_programmer::SelectionExpression,
    ) -> u64 {
        self.programmers
            .select_expression(session_id, fixtures, expression)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn apply_selection_gesture(
        &self,
        session_id: SessionId,
        references: Vec<light_programmer::SelectionReference>,
        groups: &HashMap<String, light_programmer::GroupDefinition>,
    ) -> bool {
        self.programmers
            .apply_selection_gesture(session_id, references, groups)
    }

    pub(in crate::runtime) fn finish_selection_gesture(&self, session_id: SessionId) -> bool {
        self.programmers.finish_selection_gesture(session_id)
    }

    pub(in crate::runtime) fn set(
        &self,
        session_id: SessionId,
        fixture_id: light_core::FixtureId,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
    ) {
        self.programmers
            .set(session_id, fixture_id, attribute, value);
    }

    pub(in crate::runtime) fn set_many(
        &self,
        session_id: SessionId,
        assignments: impl IntoIterator<
            Item = (
                light_core::FixtureId,
                light_core::AttributeKey,
                light_core::AttributeValue,
            ),
        >,
    ) {
        self.programmers.set_many(session_id, assignments);
    }

    pub(in crate::runtime) fn set_many_faded_with_timing(
        &self,
        session_id: SessionId,
        assignments: impl IntoIterator<
            Item = (
                light_core::FixtureId,
                light_core::AttributeKey,
                light_core::AttributeValue,
            ),
        >,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) {
        self.programmers.set_many_faded_with_timing(
            session_id,
            assignments,
            fade_millis,
            delay_millis,
        );
    }

    pub(in crate::runtime) fn set_many_immediate_with_delay(
        &self,
        session_id: SessionId,
        assignments: impl IntoIterator<
            Item = (
                light_core::FixtureId,
                light_core::AttributeKey,
                light_core::AttributeValue,
            ),
        >,
        delay_millis: Option<u64>,
    ) {
        self.programmers
            .set_many_immediate_with_delay(session_id, assignments, delay_millis);
    }

    pub(in crate::runtime) fn set_faded_with_timing(
        &self,
        session_id: SessionId,
        fixture_id: light_core::FixtureId,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) {
        self.programmers.set_faded_with_timing(
            session_id,
            fixture_id,
            attribute,
            value,
            fade_millis,
            delay_millis,
        );
    }

    #[cfg(test)]
    pub(in crate::runtime) fn set_group(
        &self,
        session_id: SessionId,
        group_id: String,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
    ) -> bool {
        self.programmers
            .set_group(session_id, group_id, attribute, value)
    }

    pub(in crate::runtime) fn set_group_faded_with_timing(
        &self,
        session_id: SessionId,
        group_id: String,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) -> bool {
        self.programmers.set_group_faded_with_timing(
            session_id,
            group_id,
            attribute,
            value,
            fade_millis,
            delay_millis,
        )
    }

    pub(in crate::runtime) fn set_group_immediate_with_delay(
        &self,
        session_id: SessionId,
        group_id: String,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
        delay_millis: Option<u64>,
    ) -> bool {
        self.programmers.set_group_immediate_with_delay(
            session_id,
            group_id,
            attribute,
            value,
            delay_millis,
        )
    }

    pub(in crate::runtime) fn set_transient_action(
        &self,
        session_id: SessionId,
        source: String,
        assignments: impl IntoIterator<
            Item = (
                light_core::FixtureId,
                light_core::AttributeKey,
                light_core::AttributeValue,
            ),
        >,
    ) -> Option<u64> {
        self.programmers
            .set_transient_action(session_id, source, assignments)
    }

    pub(in crate::runtime) fn release_transient_action(
        &self,
        session_id: SessionId,
        source: &str,
        generation: Option<u64>,
    ) -> bool {
        self.programmers
            .release_transient_action(session_id, source, generation)
    }

    pub(in crate::runtime) fn set_modes(
        &self,
        session_id: SessionId,
        blind: Option<bool>,
        preview: Option<bool>,
        highlight: Option<bool>,
        active_context: Option<Option<String>>,
    ) -> bool {
        self.programmers
            .set_modes(session_id, blind, preview, highlight, active_context)
    }

    pub(in crate::runtime) fn clear(&self, session_id: SessionId) -> bool {
        self.programmers.clear(session_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn clear_normal_values(&self, session_id: SessionId) -> bool {
        self.programmers.clear_normal_values(session_id)
    }

    pub(in crate::runtime) fn undo(&self, session_id: SessionId) -> bool {
        self.programmers.undo(session_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn redo(&self, session_id: SessionId) -> bool {
        self.programmers.redo(session_id)
    }

    pub(in crate::runtime) fn clock(&self) -> light_core::SharedClock {
        self.programmers.clock()
    }

    pub(in crate::runtime) fn interaction_version(
        &self,
        session_id: SessionId,
    ) -> Option<light_programmer::ProgrammerInteractionVersion> {
        self.programmers.interaction_version(session_id)
    }

    pub(in crate::runtime) fn command_line_state(
        &self,
        session_id: SessionId,
    ) -> Option<light_programmer::CommandLineState> {
        self.programmers.command_line_state(session_id)
    }

    pub(in crate::runtime) fn set_command_line(
        &self,
        session_id: SessionId,
        command_line: String,
    ) -> bool {
        self.programmers.set_command_line(session_id, command_line)
    }

    pub(in crate::runtime) fn update_command_line<F>(
        &self,
        session_id: SessionId,
        update: F,
    ) -> Option<light_programmer::CommandLineState>
    where
        F: FnOnce(
            &light_programmer::CommandLineState,
        ) -> (String, light_programmer::CommandTarget, bool),
    {
        self.programmers.update_command_line(session_id, update)
    }

    pub(in crate::runtime) fn set_command_target(
        &self,
        session_id: SessionId,
        target: String,
    ) -> bool {
        self.programmers.set_command_target(session_id, target)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn command_target(&self, session_id: SessionId) -> String {
        self.programmers.command_target(session_id)
    }

    pub(in crate::runtime) fn complete_command_execution(
        &self,
        session_id: SessionId,
        final_text: Option<&str>,
        pending_choice: Option<light_programmer::PendingCommandChoice>,
    ) -> Option<light_programmer::CommandLineState> {
        self.programmers
            .complete_command_execution(session_id, final_text, pending_choice)
    }

    pub(in crate::runtime) fn has_pending_command_choices_except_context(
        &self,
        excluded: Option<SessionId>,
    ) -> bool {
        self.programmers
            .has_pending_command_choices_except_context(excluded)
    }

    pub(in crate::runtime) fn clear_pending_command_choices_except_context(
        &self,
        excluded: Option<SessionId>,
    ) -> usize {
        self.programmers
            .clear_pending_command_choices_except_context(excluded)
    }

    pub(in crate::runtime) fn attach_command_context(
        &self,
        session_id: SessionId,
        context: SessionId,
    ) -> bool {
        self.programmers.attach_command_context(session_id, context)
    }

    pub(in crate::runtime) fn finish_selection_gesture_within_interaction(
        &self,
        session_id: SessionId,
    ) -> bool {
        self.programmers
            .finish_selection_gesture_within_interaction(session_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn activate_preload(&self, session_id: SessionId) -> bool {
        self.programmers.activate_preload(session_id)
    }

    pub(in crate::runtime) fn activate_preload_at(
        &self,
        session_id: SessionId,
        committed_at: chrono::DateTime<chrono::Utc>,
    ) -> bool {
        self.programmers
            .activate_preload_at(session_id, committed_at)
    }

    pub(in crate::runtime) fn release_preload(&self, session_id: SessionId) -> bool {
        self.programmers.release_preload(session_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn arm_preload(
        &self,
        session_id: SessionId,
        capture_programmer: bool,
    ) -> bool {
        self.programmers.arm_preload(session_id, capture_programmer)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn set_preload_group(
        &self,
        session_id: SessionId,
        group_id: String,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
    ) -> bool {
        self.programmers
            .set_preload_group(session_id, group_id, attribute, value)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn queue_preload_playback_action(
        &self,
        session_id: SessionId,
        playback_number: u16,
        page: Option<u8>,
        action: light_programmer::PreloadPlaybackQueueAction,
        surface: light_programmer::PreloadPlaybackQueueSurface,
    ) -> bool {
        self.programmers.queue_preload_playback_action(
            session_id,
            playback_number,
            page,
            action,
            surface,
        )
    }

    pub(in crate::runtime) fn queue_preload_playback_action_with_origin(
        &self,
        session_id: SessionId,
        playback_number: u16,
        page: Option<u8>,
        action: light_programmer::PreloadPlaybackQueueAction,
        surface: light_programmer::PreloadPlaybackQueueSurface,
        origin_desk_id: Option<Uuid>,
    ) -> bool {
        self.programmers.queue_preload_playback_action_with_origin(
            session_id,
            playback_number,
            page,
            action,
            surface,
            origin_desk_id,
        )
    }

    pub(in crate::runtime) fn preload_playback_actions(
        &self,
        session_id: SessionId,
    ) -> Option<Vec<light_programmer::PreloadPlaybackAction>> {
        self.programmers.preload_playback_actions(session_id)
    }

    pub(in crate::runtime) fn take_preload_playback_actions(
        &self,
        session_id: SessionId,
    ) -> Vec<light_programmer::PreloadPlaybackAction> {
        self.programmers.take_preload_playback_actions(session_id)
    }

    pub(in crate::runtime) fn capture_mode(
        &self,
        session_id: SessionId,
    ) -> Option<light_programmer::ProgrammerCaptureMode> {
        self.programmers.capture_mode(session_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn normal_values_revision(&self, user_id: light_core::UserId) -> u64 {
        self.programmers.normal_values_revision(user_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn preload_values_revision(&self, user_id: light_core::UserId) -> u64 {
        self.programmers.preload_values_revision(user_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn preload_playback_queue_revision(
        &self,
        user_id: light_core::UserId,
    ) -> u64 {
        self.programmers.preload_playback_queue_revision(user_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn capture_mode_revision(&self, user_id: light_core::UserId) -> u64 {
        self.programmers.capture_mode_revision(user_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn priority_revision(&self, user_id: light_core::UserId) -> u64 {
        self.programmers.priority_revision(user_id)
    }

    pub(in crate::runtime) fn handle(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingCommand>,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingResult, light_application::ActionError> {
        self.service.handle(action, ports)
    }

    pub(in crate::runtime) fn snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingLiveSnapshot, light_application::ActionError> {
        self.service.snapshot(context, ports)
    }

    pub(in crate::runtime) fn handle_values(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingValuesRequest>,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingValuesResult, light_application::ActionError> {
        self.service.handle_values(action, ports)
    }

    pub(in crate::runtime) fn values_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingValuesSnapshot, light_application::ActionError> {
        self.service.values_snapshot(context, ports)
    }

    pub(in crate::runtime) fn capture_mode_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingCaptureModeSnapshot, light_application::ActionError>
    {
        self.service.capture_mode_snapshot(context, ports)
    }

    pub(in crate::runtime) fn handle_priority(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingPriorityRequest>,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingPriorityResult, light_application::ActionError> {
        self.service.handle_priority(action, ports)
    }

    pub(in crate::runtime) fn priority_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingPrioritySnapshot, light_application::ActionError>
    {
        self.service.priority_snapshot(context, ports)
    }

    pub(in crate::runtime) fn handle_preload_values(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ProgrammingPreloadValuesRequest,
        >,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingPreloadValuesResult, light_application::ActionError>
    {
        self.service.handle_preload_values(action, ports)
    }

    pub(in crate::runtime) fn preload_values_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingPreloadValuesSnapshot, light_application::ActionError>
    {
        self.service.preload_values_snapshot(context, ports)
    }

    pub(in crate::runtime) fn preload_playback_queue_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<
        light_application::ProgrammingPreloadPlaybackQueueSnapshot,
        light_application::ActionError,
    > {
        self.service.preload_playback_queue_snapshot(context, ports)
    }

    pub(in crate::runtime) fn handle_preload_lifecycle(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ProgrammingPreloadLifecycleRequest,
        >,
        ports: &dyn light_application::ProgrammingPreloadLifecyclePorts,
    ) -> Result<light_application::ProgrammingPreloadLifecycleResult, light_application::ActionError>
    {
        self.service.handle_preload_lifecycle(action, ports)
    }

    pub(in crate::runtime) fn handle_preset_recall(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ProgrammingPresetRecallRequest,
        >,
        ports: &dyn light_application::ProgrammingPresetRecallPorts,
    ) -> Result<light_application::ProgrammingPresetRecallResult, light_application::ActionError>
    {
        self.service.handle_preset_recall(action, ports)
    }

    pub(in crate::runtime) fn handle_preset_recording(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ProgrammingPresetRecordRequest,
        >,
        ports: &dyn light_application::ProgrammingPresetRecordingPorts,
    ) -> Result<light_application::ProgrammingPresetRecordResult, light_application::ActionError>
    {
        self.service.handle_preset_recording(action, ports)
    }

    pub(in crate::runtime) fn record_preset_within_interaction(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ProgrammingPresetRecordRequest,
        >,
        ports: &dyn light_application::ProgrammingPresetRecordingPorts,
    ) -> Result<light_application::ProgrammingPresetRecordResult, light_application::ActionError>
    {
        self.service.record_preset_within_interaction(action, ports)
    }

    pub(in crate::runtime) fn handle_group_recording(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingGroupRecordRequest>,
        ports: &dyn light_application::ProgrammingGroupRecordingPorts,
    ) -> Result<light_application::ProgrammingGroupRecordResult, light_application::ActionError>
    {
        self.service.handle_group_recording(action, ports)
    }

    pub(in crate::runtime) fn record_group_within_interaction(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingGroupRecordRequest>,
        ports: &dyn light_application::ProgrammingGroupRecordingPorts,
    ) -> Result<light_application::ProgrammingGroupRecordResult, light_application::ActionError>
    {
        self.service.record_group_within_interaction(action, ports)
    }

    pub(in crate::runtime) fn handle_group_management(
        &self,
        action: light_application::ActionEnvelope<light_application::GroupManagementRequest>,
        ports: &dyn light_application::GroupManagementPorts,
    ) -> Result<light_application::GroupManagementResult, light_application::ActionError> {
        self.service.handle_group_management(action, ports)
    }

    pub(in crate::runtime) fn install_frozen_group_selection(
        &self,
        context: &light_application::ActionContext,
        session_id: SessionId,
        selection: &light_application::GroupManagementSelection,
    ) {
        self.service
            .install_frozen_group_selection(context, session_id, selection);
    }

    pub(in crate::runtime) fn handle_cue_recording(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingCueRecordRequest>,
        ports: &dyn light_application::ProgrammingCueRecordingPorts,
    ) -> Result<light_application::ProgrammingCueRecordResult, light_application::ActionError> {
        self.service.handle_cue_recording(action, ports)
    }

    pub(in crate::runtime) fn record_cue_within_interaction(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingCueRecordRequest>,
        ports: &dyn light_application::ProgrammingCueRecordingPorts,
    ) -> Result<light_application::ProgrammingCueRecordResult, light_application::ActionError> {
        self.service.record_cue_within_interaction(action, ports)
    }

    pub(in crate::runtime) fn lifecycle_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingLifecycleSnapshot, light_application::ActionError>
    {
        self.service.lifecycle_snapshot(context, ports)
    }

    pub(in crate::runtime) fn run_lifecycle_transition<T>(
        &self,
        context: &light_application::ActionContext,
        user_id: light_core::UserId,
        operation: impl FnOnce() -> T,
    ) -> T {
        self.service
            .run_lifecycle_transition(context, user_id, operation)
    }

    pub(in crate::runtime) fn handle_cue_deletion<
        P: light_application::ProgrammingCueDeletionPorts,
    >(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingCueDeletionRequest>,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueDeletionResult, light_application::ActionError>
    {
        self.service
            .handle_cue_deletion(action, &active_show.service, ports)
    }

    pub(in crate::runtime) fn delete_cue_within_interaction<
        P: light_application::ProgrammingCueDeletionPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        request: &light_application::ProgrammingCueDeletionRequest,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueDeletionOutcome, light_application::ActionError>
    {
        self.service
            .delete_cue_within_interaction(context, request, &active_show.service, ports)
    }

    pub(in crate::runtime) fn handle_cue_transfer<
        P: light_application::ProgrammingCueTransferPorts,
    >(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingCueTransferRequest>,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueTransferResult, light_application::ActionError>
    {
        self.service
            .handle_cue_transfer(action, &active_show.service, ports)
    }

    pub(in crate::runtime) fn prepare_cue_transfer_choice_within_interaction<
        P: light_application::ProgrammingCueTransferPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        request: light_application::ProgrammingCueTransferChoiceRequest,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_programmer::CueMoveCopyChoice, light_application::ActionError> {
        self.service.prepare_cue_transfer_choice_within_interaction(
            context,
            request,
            &active_show.service,
            ports,
        )
    }

    pub(in crate::runtime) fn cue_transfer_within_interaction<
        P: light_application::ProgrammingCueTransferPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        request: &light_application::ProgrammingCueTransferRequest,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueTransferOutcome, light_application::ActionError>
    {
        self.service
            .cue_transfer_within_interaction(context, request, &active_show.service, ports)
    }

    pub(in crate::runtime) fn current_cue_transfer_within_interaction<
        P: light_application::ProgrammingCueTransferPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        request: &light_application::ProgrammingCueTransferChoiceRequest,
        mode: light_application::ProgrammingCueTransferMode,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueTransferOutcome, light_application::ActionError>
    {
        self.service.current_cue_transfer_within_interaction(
            context,
            request,
            mode,
            &active_show.service,
            ports,
        )
    }

    pub(in crate::runtime) fn run_external_interaction<T>(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
        operation: impl FnOnce() -> T,
    ) -> Result<light_application::ProgrammingInteractionResult<T>, light_application::ActionError>
    {
        self.service
            .run_external_interaction(context, ports, operation)
    }

    pub(in crate::runtime) fn run_selection_refresh<T>(
        &self,
        context: &light_application::ActionContext,
        targets: impl IntoIterator<Item = light_application::ProgrammingSelectionTarget>,
        operation: impl FnOnce() -> T,
    ) -> light_application::ProgrammingSelectionRefreshResult<T> {
        self.service
            .run_selection_refresh(context, targets, operation)
    }

    pub(in crate::runtime) fn run_selection_refresh_with_owned_target<T>(
        &self,
        context: &light_application::ActionContext,
        owned_target: light_application::ProgrammingSelectionTarget,
        targets: impl IntoIterator<Item = light_application::ProgrammingSelectionTarget>,
        operation: impl FnOnce() -> T,
    ) -> light_application::ProgrammingSelectionRefreshResult<T> {
        self.service.run_selection_refresh_with_owned_target(
            context,
            owned_target,
            targets,
            operation,
        )
    }

    pub(in crate::runtime) fn replace_user_programmer<T>(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
        target: light_application::ProgrammingLifecycleTarget,
        operation: impl FnOnce() -> light_application::ProgrammingLifecycleCompletion<T>,
    ) -> Result<light_application::ProgrammingLifecycleResult<T>, light_application::ActionError>
    {
        self.service
            .replace_user_programmer(context, ports, target, operation)
    }

    pub(in crate::runtime) fn update_within_interaction<
        P: light_application::programming_update::ProgrammingUpdatePorts,
    >(
        &self,
        action: light_application::ActionEnvelope<
            light_application::programming_update::ProgrammingUpdateCommand,
        >,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<
        light_application::programming_update::ProgrammingUpdateResult,
        light_application::ActionError,
    > {
        self.service
            .update_within_interaction(action, &active_show.service, ports)
    }

    pub(in crate::runtime) fn update_targets<
        P: light_application::programming_update::ProgrammingUpdatePorts,
    >(
        &self,
        action: light_application::ActionEnvelope<
            light_application::programming_update::ProgrammingUpdateTargetsRequest,
        >,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<
        light_application::programming_update::ProgrammingUpdateTargetsResult,
        light_application::ActionError,
    > {
        self.service
            .update_targets(action, &active_show.service, ports)
    }

    pub(in crate::runtime) fn preview_update<
        P: light_application::programming_update::ProgrammingUpdatePorts,
    >(
        &self,
        action: light_application::ActionEnvelope<
            light_application::programming_update::ProgrammingUpdatePreviewRequest,
        >,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<
        light_application::programming_update::ProgrammingUpdatePreviewResult,
        light_application::ActionError,
    > {
        self.service
            .preview_update(action, &active_show.service, ports)
    }

    pub(in crate::runtime) fn handle_update<
        P: light_application::programming_update::ProgrammingUpdatePorts,
    >(
        &self,
        action: light_application::ActionEnvelope<
            light_application::programming_update::ProgrammingUpdateCommand,
        >,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<
        light_application::programming_update::ProgrammingUpdateResult,
        light_application::ActionError,
    > {
        self.service
            .handle_update(action, &active_show.service, ports)
    }
}
