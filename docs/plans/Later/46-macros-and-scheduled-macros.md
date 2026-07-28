# Macros

## Status and ownership

**IMPLEMENTABLE.** The language, package model, execution boundary, permissions, lifecycle,
persistence, editor, interactions, panes, Programmer access, failure behavior, and acceptance
requirements are settled below.

This is the sole plan that defines ToskLight Macro product semantics. Other plans may define how
their own feature refers to or starts a Macro, but must link here instead of redefining Macro
language, packaging, permissions, execution, UI, or lifecycle behavior.

Wall-clock trigger rules remain owned by [Schedules](../Next/53-schedules.md). Selective import
workflow remains owned by [Partial Show Load](../Next/52-partial-show-load.md). Those plans consume
the Macro contracts defined here.

## Goal and boundaries

Macros are operator-authored TypeScript packages that compose the same typed application services
used by the UI, command line, HTTP, OSC, attached hardware, Cues, and Playbacks.

A Macro may query and change everything inside the active show, including fixtures, Patch, Stage
positions, selection, Groups, Presets, Dynamics, Cues, Cuelists, Playbacks, Programmer/Preload,
show-owned layouts, other Macro entrypoints, and future Schedules. Every operation must use the
ordinary typed service so revisions, validation, undo, events, audit, persistence, and live output
semantics remain authoritative.

A Macro cannot:

- change desk, user, authentication, security, or installation settings;
- create, save, load, switch, overwrite, or delete shows;
- edit its own or another Macro's source, manifest, permissions, or library entry;
- access a database, engine lock, protocol socket, process, environment variable, arbitrary
  filesystem path, browser DOM, React component, or raw application state;
- import code from npm, a URL, another Macro package, or the desk filesystem; or
- execute from Dynamics or the DMX render path.

Macros may inspect, edit, start, and stop Dynamics. Dynamics never call Macro functions and no
JavaScript runs for a Dynamic or output sample.

## Language and package model

### Runtime and compiler

- Macro source is TypeScript executed as JavaScript in QuickJS through `rquickjs`.
- The server bundles the native TypeScript compiler for every supported headless target. A
  production desk does not require Node.
- Compiler output targets ES2021 modules with `strictNullChecks: true`,
  `noImplicitAny: false`, no JSX, and no decorators. Operators may add types but do not have to
  annotate every value.
- Monaco provides live diagnostics, completion, navigation, generated ToskLight SDK types, and
  local/show diffs. Server validation remains authoritative.
- QuickJS is an in-process containment boundary for trusted operator code that may be buggy. It is
  not presented as an operating-system security boundary for hostile code.

Before production schema work, a bounded engine proof must validate async promises, virtual module
loading, hard interruption, memory and stack limits, cancellation, and disposal on macOS arm64 and
x64, Linux arm64 and x64, and Windows. Do not enable an `rquickjs` allocator feature that makes its
memory limit ineffective. If supported-target packaging or hard interruption fails, stop and
report the blocker rather than silently selecting another engine.

### Portable package

A `MacroPackage` has:

- a stable UUID and operator-facing name;
- a declarative manifest;
- one entry module and a map of normalized relative `.ts` module paths to source;
- a current editable revision and immutable revision ancestry/content digests;
- a last valid revision, when one has compiled successfully;
- validation diagnostics and compiler, Macro SDK, host API, and runtime versions; and
- portable presentation metadata, including the default dark-red Macro pool treatment and
  optional item presentation.

The manifest declares:

- named entrypoints and one optional default Workflow entrypoint;
- input and result schemas;
- requested host capabilities;
- allowed Programmer access: `isolated`, `shared`, or `both`;
- pane declarations;
- requested HTTP origins;
- show-owned package storage; and
- package and host API compatibility versions.

Metadata is declarative and cannot be registered or mutated by running TypeScript. In particular,
pane types and permission requests remain inspectable when the Macro is not running.

### Modules and imports

Source may import only:

- relative modules inside the same package; and
- the generated `@tosklight/macro` SDK.

Reject bare/npm imports, URL imports, absolute paths, parent traversal, native modules, and dynamic
imports. The complete module graph is resolved and validated before a revision becomes executable.
Calling another Macro is a supervised `MacroService` operation by stable package and entrypoint
identity, not an import.

## Revisions, type checking, and executable state

Type checking happens when a package enters or changes the library, not on every start:

1. Every import validates the complete module graph locally. Imported diagnostics, emitted
   JavaScript, or claims of validity are never trusted.
