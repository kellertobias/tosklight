# Dedicated Virtual Playbacks and Exclusion Zones

## Queue position and status

**Doing, claimed after the completed focused Dynamics lane-layout regression and before
repository-wide dead-code removal.**


Execute this plan after the focused
[Dynamics lane-layout regression](../finished/14a-dynamics-lane-layout-and-interaction-regression.md).
This file was moved to `doing/` before changing implementation code; follow the state and
verification workflow in [`../README.md`](../README.md).

This document is the single implementation contract for the dedicated Virtual
Playback refactor and its exclusion zones. The current page-slot implementation
remains operator truth until this chunk is complete. Documentation changes alone do
not implement this plan.

## Goal

Replace Virtual Playback cells that mirror low-number physical playback positions with
real, independently addressed Virtual Playbacks. Preserve normal Playback runtime,
output, ownership, feedback, configuration, Preload, and cross-surface behavior.
Rebuild exclusion zones on those new identities so zone arbitration remains
authoritative rather than becoming a browser-local visual effect.

## Compatibility boundary

ToskLight is pre-alpha. This refactor does **not** support loading or migrating show
files, pane data, assignments, or exclusion-zone snapshots written by the old
page-slot Virtual Playback model.

- Make the schema/version boundary explicit and reject incompatible old data with a
  visible development-facing error. Do not guess or partially translate it.
- No legacy dual-read, compatibility route, shadow write, or old-address alias remains
  after the refactor.
- Regenerate the repository's three canonical SQLite shows through the real server
  save path:
  - `assets/demo.show`
  - `tests/fixtures/compact-rig.show`
  - `tests/fixtures/default-stage.show`
- Update every test fixture, deterministic Storybook provider, screenshot seed, and
  demo setup that embeds old Virtual Playback addresses.
- Prove that each regenerated show opens through normal startup, produces its expected
  assignments and output, and contains no old Virtual Playback representation.

This exception is deliberately narrower than general persisted-show compatibility
policy: it applies only to this pre-alpha Virtual Playback schema break and the
repository-owned example shows named above.

## Address space and identity

- A concrete Playback address remains **page plus playback number**, but every Virtual
  Playback number belongs to exactly one page and therefore remains globally
  unambiguous across desks.
- Physical playbacks retain numbers **1 through 1000** on each page.
- Every Virtual Playback page owns one bank of 300 numbers. Page 1 owns **1001 through
  1300**, page 2 owns **1301 through 1600**, page 3 owns **1601 through 1900**, and page
  `p` starts at `1001 + 300 × (p - 1)`.
- The existing 127-page boundary therefore makes **39100** the highest Virtual
  Playback number. A `{page, number}` pair is valid only when the number belongs to
  that page's bank.
- A page contains at most 300 independently assignable Virtual Playbacks. Definitions
  remain sparse; the implementation must not allocate all 38,100 possible Virtual
  Playback definitions eagerly.
- Page 2 Virtual Playback 1301 is independent from page 1 Virtual Playback 1001 and
  from physical page 2 playback 1. Every desk that addresses page 2 Virtual Playback
  1301 reaches the same show-owned assignment and runtime.
- Assigning, configuring, clearing, operating, recording to, or deleting a Virtual
  Playback never creates, moves, retargets, or clears a physical playback.
- A Virtual Playback references the normal Cuelist, Group, Dynamic, master, or other
  supported Playback target. It does not copy the source object or create a separate
  runtime engine.
- Operator labels, selection, audit, running-source feedback, and error messages use
  the stable number, optionally accompanied by its derived page, such as **Virtual
  Playback 1901** or **Virtual page 4 · Playback 1901**.

Update all numeric types, validators, generated wire schemas, persistence indexes, and
test helpers that currently assume a maximum playback number of 127 or an eight-bit
slot. Range errors must name the accepted physical or virtual range.

## Pane grid and page modes

- A Virtual Playbacks pane has explicit **Rows** and **Columns**. Both are positive
  integers and their product may not exceed 300.
- A 20×15 grid is the ordinary full-page acceptance case. Smaller sparse grids with
  empty cells are supported.
