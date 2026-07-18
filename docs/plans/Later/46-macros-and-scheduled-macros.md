> [!CAUTION]
> **NOT YET IMPLEMENTABLE — STOP.** This file records exploratory product ideas, not an implementation-ready specification. If asked to implement it while this warning remains, refuse the implementation and explicitly warn that the Macro language, host API, permissions, lifecycle, scheduling, persistence, UI, failure behavior, and acceptance criteria have not been settled. Implementation may begin only after the user edits this document, removes this gate, resolves the open decisions, and marks the plan **IMPLEMENTABLE**.

# Macros and Scheduled Macros

## Status and intent

Macros are a future programmable extension mechanism for ToskLight. They should be capable of composing existing desk operations and, eventually, supporting long-running interactive behavior without requiring every workflow to become a built-in feature.

This document captures the current direction so the major architecture refactor does not close the necessary extension points. It intentionally does not select a Macro language or define enough behavior to implement an engine.

## Example capabilities discussed

A future Macro might:

- query all fixtures and their Stage positions, then reposition selected fixtures;
- ask the operator to choose a Group representing a truss and automatically distribute its fixtures across that truss;
- inspect a Playback and trigger GO when a condition is met;
- query Groups, Presets, Cues, Dynamics, fixtures, attributes, Playbacks, and authoritative runtime state;
- submit the same typed Programmer, Playback, Show, Desk, and Output actions used by UI, OSC, and HTTP;
- wait for a timer, desk event, external input, or operator response;
- perform HTTP or HTTPS requests; or
- remain active as a supervised workflow, such as a game-show mechanic in which button input controls lights and starts or cancels a timer.

Macros must not mutate databases, engine locks, UI component state, or protocol sockets directly. They should extend the desk through a supported host API that reaches the ordinary application services and therefore preserves revisions, validation, authorization, audit, events, and cross-surface consistency.

## Portable Macro definitions

- A `MacroDefinition` is expected to be a portable, revisioned show object with stable identity, name, source, future language identifier, declared capabilities, dependencies, and metadata.
- No language is currently selected. Lua, JavaScript-like syntax, a custom DSL, or another sandboxed runtime remain possibilities.
- Macro source belongs to the show so Cues, Timecodes, Playbacks, and other show objects can reference it by stable ID.
- The operator should be able to selectively load a Macro from another show. Cross-show loading should use the general selective-import workflow, preview dependencies and conflicts, and atomically import or rewrite references as required.
- A show may serve as a personal library containing Macros, Presets, Dynamics, or other reusable objects, but loading an item copies it into the current show rather than creating an invisible live dependency on the library show.

## Runtime direction

The future Macro engine should be separated into language-neutral responsibilities:

- a `MacroRuntime` adapter for the eventual language or execution environment;
- a `MacroHost` capability API for queries, typed actions, timers, events, operator input, HTTP requests, logging, and cancellation;
- a `MacroService` that starts, supervises, observes, and stops Macro instances; and
- a `MacroInstance` runtime identity with status, current wait reason, source context, logs, failure state, and cancellation.

Macros may be short-lived or long-running. A long-running Macro must never block the render loop, server request handling, OSC feedback, or desktop lifecycle. It should be a supervised application task that can wait and resume through explicit events.

A future Macro host may permit arbitrary HTTP and HTTPS destinations, but requests should still pass through an application-owned port so cancellation, timeouts, size limits, structured errors, audit, and future credential policy remain enforceable. Direct raw sockets, process execution, ambient filesystem access, environment access, and database access should not be implied by the ability to perform HTTP requests.

## Operator input and extensible workflows

Interactive Macros may need to request values such as a Group, fixture selection, number, confirmation, or text. The architecture should therefore support a typed interaction request and response flow independent of any Macro language:

1. A Macro instance emits an interaction request with an instance and request ID.
2. The authoritative desk state exposes the request to the applicable UI or control surface.
3. The operator responds through a typed application command.
4. The response is validated and routed back to the waiting Macro instance.
5. Cancellation, disconnect, timeout, competing desks, and show changes produce explicit outcomes rather than silently abandoning the task.