2. Every save stores the editable source, runs the authoritative server type check, and updates
   diagnostics.
3. A successful save promotes that immutable revision to the package's last valid revision and
   caches emitted JavaScript by source, compiler, SDK, host API, and runtime digest.
4. A failed save keeps the source and diagnostics but retains the previous last valid revision.
5. Starting performs only a cheap executable-status, digest, and runtime-version check. It never
   invokes the TypeScript compiler.

The operator-visible states are:

- **Valid**: the current revision is the execution revision;
- **Checking**: import/save validation is still running;
- **Running previous valid revision**: current source is invalid, so new starts use the last valid
  immutable revision; and
- **Not executable**: neither the current source nor an earlier retained revision is valid.

The editor and Macro Pool must show which source revision is open and which revision will execute.
There is no silent fallback indicator and no type check on the live start path.

Existing instances keep the code revision they already loaded. A later valid or invalid save does
not hot-swap or cancel them.

Compiler caches are application-owned. Server startup and application upgrades proactively rebuild
missing or stale cache entries. Until rebuilding completes, a package is temporarily unavailable;
a live start request must not synchronously compile it.

## Library and show portability

### Local execution authority

The installation Macro library is authoritative for execution. It stores the current editable
revision, diagnostics, last valid revision, revision history needed for comparison, permissions,
and compiled cache.

Adding a local Macro to a show immediately embeds its current editable revision and last valid
revision. The show copy is a transferable package snapshot, not a live execution dependency.
ToskLight always executes the local valid revision.

When a show is opened:

- a missing local package always produces an install prompt before it can run;
- any differing local/show package produces a comparison prompt;
- the prompt shows manifest, entrypoint, permission, pane, and validation changes, added and
  removed files, and a Monaco side-by-side diff for every changed module;
- accepting replaces the complete local package atomically;
- declining retains the local package and continues to run its local valid revision; and
- no file-by-file source merge is offered.

An imported invalid current revision may retain the prior local last valid revision. A newly
imported package with invalid current source is executable only when its included last valid source
also validates locally.

### When the show copy updates

Normal active-show autosave and named revisions do not refresh an embedded Macro package after the
local library changes.

Save As and portable show export perform a Macro preflight:

- every missing or conflicting package must be resolved;
- the output receives the current local editable source, validation status, and locally validated
  last valid source; and
- compiled QuickJS or TypeScript artifacts are never trusted as portable data.

This deliberately keeps local editing independent while ensuring a deliberately transferred show
contains the latest reviewed library packages.

### Show-owned package storage

A package may request one confined storage namespace stored inside the show. It is a separate
portable object keyed by package ID and travels with Save As, export, and selective import.

The SDK exposes typed read, write, list, and delete operations with normalized relative paths,
compare-and-set revisions, atomic writes, file and package quotas, and explicit missing/conflict
errors. It never exposes an operating-system path or the general File Manager.

Accepted storage mutations use ordinary show persistence automatically. A Macro cannot force a
show save or load.

## Entrypoints and shared state

One package may declare several named entrypoints of different kinds.

### Function

A Function is a headless async operation with typed input and result. Multiple calls may run
concurrently. It cannot show dialogs or provide pane behavior.

Functions may be called from another Macro or an application integration, but never from Dynamics.

### Workflow

A Workflow is a user-facing async process. Multiple instances of the same Workflow may run
concurrently. It may:

- use the initiating user's Programmer when allowed;
- use an isolated Macro Programmer;
- show sequential dialogs;
- wait for timers, events, or operator responses;
- call other Macro entrypoints; and
- finish with a typed result or remain active until cancelled.

### Service

A Service is a long-running entrypoint that may provide behavior for declared panes. At most one
instance of a given Service may run per desk. All copies of its pane on that desk share that
instance. Different desks have independent Service instances.

### Package-shared state

All entrypoints in one package share one show-wide transient structured state store, including
entrypoints running for different desks. This supports a game-show package where one Service owns a
pane while Cue-triggered Functions or Workflows update the same game state.

The shared state API uses revisioned reads and compare-and-set updates so concurrent entrypoints
cannot silently overwrite one another. It accepts only structured Macro values, not JavaScript
functions, handles, runtime objects, or application references.

Shared state is cleared on show change, server restart, package replacement, or explicit reset. It
is not stored in the show. Durable package data belongs in the show-owned storage namespace.

## Host API and values

The generated SDK exposes capability-scoped namespaces for:

- fixtures, logical heads, fixture types, Patch, and Stage positions;
- ordered fixture selection and Groups;
- Presets and attribute values;
- Dynamics definitions and instances, except Macro-to-Dynamics callbacks;
- Cues, Cuelists, Cue recording, and playback continuation;
- Playbacks and their typed actions;
- Programmer values, history, Preload, and recording;
- show-owned layouts and Macro pane state;
- application events and monotonic timers;
- child Macro calls;
- logging and cancellation;
- declarative operator interactions;
- audited HTTP; and
- show-owned package storage.

`MacroValue` supports null, booleans, numbers, strings, arrays and records, colors, durations,
ordered fixture selections, and stable typed references to Groups, Presets, Dynamics, fixtures,
Cues, Cuelists, Playbacks, and other supported show objects.

There is no generic JSON show mutation. If an in-scope show operation does not yet have a typed
application service, implement that service first and then expose it through the SDK. The generated
TypeScript declarations and Rust/wire contracts must have a drift check.

## Programmer authority

Each entrypoint declares `isolated`, `shared`, or `both`.

### Shared

Shared access means the real Programmer of the user who initiated the execution. Changes are
visible immediately and use ordinary Programmer history and undo. The handle is unavailable when
the execution has no initiating user.

### Isolated

An isolated Macro Programmer is an execution-owned private staging authority, not a fake user or
persisted user session. Intermediate changes are invisible. The Macro may atomically publish or
replace its Macro-owned active/gold Preload scene.

### Both

`both` exposes explicit isolated and shared handles. It does not silently change a global current
Programmer. The Macro may build in isolation and deliberately transfer values into the user's
Programmer.

Scheduled Macro execution is isolated-only because a Schedule has no initiating user.

Stopping a Macro removes its isolated Programmer, Macro-owned live contribution, open dialogs, and
runtime. Already committed show mutations, package-storage writes, Playback actions, Cue changes,
or values transferred to a user Programmer remain. Cancellation is not a general rollback system.

## Dialogs and operator interaction

Macros describe dialogs using typed data rendered by ToskLight through the authoritative modal
stack. A dialog may contain:

- title, explanatory text, and named actions;
- sections and responsive rows or columns;
- the form controls supported by ToskLight;
- color, text, number, choice, and confirmation inputs;
- Group and Preset selection;
- ordered fixture selection using the Fixture Sheet; and
- client and server validation.

Macros never supply HTML, React, CSS, browser JavaScript, DOM callbacks, or component imports.

An execution may have one outstanding dialog at a time and may show another after receiving the
first result. Multiple Workflow instances may therefore produce independently stacked dialogs.
Closing a dialog returns a typed cancellation result to the Workflow; it does not necessarily
cancel the complete Macro unless the Workflow chooses that behavior.

Manual interaction targets the initiating desk. Future Schedules resolve their configured desk
aliases and may show one request on several desks. The first valid response wins, resumes the one
execution, and closes the dialog on every other targeted desk. Stale or duplicate replies are
rejected explicitly.

Interaction requests and responses carry execution ID, request ID, revision, audience, schema, and
status. They are authoritative server state, recoverable through snapshots after reconnects, not
ephemeral client notifications.

## Macro panes

Pane declarations are package metadata. Once a show contains the package, its pane types are
available even when its Service is not running.

The desktop registers one generic persisted Macro pane kind whose metadata identifies the package
and declared pane. An idle pane shows that the Service is stopped and offers **Start Macro**. A
running Service publishes a typed pane surface descriptor and receives typed action IDs and values.
The descriptor uses the same safe component vocabulary as Macro dialogs.

Stopping a Service empties its panes but does not remove their layout metadata. Server restart or
show reload leaves them idle; Services never checkpoint or automatically resume in the first
implementation.

## HTTP and other permissions

All capabilities are requested by the portable manifest and granted in local Macro settings.
Portable show content cannot grant itself authority.

HTTP is disabled by default. Local settings approve exact HTTP or HTTPS origins per package.
Private, loopback, and link-local lighting-network devices are permitted only through an explicit
origin approval. The application-owned HTTP port must:

- support ordinary methods, headers, request bodies, and response bodies;
- enforce timeout, request/response size, redirect, and rate limits;
- revalidate the destination, DNS result, and permission on every redirect;
- integrate cancellation;
- return structured errors; and
- record attempted and terminal audit events without logging sensitive values.

The first implementation has no secret store. Operators may set literal headers or bodies, but the
editor must warn that credentials written in source are visible in local/show diffs and portable
show files.