- Cell position `n` on page `p` maps in row-major order to Virtual Playback number
  `1000 + 300 × (p - 1) + n`. Empty positions remain visible and assignable.
- Resizing the pane never changes its logical rows, columns, addresses, or assignments.
  The pane fits, scales, scrolls, and virtualizes rendering as needed.
- A pane mounts only its configured cells and subscribes narrowly to their 300-number
  bank. The server remains authoritative for every page and definition.
- Each pane has one page mode:
  - **Follow Main** resolves against the main page of its control desk.
  - **Pinned** resolves against one explicit fixed page stored with the pane.
- A Follow Main pane changes its displayed Virtual Playback addresses when the desk's
  main page changes. A Pinned pane does not.
- Page changes alter only the displayed address range. They never start, stop,
  normalize, or otherwise operate a Playback.
- Multiple panes and different desks may display the same page and Virtual Playback
  numbers. They are projections of the same show-owned definitions, runtime, and
  exclusion zones, not copies.

## Assignment, configuration, and runtime

- Reuse the standard Playback Configuration and Set/Record/Update workflows. A Virtual
  Playback remains a one-button, faderless target unless a later explicit plan adds
  another topology.
- Right-clicking a Virtual Playback and pressing `[SET]` followed by that Virtual
  Playback open the same standard Playback Configuration modal used by physical
  Playbacks. The pane does not expose separate **Set Source** or **Add Target**
  shortcuts and does not maintain a second source-copy assignment workflow.
- Preserve the configured action, icon, image background, color, name, current Cue,
  loaded/paused/running state, Update/Record target state, and error feedback already
  shown by the pool-grid card.
- `On`, `Off`, `GO`, back, load, stop, release, fade, restart, Toggle, Flash, Temp,
  Swap, and target-specific actions use the same authoritative Playback service and
  ownership semantics as the equivalent physical Playback.
- Preload retains a separate Virtual Playback capture domain. A captured action stores
  the stable Virtual Playback number and its validated page qualifier, commits in
  preserved order at Preload GO, and always retains a deliberate Off/release path.
- Active Virtual Playbacks appear in running-source feedback and remain operable when
  their pane is closed, another page is visible, or their cell is outside the rendered
  viewport.
- Dynamic Playback assignments implemented by chunk 14 use the same dedicated Virtual
  Playback identity and transport after this refactor; do not create a second
  Dynamic-specific virtual address space.

## Exclusion-zone data model

An exclusion zone is a show-owned named, ordered set of Virtual Playback numbers. When
a member receives an authoritative activation that leaves it On, that member wins and
the other applicable members are released through the normal Playback service.

- Each zone stores a stable ID, a trimmed non-empty name of at most 80 characters, and
  at least two unique Virtual Playback numbers.
- A zone stores **Virtual Playback numbers**, never copied target-object IDs, pane-cell
  positions, desk IDs, surface IDs, or page modes. UI selection resolves each displayed
  cell to its stable number before saving the zone.
- An unassigned Virtual Playback number may be a member. Assigning it later activates
  the existing zone intent.
- Changing, shrinking, moving, duplicating, or deleting a pane never retargets, copies,
  or deletes zone membership.
- A Virtual Playback number may belong to multiple zones. Activating it releases the
  deduplicated union of all other members in every applicable zone.
- Every desk, pane, OSC controller, REST caller, WebSocket caller, attached-hardware
  input, and Preload action uses the same show-owned zones. Desk layout and selected
  page do not partition or alter arbitration.
- Creating, renaming, editing, reordering, or deleting a zone is configuration-only
  and never operates a Playback. If several members are already On, they remain On
  until the next qualifying activation.
- All winner and peer-release mutations pass through one serialized server action.
  The last accepted activation wins, including concurrent UI, command, REST, WebSocket,
  OSC, attached-hardware, and Preload inputs.
- Restart restores activation provenance before output resumes and deterministically
  normalizes the show-owned zones using the last accepted activation. Exact legacy-zone
  migration is not required; only state written by the new schema participates.
- Automatic full-override release and the separate fader-zero/Flash-release auto-off
  options remain independent from mutual exclusion.

## Exclusion-zone operator workflow and visuals

