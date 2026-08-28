//! FIFO, one-shot execution for show-owned command Macros.
//!
//! The service deliberately knows nothing about command grammar or transports. A composition
//! adapter supplies the authoritative prevalidator and command executor for the authenticated
//! operator session that started each run.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    time::Duration,
};

use uuid::Uuid;

use crate::{ActionContext, CommandMacroDefinition};

const EXECUTION_QUEUE_LIMIT: usize = 256;
pub const DEFAULT_MACRO_HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandMacroTrigger {
    Pool,
    Editor,
    Playback {
        playback_number: u16,
    },
    CommandLine,
    Http,
    WebSocket,
    Osc,
    Hardware,
    Schedule,
    Timecode,
    /// A tracking zone: somebody walked into it, or out of it.
    Tracking,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandMacroExecutionState {
    Queued,
    Validating,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl CommandMacroExecutionState {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandMacroExecutionSnapshot {
    pub execution_id: Uuid,
    pub macro_id: Uuid,
    pub macro_number: u16,
    pub macro_name: String,
    pub source_revision: u64,
    pub desk_id: Uuid,
    pub session_id: Uuid,
    pub state: CommandMacroExecutionState,
    pub line: Option<u32>,
    pub statement: Option<u32>,
    pub command: Option<String>,
    pub message: Option<String>,
    pub trigger: CommandMacroTrigger,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandMacroRuntimeSnapshot {
    pub desk_id: Uuid,
    pub active: Vec<CommandMacroExecutionSnapshot>,
    pub recent: Vec<CommandMacroExecutionSnapshot>,
}

#[derive(Clone, Debug)]
pub struct CommandMacroRunRequest {
    pub definition: CommandMacroDefinition,
    pub source_revision: u64,
    pub context: ActionContext,
    pub trigger: CommandMacroTrigger,
    /// One-based source line for the editor's Run line action.
    pub only_line: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandMacroExecutionError {
    pub message: String,
}

impl CommandMacroExecutionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Authenticated adapter for one queued invocation.
///
/// `prevalidate` receives every selected executable line at once and must not mutate desk or show
/// state. `execute` is called only after that complete prevalidation succeeds.
pub trait CommandMacroExecutionHost: Send + Sync + 'static {
    fn prevalidate(
        &self,
        lines: &[CommandMacroOwnedLine],
    ) -> Result<(), CommandMacroExecutionError>;

    fn execute(
        &self,
        execution_id: Uuid,
        macro_id: Uuid,
        macro_revision: u64,
        line: &CommandMacroOwnedLine,
    ) -> Result<(), CommandMacroExecutionError>;

    /// Executes the already-prevalidated sequence while retaining any adapter-owned interaction
    /// gate for the whole run. The default remains suitable for isolated hosts; production desk
    /// adapters override this so manual command edits cannot interleave between lines.
    fn execute_sequence(
        &self,
        execution_id: Uuid,
        macro_id: Uuid,
        macro_revision: u64,
        lines: &[CommandMacroOwnedLine],
        is_cancelled: &dyn Fn() -> bool,
        on_line: &mut dyn FnMut(&CommandMacroOwnedLine),
    ) -> Result<CommandMacroSequenceOutcome, CommandMacroExecutionError> {
        for line in lines {
            if is_cancelled() {
                return Ok(CommandMacroSequenceOutcome::Cancelled);
            }
            on_line(line);
            if let Some(delay_millis) = line.delay_millis {
                if !wait_for_macro_delay(delay_millis, is_cancelled) {
                    return Ok(CommandMacroSequenceOutcome::Cancelled);
                }
                continue;
            }
            self.execute(execution_id, macro_id, macro_revision, line)?;
        }
        Ok(CommandMacroSequenceOutcome::Succeeded)
    }
}

/// Waits on the caller's Macro worker while polling cancellation. The caller retains any desk
/// interaction gate it already owns; output and dynamic engines continue on their own workers.
pub fn wait_for_macro_delay(delay_millis: u64, is_cancelled: &dyn Fn() -> bool) -> bool {
    let mut remaining = delay_millis;
    while remaining > 0 {
        if is_cancelled() {
            return false;
        }
        let slice = remaining.min(10);
        std::thread::sleep(Duration::from_millis(slice));
        remaining -= slice;
    }
    !is_cancelled()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandMacroSequenceOutcome {
    Succeeded,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandMacroOwnedLine {
    pub number: u32,
    pub statement: u32,
    pub command: String,
    pub delay_millis: Option<u64>,
}

#[derive(Clone)]
pub struct CommandMacroExecutionService {
    sender: SyncSender<QueueItem>,
    shared: Arc<Shared>,
}

impl CommandMacroExecutionService {
    pub fn new(history_limit: usize) -> Self {
        let (sender, receiver) = mpsc::sync_channel(EXECUTION_QUEUE_LIMIT);
        let shared = Arc::new(Shared {
            state: Mutex::new(RuntimeState {
                records: BTreeMap::new(),
                recent: VecDeque::new(),
                history_limit,
            }),
        });
        let worker_shared = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("tosklight-command-macros".into())
            .spawn(move || {
                while let Ok(item) = receiver.recv() {
                    run_item(&worker_shared, item);
                }
            })
            .expect("spawn command Macro worker");
        Self { sender, shared }
    }

    pub fn start(
        &self,
        request: CommandMacroRunRequest,
        host: Arc<dyn CommandMacroExecutionHost>,
    ) -> Result<CommandMacroExecutionSnapshot, CommandMacroExecutionError> {
        request
            .definition
            .validate()
            .map_err(CommandMacroExecutionError::new)?;
        // Still a precondition: a Macro runs as the desk's operator, not anonymously.
        let session_id = request.context.session_id.ok_or_else(|| {
            CommandMacroExecutionError::new("Macro execution requires an operator session")
        })?;
        let lines = selected_lines(&request)?;
        let execution_id = Uuid::new_v4();
        let cancellation = Arc::new(AtomicBool::new(false));
        let snapshot = CommandMacroExecutionSnapshot {
            execution_id,
            macro_id: request.definition.id,
            macro_number: request.definition.number,
            macro_name: request.definition.name.clone(),
            source_revision: request.source_revision,
            desk_id: request.context.desk_id,
            session_id,
            state: CommandMacroExecutionState::Queued,
            line: None,
            statement: None,
            command: None,
            message: None,
            trigger: request.trigger,
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
        };
        self.shared.state.lock().unwrap().records.insert(
            execution_id,
            ExecutionRecord {
                snapshot: snapshot.clone(),
                cancellation: Arc::clone(&cancellation),
            },
        );
        if self
            .sender
            .try_send(QueueItem {
                execution_id,
                macro_id: request.definition.id,
                source_revision: request.source_revision,
                lines,
                cancellation,
                host,
            })
            .is_err()
        {
            self.shared
                .state
                .lock()
                .unwrap()
                .records
                .remove(&execution_id);
            return Err(CommandMacroExecutionError::new(
                "Macro execution queue is full or unavailable",
            ));
        }
        Ok(snapshot)
    }

    pub fn cancel(
        &self,
        desk_id: Uuid,
        execution_id: Uuid,
    ) -> Result<CommandMacroExecutionSnapshot, CommandMacroExecutionError> {
        let mut state = self.shared.state.lock().unwrap();
        let record = state
            .records
            .get_mut(&execution_id)
            .ok_or_else(|| CommandMacroExecutionError::new("Macro execution was not found"))?;
        if record.snapshot.desk_id != desk_id {
            return Err(CommandMacroExecutionError::new(
                "Macro execution belongs to another desk",
            ));
        }
        if !record.snapshot.state.is_terminal() {
            record.cancellation.store(true, Ordering::Release);
            if matches!(record.snapshot.state, CommandMacroExecutionState::Queued) {
                finish_record(
                    &mut state,
                    execution_id,
                    CommandMacroExecutionState::Cancelled,
                    Some("Cancelled before execution".into()),
                );
            }
        }
        Ok(state
            .records
            .get(&execution_id)
            .expect("cancelled record remains retained")
            .snapshot
            .clone())
    }

    pub fn execution(
        &self,
        desk_id: Uuid,
        execution_id: Uuid,
    ) -> Option<CommandMacroExecutionSnapshot> {
        self.shared
            .state
            .lock()
            .unwrap()
            .records
            .get(&execution_id)
            .filter(|record| record.snapshot.desk_id == desk_id)
            .map(|record| record.snapshot.clone())
    }

    pub fn snapshot(&self, desk_id: Uuid) -> CommandMacroRuntimeSnapshot {
        let state = self.shared.state.lock().unwrap();
        let mut active = state
            .records
            .values()
            .filter(|record| {
                record.snapshot.desk_id == desk_id && !record.snapshot.state.is_terminal()
            })
            .map(|record| record.snapshot.clone())
            .collect::<Vec<_>>();
        active.sort_by(|left, right| left.started_at.cmp(&right.started_at));
        let recent = state
            .recent
            .iter()
            .filter_map(|execution_id| state.records.get(execution_id))
            .filter(|record| record.snapshot.desk_id == desk_id)
            .map(|record| record.snapshot.clone())
            .collect();
        CommandMacroRuntimeSnapshot {
            desk_id,
            active,
            recent,
        }
    }
}

impl Default for CommandMacroExecutionService {
    fn default() -> Self {
        Self::new(DEFAULT_MACRO_HISTORY_LIMIT)
    }
}

struct Shared {
    state: Mutex<RuntimeState>,
}

struct RuntimeState {
    records: BTreeMap<Uuid, ExecutionRecord>,
    /// Newest terminal execution first.
    recent: VecDeque<Uuid>,
    history_limit: usize,
}

struct ExecutionRecord {
    snapshot: CommandMacroExecutionSnapshot,
    cancellation: Arc<AtomicBool>,
}

struct QueueItem {
    execution_id: Uuid,
    macro_id: Uuid,
    source_revision: u64,
    lines: Vec<CommandMacroOwnedLine>,
    cancellation: Arc<AtomicBool>,
    host: Arc<dyn CommandMacroExecutionHost>,
}

fn selected_lines(
    request: &CommandMacroRunRequest,
) -> Result<Vec<CommandMacroOwnedLine>, CommandMacroExecutionError> {
    let all = crate::compile_macro_source(&request.definition.source)
        .map_err(|error| {
            CommandMacroExecutionError::new(format!(
                "Macro line {} cannot run: {}",
                error.line, error.message
            ))
        })?
        .lines
        .into_iter()
        .enumerate()
        .map(|(statement, line)| CommandMacroOwnedLine {
            number: line.number as u32,
            statement: statement as u32 + 1,
            command: line.command,
            delay_millis: line.delay_millis,
        })
        .collect::<Vec<_>>();
    let Some(only_line) = request.only_line else {
        return Ok(all);
    };
    let selected = all
        .into_iter()
        .filter(|line| line.number == only_line)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        Err({
            CommandMacroExecutionError::new(format!(
                "Macro source line {only_line} is blank, a comment, or does not exist"
            ))
        })
    } else {
        Ok(selected)
    }
}

fn run_item(shared: &Shared, item: QueueItem) {
    if item.cancellation.load(Ordering::Acquire) {
        return;
    }
    update_state(
        shared,
        item.execution_id,
        CommandMacroExecutionState::Validating,
        None,
        None,
    );
    if let Err(error) = item.host.prevalidate(&item.lines) {
        finish(
            shared,
            item.execution_id,
            CommandMacroExecutionState::Failed,
            Some(error.message),
        );
        return;
    }
    if item.cancellation.load(Ordering::Acquire) {
        finish(
            shared,
            item.execution_id,
            CommandMacroExecutionState::Cancelled,
            Some("Cancelled after validation".into()),
        );
        return;
    }
    update_state(
        shared,
        item.execution_id,
        CommandMacroExecutionState::Running,
        None,
        None,
    );
    let cancellation = Arc::clone(&item.cancellation);
    let result = item.host.execute_sequence(
        item.execution_id,
        item.macro_id,
        item.source_revision,
        &item.lines,
        &|| cancellation.load(Ordering::Acquire),
        &mut |line| {
            update_state(
                shared,
                item.execution_id,
                CommandMacroExecutionState::Running,
                Some(line),
                None,
            );
        },
    );
    match result {
        Err(error) => {
            finish(
                shared,
                item.execution_id,
                CommandMacroExecutionState::Failed,
                Some(error.message),
            );
            return;
        }
        Ok(CommandMacroSequenceOutcome::Cancelled) => {
            finish(
                shared,
                item.execution_id,
                CommandMacroExecutionState::Cancelled,
                Some("Cancelled between Macro lines".into()),
            );
            return;
        }
        Ok(CommandMacroSequenceOutcome::Succeeded) => {}
    }
    finish(
        shared,
        item.execution_id,
        CommandMacroExecutionState::Succeeded,
        Some(format!("Executed {} Macro line(s)", item.lines.len())),
    );
}

fn update_state(
    shared: &Shared,
    execution_id: Uuid,
    state: CommandMacroExecutionState,
    line: Option<&CommandMacroOwnedLine>,
    message: Option<String>,
) {
    let mut runtime = shared.state.lock().unwrap();
    let Some(record) = runtime.records.get_mut(&execution_id) else {
        return;
    };
    if record.snapshot.state.is_terminal() {
        return;
    }
    record.snapshot.state = state;
    if let Some(line) = line {
        record.snapshot.line = Some(line.number);
        record.snapshot.statement = Some(line.statement);
        record.snapshot.command = Some(line.command.clone());
    }
    record.snapshot.message = message;
}

fn finish(
    shared: &Shared,
    execution_id: Uuid,
    state: CommandMacroExecutionState,
    message: Option<String>,
) {
    finish_record(
        &mut shared.state.lock().unwrap(),
        execution_id,
        state,
        message,
    );
}

fn finish_record(
    runtime: &mut RuntimeState,
    execution_id: Uuid,
    state: CommandMacroExecutionState,
    message: Option<String>,
) {
    let Some(record) = runtime.records.get_mut(&execution_id) else {
        return;
    };
    if record.snapshot.state.is_terminal() {
        return;
    }
    record.snapshot.state = state;
    record.snapshot.message = message;
    record.snapshot.finished_at = Some(chrono::Utc::now().to_rfc3339());
    runtime.recent.push_front(execution_id);
    while runtime.recent.len() > runtime.history_limit {
        if let Some(expired) = runtime.recent.pop_back() {
            runtime.records.remove(&expired);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Condvar, Mutex},
        time::{Duration, Instant},
    };

    use super::*;
    use crate::ActionSource;

    #[derive(Default)]
    struct RecordingHost {
        events: Mutex<Vec<String>>,
        gate: (Mutex<bool>, Condvar),
        fail: Mutex<Option<String>>,
    }

    impl RecordingHost {
        fn events(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }

        fn release(&self) {
            *self.gate.0.lock().unwrap() = true;
            self.gate.1.notify_all();
        }
    }

    impl CommandMacroExecutionHost for RecordingHost {
        fn prevalidate(
            &self,
            lines: &[CommandMacroOwnedLine],
        ) -> Result<(), CommandMacroExecutionError> {
            self.events
                .lock()
                .unwrap()
                .push(format!("validate:{}", lines.len()));
            if lines.iter().any(|line| line.command == "INVALID") {
                return Err(CommandMacroExecutionError::new("invalid line"));
            }
            Ok(())
        }

        fn execute(
            &self,
            _execution_id: Uuid,
            _macro_id: Uuid,
            _macro_revision: u64,
            line: &CommandMacroOwnedLine,
        ) -> Result<(), CommandMacroExecutionError> {
            self.events.lock().unwrap().push(line.command.clone());
            if line.command == "WAIT" {
                let mut released = self.gate.0.lock().unwrap();
                while !*released {
                    released = self.gate.1.wait(released).unwrap();
                }
            }
            if self.fail.lock().unwrap().as_deref() == Some(line.command.as_str()) {
                return Err(CommandMacroExecutionError::new("command failed"));
            }
            Ok(())
        }
    }

    fn request(source: &str) -> CommandMacroRunRequest {
        CommandMacroRunRequest {
            definition: CommandMacroDefinition {
                id: Uuid::new_v4(),
                number: 1,
                name: "Macro".into(),
                source: source.into(),
                presentation: Default::default(),
            },
            source_revision: 4,
            context: ActionContext::operator(
                Uuid::from_u128(1),
                Uuid::from_u128(3),
                ActionSource::UserInterface,
            ),
            trigger: CommandMacroTrigger::Pool,
            only_line: None,
        }
    }

    fn wait_terminal(
        service: &CommandMacroExecutionService,
        execution_id: Uuid,
    ) -> CommandMacroExecutionSnapshot {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let snapshot = service.execution(Uuid::from_u128(1), execution_id).unwrap();
            if snapshot.state.is_terminal() {
                return snapshot;
            }
            assert!(Instant::now() < deadline, "Macro execution timed out");
            std::thread::yield_now();
        }
    }

    #[test]
    fn prevalidates_every_line_before_mutation() {
        let service = CommandMacroExecutionService::new(10);
        let host = Arc::new(RecordingHost::default());
        let started = service
            .start(request("FIRST\nINVALID\nTHIRD"), host.clone())
            .unwrap();
        let finished = wait_terminal(&service, started.execution_id);
        assert_eq!(finished.state, CommandMacroExecutionState::Failed);
        assert_eq!(host.events(), ["validate:3"]);
    }

    #[test]
    fn invalid_macro_delay_reports_its_source_line_before_the_run_is_queued() {
        let service = CommandMacroExecutionService::new(10);
        let host = Arc::new(RecordingHost::default());
        let error = service
            .start(request("FIRST\nDELAY -1\nTHIRD"), host.clone())
            .unwrap_err();

        assert!(error.message.contains("Macro line 2"), "{}", error.message);
        assert!(error.message.contains("non-negative"), "{}", error.message);
        assert!(host.events().is_empty());
    }

    #[test]
    fn fifo_execution_fails_fast_without_rolling_back_prior_lines() {
        let service = CommandMacroExecutionService::new(10);
        let first = Arc::new(RecordingHost::default());
        *first.fail.lock().unwrap() = Some("SECOND".into());
        let second = Arc::new(RecordingHost::default());
        let first_run = service
            .start(request("FIRST\nSECOND\nTHIRD"), first.clone())
            .unwrap();
        let second_run = service.start(request("AFTER"), second.clone()).unwrap();

        assert_eq!(
            wait_terminal(&service, first_run.execution_id).state,
            CommandMacroExecutionState::Failed
        );
        assert_eq!(
            wait_terminal(&service, second_run.execution_id).state,
            CommandMacroExecutionState::Succeeded
        );
        assert_eq!(first.events(), ["validate:3", "FIRST", "SECOND"]);
        assert_eq!(second.events(), ["validate:1", "AFTER"]);
    }

    #[test]
    fn run_line_executes_each_semicolon_statement_with_unique_identity() {
        let service = CommandMacroExecutionService::new(10);
        let host = Arc::new(RecordingHost::default());
        let mut selected = request("FIRST; SECOND\nTHIRD");
        selected.only_line = Some(1);
        let started = service.start(selected, host.clone()).unwrap();
        let finished = wait_terminal(&service, started.execution_id);
        assert_eq!(finished.state, CommandMacroExecutionState::Succeeded);
        assert_eq!(finished.line, Some(1));
        assert_eq!(finished.statement, Some(2));
        assert_eq!(host.events(), ["validate:2", "FIRST", "SECOND"]);
    }

    #[test]
    fn macro_delay_waits_without_dispatching_a_programmer_command() {
        let service = CommandMacroExecutionService::new(10);
        let host = Arc::new(RecordingHost::default());
        let started_at = Instant::now();
        let started = service
            .start(request("FIRST\nDELAY 0.03\nTHIRD"), host.clone())
            .unwrap();
        let finished = wait_terminal(&service, started.execution_id);

        assert_eq!(finished.state, CommandMacroExecutionState::Succeeded);
        assert!(started_at.elapsed() >= Duration::from_millis(25));
        assert_eq!(host.events(), ["validate:3", "FIRST", "THIRD"]);
    }

    #[test]
    fn cancellation_interrupts_a_macro_delay_before_the_next_line() {
        let service = CommandMacroExecutionService::new(10);
        let host = Arc::new(RecordingHost::default());
        let started = service
            .start(request("FIRST\nDELAY 1\nMUST NOT RUN"), host.clone())
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let snapshot = service
                .execution(Uuid::from_u128(1), started.execution_id)
                .unwrap();
            if snapshot.command.as_deref() == Some("DELAY 1") {
                break;
            }
            assert!(Instant::now() < deadline, "Macro delay did not start");
            std::thread::yield_now();
        }

        let cancelled_at = Instant::now();
        service
            .cancel(Uuid::from_u128(1), started.execution_id)
            .unwrap();
        let finished = wait_terminal(&service, started.execution_id);

        assert_eq!(finished.state, CommandMacroExecutionState::Cancelled);
        assert!(cancelled_at.elapsed() < Duration::from_millis(250));
        assert_eq!(host.events(), ["validate:3", "FIRST"]);
    }

    #[test]
    fn cancellation_is_observed_between_lines_and_history_is_bounded() {
        let service = CommandMacroExecutionService::new(1);
        let host = Arc::new(RecordingHost::default());
        let started = service
            .start(request("WAIT\nMUST NOT RUN"), host.clone())
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while host.events().len() < 2 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        service
            .cancel(Uuid::from_u128(1), started.execution_id)
            .unwrap();
        host.release();
        let finished = wait_terminal(&service, started.execution_id);
        assert_eq!(finished.state, CommandMacroExecutionState::Cancelled);
        assert_eq!(host.events(), ["validate:2", "WAIT"]);

        let next_host = Arc::new(RecordingHost::default());
        let next = service.start(request("NEXT"), next_host).unwrap();
        wait_terminal(&service, next.execution_id);
        let runtime = service.snapshot(Uuid::from_u128(1));
        assert_eq!(runtime.recent.len(), 1);
        assert_eq!(runtime.recent[0].execution_id, next.execution_id);
        assert!(
            service
                .execution(Uuid::from_u128(1), started.execution_id)
                .is_none()
        );
    }
}