Direct sockets, arbitrary filesystem access, environment access, process execution, browser
network APIs, and database access remain unavailable.

## Execution and supervision

The existing language-neutral `MacroRuntime`, `MacroHost`, and `MacroService` seams remain the
boundary. Convert blocking invocation and waits to Tokio-aware async operations with cancellation
tokens.

Use one QuickJS runtime per execution. Package-shared state remains in the Rust application layer
so a bad execution can be disposed without corrupting other instances.

Every execution snapshot includes:

- execution, package, revision, and entrypoint identity;
- kind and initiating source context;
- desk and optional user/session context;
- arguments and terminal result;
- queued, running, waiting, cancellation-requested, completed, cancelled, failed, or interrupted
  phase;
- current wait reason;
- start time and elapsed time;
- bounded logs and resource use;
- parent/child relationships; and
- structured failure or interruption reason.

Parent cancellation cascades to awaited children. Independently started Services remain separate
executions. Runtime panics, limit violations, host failures, and rejected promises become terminal
failures instead of crashing or wedging the server.

On server restart or active-show change, cancel every execution from the old runtime, close its
dialogs, release isolated Programmer state, mark it interrupted where history survives, and leave
declared panes idle. There is no JavaScript VM checkpoint or automatic restart in the first
implementation.

Initial installation defaults are:

- at most 2 MiB of TypeScript source and 128 modules per package;
- 32 MiB QuickJS heap and 1 MiB JavaScript stack per execution;
- 100 ms maximum uninterrupted JavaScript turn;
- 30 seconds for a headless Function unless locally raised;
- unlimited Workflow/Service wall time only while they continue to yield through host waits;
- 16 MiB show-owned storage per package;
- bounded logs, child depth/count, pending waits, and host-call rates; and
- the existing Macro HTTP request, response, timeout, and redirect limits.

Installation policy may lower or deliberately raise these limits. Macro code cannot change them.

## Operator surfaces and invocation

### Macro Pool and editor

Add a dark-red Macro Pool with package name, validation/execution status, presentation, and running
state. A card starts its default Workflow entrypoint. If required arguments are missing, the
manifest's input schema opens the generated initial dialog.

The package editor provides:

- module tree and Monaco TypeScript editor;
- manifest/settings editor;
- live and authoritative server diagnostics;
- explicit checking progress;
- current and execution revision labels;
- revision history and last-valid fallback status;
- capability and HTTP-origin review; and
- local/show import comparison.

### Command line, API, and OSC

`MACRO <pool-number> [ENT]` starts the package's default Workflow through the authoritative command
line and records normal command history. No fixed software-keyboard or physical-key shortcut is
assigned initially.

OSC/custom control mappings may emit the Macro command token or start a named entrypoint directly.
HTTP, WebSocket, OSC, Cue, Playback, Timecode, and Macro-to-Macro starts reach the same
`MacroService`.

### Running & Output

Add a Macros section to **Running & Output** and include Macros in its active count. Each row shows
package and entrypoint, executing revision, source, desk, phase or wait reason, elapsed time, and
recent failure/log status, with an authoritative **Cancel** action.

Cancelling a Service returns its declared panes to idle. Cancelling a Workflow closes its dialogs.
All cleanup is server-owned and idempotent.

## Transport and event contracts

Add typed wire contracts for:

- package/library snapshots, validation, revisions, imports, and diffs;
- entrypoint definitions and schemas;
- execution start, cancel, snapshots, results, logs, and history;
- shared state;
- interaction requests and responses;
- pane surfaces and pane actions;
- show-owned storage; and
- permissions and HTTP-origin grants.

Library/source editing uses request-identified object intents. Live start, cancel, interaction, and
pane actions use ordered WebSocket frames from the desk with plain HTTP equivalents for
integrators, following `docs/engineering/api-rules.md`.

Lifecycle and interaction transitions are first-class Macro events, not generic operator
notifications. Reconnects and event gaps repair from scoped snapshots without replaying live
actions or refreshing unrelated bootstrap data.

## Trigger integration and scheduling boundary

Manual, Macro-to-Macro, Cue, Playback, Timecode, HTTP, OSC, and future Schedule starts use stable
package plus entrypoint identity and the same execution service.

The first Macro implementation defines the runtime side of scheduled execution:

- scheduled entrypoints must allow isolated Programmer access;
- the Schedule supplies source context and resolved dialog desk aliases;
- starting a disallowed duplicate Service fails that occurrence without blocking other work;
- scheduled failures and cancellation are terminal Macro outcomes returned to the Schedule; and
- multi-desk interaction uses first-valid-response-wins.