- Shift plus pointer/touch toggles temporary cell selection without operating the
  Playback. Software keyboard Shift and attached-hardware Shift produce the same
  gesture while the pane owns interaction.
- Selecting at least two cells exposes **Create Exclusion Zone** and **Cancel Zone
  Selection** in the pane's authoritative window title. Creation asks for the name and
  remains inert.
- Pane Settings keeps Rows, Columns, Page mode, Follow Main/Pinned, and the position
  count in **Virtual Playbacks**. It keeps zone management in a separate **Exclusion
  Zones** tab and does not expose pane-level Cuelist color-mode controls.
- **Edit Zone** closes Pane Settings and selects the zone's stored cells on the live
  grid. The title actions become **Update Exclusion Zone** and **Cancel Edit**.
  The pane derives visible cells from the zone's stored playback numbers. Updating
  replaces that zone's membership in place while preserving its ID, name, order, and
  members on other pages; canceling persists nothing.
- Temporary selection and saved zone membership are unmistakably different visual
  states. Saved membership is not communicated by color alone.
- Orthogonally neighboring members of one zone share a single outer fence with no
  internal fence edge. Disconnected components render as separately fenced islands.
  Diagonal contact alone does not join two components.
- Overlapping Virtual Playback numbers expose every applicable zone name through
  reachable detail and Settings, not only an opaque color.
- Zone styling coexists with configured light/dark Playback colors, active/running
  state, Record, Update, focus, disabled/error state, empty cells, and held actions.
  It cannot obscure the address, label, action, current Cue, or On/Off distinction.
- Loading, saving, reconnecting, conflicts, and failures have visible progress and
  actionable errors. Reconnect replaces speculative state with the authoritative
  surface snapshot.
- The complete workflow is keyboard reachable and uses row-major focus order,
  meaningful zone/member announcements, and appropriate touch targets.

The former standalone visual-review plan is incorporated here. Visual acceptance
occurs against the production implementation in representative software-only and
hardware-connected layouts before screenshots are refreshed.

## Persistence, API, and events

- Virtual Playback assignments and exclusion zones are portable show data under the
  normal active-show persistence owner and are shared by every desk operating that
  show.
- Pane rows, columns, page mode, pinned page, and stable surface ID remain
  Desktop/control-desk layout data.
- Exclusion zones have one show-owned revisioned snapshot. They contain playback
  numbers and have no desk or surface partition.
- Moving, duplicating, or removing a pane and removing a historical desk do not mutate
  zones. Only the explicit zone-delete intent removes a zone.
- Follow `docs/engineering/api-rules.md`: show and desk are authenticated context, not
  duplicated mutable operands.
- Reads use typed snapshots; configuration writes use typed, retry-safe intent actions
  with request IDs and revisions; live Playback operations use the shared ordered
  action boundary.
- Accepted configuration and runtime changes publish typed capability events.
  High-frequency telemetry remains bounded and is not persisted as one durable event
  per frame.
- Audit entries distinguish zone configuration changes, winner activation, peer
  releases, source desk/surface provenance, stable playback number, and rejected
  conflicts without copying sensitive request bodies.

## Implementation boundaries

- Domain identity, validation, Playback runtime, exclusion arbitration, persistence
  commands, and migration rejection belong in Rust.
- `crates/shared/show` remains the only portable show codec/persistence owner.
- `crates/light/adapters/headless` owns HTTP, WebSocket, OSC, session/desk context,
  server orchestration, and event projection.
- `apps/light-desktop` owns pane composition, page-mode controls, selection gestures,
  Settings, and accessible visual state; it does not decide runtime exclusion.
- Shared UI packages may render typed grid/card state and emit callbacks, but cannot
  own server state, address resolution, persistence, or action semantics.
- Keep current physical/current-page and explicit-page Playback paths working. Add
  explicit virtual address types instead of inferring them from a numeric accident.

## Consolidated documentation ownership

This plan replaces the future implementation discussion formerly held in
`docs/plans/Next/59-virtual-playbacks.md` and incorporates
`docs/plans/Manual Work/38-virtual-playback-exclusion-zone-visuals.md`.

The following remain separate by design:

- operator Help and `docs/plans/Done` describe the current implementation until this
  chunk ships, then must be updated in the completion pass;
- `.tour/glossary/domain.md` and `.tour/tours/26-playback-runtime.md` remain current
  architecture guidance and must be updated when the dedicated identity ships;
- Preload's broader LTP/manual review retains non-Virtual Playback responsibilities but
  delegates dedicated Virtual Playback identity and release acceptance to this plan;
- playback auto-off, Timecode, external-screen allowlists, Dynamics, and other feature
  plans may require parity on Virtual Playback surfaces but do not redefine Virtual
  Playback identity or exclusion zones; and
- completed Storybook/refactoring plans remain historical evidence and are not rewritten
  as if they had implemented this future address model.

Do not create another Virtual Playback identity or exclusion-zone implementation plan.
Add newly discovered requirements to this file while it is pending or doing.

### Completion-pass documentation and visual checklist

Do not apply this checklist early: each item replaces a truthful description or
characterization of the current page-slot implementation only after the dedicated
identity, page modes, and sparse-grid runtime are authoritative.

- Update `docs/help/05-Pane-Reference/02-cues-and-playbacks.md` and
  `docs/help/40-Running-a-Show/05-virtual-playbacks.md` with the 300-number page banks,
  1001/1301/1601 starts, 20×15 and 300-cell limit, Follow Main and Pinned behavior,
  stable-number labels, and server-owned cross-desk exclusion arbitration.
- Update `docs/help/50-Protocols/01-osc-rest-and-websocket.md` so protocol examples
  distinguish dedicated Virtual Playback addresses from physical current-page and
  explicit-page Playback controls; remove any compatibility wording that would imply
  an old page-slot alias remains.
- Update `.tour/glossary/domain.md` with dedicated Virtual Playback numbers, page-bank
  mapping, displayed cells, and show-owned exclusion-zone definitions. Update
  `.tour/tours/26-playback-runtime.md` with the shared Playback runtime path,
  physical-versus-virtual address domains, cross-desk identity, Preload capture, and
  serialized peer release.
- Replace the 1–12 Storybook row/column controls, page-slot fixture data, and
  `cell 128 unavailable` characterization in
  `apps/light-desktop/src/components/control/virtualPlayback/VirtualPlaybackGrid.stories.tsx`
  and `apps/ui-library/storybook/tests/ui-stories.spec.ts`. Add representative 20×15,
  sparse, Follow Main, Pinned, overlapping-zone, cross-page membership,
  configured-color, active, error, and accessibility states using stable playback
  numbers.
- Refresh `panes/virtual-playbacks.png` from the production-backed documentation
  story and `panes/virtual-playbacks-settings.png` through the live desktop path in
  `docs/help/screenshot-manifest.json`. Review both visually in software-only and
  hardware-connected layouts before accepting them.
- Refresh `docs/help/99-Development/02-help-coverage.md`,
  `docs/help/99-Development/02-test-bench-coverage.md`, and the generated semantic
  coverage catalog only after executable `VPB-007`, Preload, auto-off, and
  cross-surface coverage proves the same shipped contract.

## Verification and completion

Start with focused domain, wire, adapter, UI, and semantic checks, then run every
affected broader gate. Completion requires at least:

1. Rust domain tests for the 1/1000 physical boundary, page-bank starts
   1001/1301/1601, final 39100 boundary, page/number validation, action semantics,
   show-wide exclusion union, serialization, restart, and sparse allocation.
2. Persistence/startup tests proving old schema rejection and successful real-path
   opening of all three regenerated canonical shows.
3. Generated wire/client checks for full-width playback numbers and explicit physical
   versus virtual address types.
4. UI tests for 20×15 and smaller sparse grids, fixed logical geometry, Follow Main,
   Pinned, bank changes, multiple panes and desks, assignment/configuration, hidden
   active sources, exclusion editing/deletion, overlap, accessibility, and errors.
5. Cross-surface tests for software, software/hardware Shift zone selection, REST,
   WebSocket, OSC, attached-hardware, Preload, runtime/output feedback, and two desks
   sharing assignments, runtime, and zones despite different layouts. Virtual
   Playbacks do not consume the physical F1–F8 Playback shortcuts.