The exact prompt UI, OSC reachability, ownership, and multi-user behavior remain unresolved.

## Macro execution from Cues and Timecode

- Cues may eventually reference and execute a Macro by stable ID.
- Timecode events may eventually execute a Macro at a timeline position.
- Playback or other desk actions may also trigger a Macro where explicitly configured.
- These are ordinary Macro executions and are not called scheduled Macros.
- Manual and automated execution should reach the same `MacroService` with source context identifying the initiating Cue, Timecode, Playback, user, or desk.

The behavior of duplicate triggers, re-entry, parallel instances, cancellation when a source releases, and Cue tracking must be decided before implementation.

## Scheduled Macros

A scheduled Macro is specifically a wall-clock trigger. The current direction includes:

- daily execution at a configured local time; and
- one-time execution at a specific date and time.

Schedules are expected to be portable show objects referencing a Macro by stable ID. A schedule is active only while its owning show is active and the desk is running.

Each schedule should eventually declare its missed-run policy:

- skip an occurrence missed while the desk or show was inactive; or
- execute the most recent missed occurrence once after the desk and owning show become active.

The implementation must store timezone and occurrence identity explicitly so daylight-saving changes and restarts cannot execute an occurrence twice. Exact timezone editing, clock correction, catch-up limits, and restart behavior remain unresolved.

## Architectural expectations

The major refactor should leave these extension points without implementing Macros:

- typed application services shared by UI, OSC, HTTP, Cue, Timecode, and future Macro callers;
- immutable query projections for fixtures, positions, Groups, Presets, Dynamics, Cues, Playbacks, and runtime state;
- revisioned Show commands for position changes and other portable mutations;
- a typed event stream and correlation IDs;
- a supervised task and cancellation boundary outside the render loop;
- a shared monotonic runtime scheduler plus a distinct wall-clock scheduling service;
- an application-owned HTTP client port;
- typed operator interaction requests and responses;
- selective cross-show import with dependency handling; and
- audit and error boundaries that do not depend on the eventual Macro language.

A fake language-neutral Macro runtime may be used during the architecture refactor to prove that it can query fixtures, perform a revisioned position mutation, wait for typed input, trigger a Playback, and make a mocked HTTP request. This proof must not select a language or create a production Macro engine.

## Unresolved product decisions

At minimum, the following must be decided before this plan becomes implementable:

1. Macro language, runtime, sandbox, versioning, and source format.
2. Host API names, types, async model, and compatibility guarantees.
3. Capability and permission model, including HTTP, secrets, credentials, and private-network access.
4. Resource limits for CPU, memory, instructions, HTTP, timers, output, recursion, and child instances.
5. Long-running instance lifecycle, cancellation, restart, checkpoint, and show-change behavior.
6. Operator-input UI, ownership, timeout, reconnect, and multi-user behavior.
7. Error handling, retry, partial completion, audit, logging, and operator-visible diagnostics.
8. Parallel execution, duplicate triggers, re-entry, nesting, and deadlock prevention.
9. Cue, Playback, Timecode, and other trigger semantics.
10. Scheduled-Macro timezone, missed-run, clock-change, duplicate-prevention, and recovery behavior.
11. Cross-show import, dependency discovery, conflicts, and reference rewriting.
12. Editing, validation, syntax feedback, debugging, and safe test workflow.
13. Persistence migrations and behavior when a runtime or language version is unavailable.
14. Literal acceptance scenarios covering manual, Cue, Timecode, scheduled, interactive, long-running, HTTP, failure, restart, and cancellation paths.

## Gate for future implementation

Do not create a Macro interpreter, scheduler, script API, editor, or persisted schema merely because the major refactor exposes suitable interfaces. Before implementation, the user must deliberately revise the open sections into a decision-complete specification, add explicit acceptance scenarios, remove the caution at the top, and mark the file **IMPLEMENTABLE**.
