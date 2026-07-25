//! Language-neutral application seams for future sandboxed Macros.
//!
//! No Macro language, parser, persistence codec, scheduler policy, or sandbox is selected here.

mod host;
mod http;
mod model;
mod service;

pub use host::{MacroHost, MacroHostBackend};
pub use model::{
    GroupProjection, MacroAuditEntry, MacroAuditedAction, MacroCapability, MacroDefinition,
    MacroDependency, MacroError, MacroErrorKind, MacroEventFilter, MacroExecutionId,
    MacroExecutionOutcome, MacroExecutionPhase, MacroExecutionRequest, MacroExecutionSnapshot,
    MacroHostAction, MacroHttpAuditEvent, MacroHttpFailureKind, MacroHttpPolicy, MacroHttpRequest,
    MacroHttpResponse, MacroHttpTerminal, MacroHttpTransportError, MacroHttpTransportErrorKind,
    MacroHttpTransportResponse, MacroId, MacroInvocation, MacroLanguageId, MacroObservedEvent,
    MacroResume, MacroValue, MacroWaitRequest, MacroWaitState, OperatorInputKind,
    OperatorInputValue,
};
pub use service::{MacroRuntime, MacroService, MacroTask, MacroTaskRunner};

pub use crate::scheduling::CancellationSignal;

#[cfg(test)]
mod tests;