6. Updated semantic `VPB-007` coverage for the new identity and exclusion contract.
   Delete obsolete old-model assertions instead of retaining a compatibility suite.
7. Focused Preload and auto-off scenarios proving normal release/ownership behavior on
   dedicated Virtual Playback addresses.
8. `npm run test:unit`, the focused root Playwright specs, `npm run test:e2e-api`,
   `npm run test:e2e-ui`, and then `npm run test:e2e`.
9. `npm run open`, readiness verification, real software-only and hardware-connected
   visual review, and inspection of `.artifacts/runtime/light-data/light-headless.log`.
10. Help/manual updates, deterministic screenshots, `npm run manual`, the refreshed
    coverage catalog, a `## Result`, move to `finished/`, and a semantic commit.

Do not mark this chunk complete from plans, generated types, static checks, component
stories, or mocked runtime evidence alone.

## Progress checkpoint — 2026-07-28

This plan remains **doing**. The latest large-window usage checkpoint was 77%, and the
strict completion threshold is above 75%, so no later queue phase has been claimed.

Implemented and focused-verified before the corrected global-number contract:

- dedicated physical and page-qualified Virtual Playback identities, topology, wire
  projections, regenerated canonical shows, sparse grids, Follow Main/Pinned modes,
  Preload capture, automatic release paths, revisioned desk-partitioned zones, restart
  normalization, and dedicated OSC routing;
- the large-grid scroll/click retention regression and undersized-grid cell overlap;
- strict preload projection decoding for Virtual identities and fader pickup state;
- exact Virtual OSC exclusion audit provenance; and
- named-zone creation after topology authority settles;
- strict `virtual_playbacks` data in the live documentation seed and the remaining
  focused/shared Playback-page test fixtures reached by the full UI gate; and
- current production Storybook states for 20×20, sparse-large, Pinned, overlapping,
  hidden-member, and error cases.

The 2026-07-29 correction supersedes the page-reused 1001–9998 identity,
8,998-cell panes, cell-position zone storage, and desk/surface partitions listed
above. Those implementation and verification claims must be replaced with 300-number
page banks, show-owned playback-number zones, cross-desk proof, and explicit deletion
coverage before completion.

Current verification evidence:

- `cargo check -p light-headless-runtime --lib` passed;
- `cargo build -p light-headless --no-default-features` passed;
- focused Rust identity, engine boundary, topology, preload, OSC, UI component,
  canonical-show, `PRELOAD-004`, and `VPB-007` checks passed;
- direct full API Playwright coverage passed, 24/24; and
- focused named-zone `VPB-007 @bench @ui` passed;
- the focused component suites passed, 56/56, and the Storybook contract passed;
- `npm run storybook:build`, the focused help screenshot gate, the live desktop help
  screenshot gate, and `npm run manual` passed; the Virtual Playback pane and its
  in-pane exclusion-zone settings screenshots were visually reviewed;
- the semantic catalog was regenerated and `npm run test:semantic-test-docs` passed,
  8/8; and
- after the strict-schema fixture corrections, the owned focused Playback subset
  passed 6/10. The remaining four failures are the existing physical SET interaction
  regression: the card receives focus but the Playback Configuration dialog does not
  open.

Still required before completion:

- make the required full UI and complete E2E gates green. The latest full UI run was
  117 passed, 1 skipped, and 37 failed. Dedicated Virtual Playback layout, zone,
  Preload, and named-zone cases passed; twelve old Playback-page fixture failures were
  corrected afterward. The remaining failures span fixture-schema, stale dock/control
  selectors, telemetry, physical Playback Configuration, frontend warmup, and Stage
  performance;
- make `npm run test:unit` green. Semantic documentation passed, but the repository
  architecture gate currently reports direct Dynamics wire-DTO imports, duplicate
  package-owned CSS rules, and a stale test-bench migration inventory;
- rerun the broad gates after those failures are resolved, then run the packaged
  desktop/readiness/log and both software-only and hardware-connected layout reviews;
  and
- only then add `## Result`, move this file to `finished/`, and create the plan's
  semantic commit.