[Schedules](../Next/53-schedules.md) remains solely responsible for wall-clock triggers,
timezones, missed-run policy, occurrence identity, catch-up, clock changes, and schedule history.

## Selective import

Replace the prototype generic Macro import descriptor with the final package, revision, manifest,
storage, and stable-reference schema.

Preview must show:

- the selected package and all required show-object dependencies;
- its current and last valid revisions;
- package storage;
- references from Cues, Playbacks, Timecode, Schedules, or other Macros;
- ID conflicts and planned rewrites; and
- whether the local library comparison must be resolved.

Apply the package, storage, dependencies, and rewritten references atomically. Import never creates
an invisible live dependency on another show.

## Verification and acceptance

### Runtime and compiler

1. Import and every save perform authoritative type checking; repeated starts make zero compiler
   calls.
2. A valid save promotes the execution revision.
3. An invalid save retains editable source and diagnostics while new starts visibly use the
   previous valid revision.
4. A package with no valid revision cannot start.
5. Imports recheck current and fallback source and cannot smuggle a compiled artifact.
6. Startup or compiler/runtime upgrades rebuild stale caches before packages become available and
   never compile synchronously on start.
7. Optional annotations, type errors, module errors, SDK drift, forbidden imports, and
   QuickJS-compatible emission have focused tests.
8. Infinite loops, memory and stack exhaustion, panics, rejected promises, and cancellation during
   JavaScript, HTTP, timer, event, or interaction waits terminate safely.

### Persistence and portability

9. Existing shows with no Macros load with no behavior change.
10. Adding a local Macro embeds its current and last valid source revisions.
11. Missing and differing local/show packages always produce the required preview and diff.
12. Declining an update preserves and runs the local valid version.
13. Normal autosave and named revisions do not refresh an embedded package.
14. Save As and export embed the current local package only after conflicts are resolved.
15. Show-owned package storage survives restart, Save As, export, import, and transfer while
    remaining confined and quota-limited.
16. Failed migration or invalid Macro data preserves the original show and follows normal recovery
    behavior.

### Entrypoints, state, and Programmer

17. Functions run concurrently with typed inputs/results and cannot open dialogs or panes.
18. Multiple Workflow instances can run concurrently.
19. One Service instance per package entrypoint and desk serves every copy of its panes on that
    desk.
20. Entry points across desks share the same show-wide package state with explicit conflict
    behavior.
21. Shared state clears on restart/show change/package replacement and durable storage does not.
22. `isolated`, `shared`, and `both` enforce exact Programmer authority.
23. Scheduled execution cannot obtain a shared Programmer.
24. Isolated changes remain private until one atomic active/gold Preload commit.
25. Cancellation releases execution-owned state but preserves already committed actions and
    transfers.

### UI and control surfaces

26. Macro Pool, Monaco editing, diagnostics, revision fallback, import diffs, and capability review
    are visible in the real desktop.
27. Structured dialogs render every supported form, Group/Preset picker, and ordered Fixture Sheet
    selection through the modal stack.
28. A Workflow can show sequential dialogs.
29. The first response to a multi-desk dialog wins and closes every other copy.
30. Reconnect repairs pending interactions without duplicate responses or lost requests.
31. Declared panes remain available and show **Start Macro** while their Service is idle.
32. **Running & Output** lists and cancels Functions, Workflows, and Services with correct cleanup.
33. Macro Pool, `MACRO <number>`, HTTP, WebSocket, OSC mappings, Cue/Playback, Timecode, and
    Macro-to-Macro starts converge on the same execution identity and outcome.

### Security and performance

34. Tests prove that Macro code cannot access desk settings, show management, arbitrary files,
    processes, environment variables, raw databases/sockets, browser APIs, self-editing, or
    unapproved HTTP origins.
35. HTTP redirects, DNS changes, private-network origins, sizes, timeouts, rates, and cancellation
    cannot escape policy.
36. Storage traversal, stale writes, and quota bypass are rejected atomically.
37. Active Workflows and Services do not block server requests, OSC feedback, desktop lifecycle,
    or the output scheduler.
38. The existing 32-universe/100 Hz acceptance benchmark still passes with Macros active, and
    instrumentation proves JavaScript never enters Dynamics or DMX sampling.
39. Operator-facing behavior is verified through `npm run open`, not inferred only from unit or
    static checks.
