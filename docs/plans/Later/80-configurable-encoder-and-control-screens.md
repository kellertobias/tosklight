# Configurable Encoder and Control Screens

## Status

**Later — planning only.** This plan defines the intended screen and encoder-surface model. It
does not authorize product code, persistence migrations, generated contracts, help changes, or
executable tests yet.

This work extends the finished
[External Screen Fixed Full-Screen Pane](../refactoring/finished/25-external-screen-fixed-pane.md)
feature. It must preserve that feature's view-only behavior and existing saved desk data rather
than reopening or weakening its contract. Encoder ordering and width-dependent page packing come
from the living
[Canonical Attribute Consolidation and Encoder Layouts](79-canonical-attribute-consolidation-and-encoder-layouts.md)
plan.

## Goal

Let an operator configure a ToskLight desk for either four or six visible encoders and place the
interactive lower control surface on the main screen or on one configured secondary screen. The
secondary surface must work both as a native Tauri screen and as a normal authenticated browser
page.

The browser form is intentionally a control surface, not a reduced Desktop. It shows the actual
production lower controls—buttons, command line, keypad, encoder groups/pages, encoder values, and
the lower-section mode controls—without the upper Desktop, panes, Dock, or setup navigation.

The screen model should also allow an existing view-only fixed pane to occupy the left or right
side of a screen, in addition to the existing full-workspace presentation.

## Product decisions

1. The supported encoder counts in the first implementation are **4 encoders** and
   **6 encoders**. Five remains useful in the layout experiment and packing design, but is not a
   selectable production screen mode in this scope.
2. Encoder count is screen presentation configuration. It does not change fixture capability,
   canonical attributes, Programmer data, Cue data, or show portability.
3. The complete interactive lower control surface has exactly one configured on-screen owner per
   desk: the default/main screen or one optional/secondary screen. ToskLight does not silently show
   two writable copies merely because two windows are open.
4. Moving the surface moves its software controls as a unit. It does not split the command line,
   keypad, programmer buttons, and encoders across unrelated screens in the first implementation.
5. Native windows and browser pages render the same production control components and use the
   same server-authoritative desk session. There is no browser-only clone of encoder or command
   behavior.
6. A configured browser control surface is not the sibling `light-hardware-controls` application.
   That application remains the dedicated attached-hardware/OSC product. This plan adds a web
   rendering of the main application's lower control section using the normal application
   transport.
7. An attached physical encoder panel continues to report and use its own hardware profile. A
   screen's four/six choice controls visible software layout only unless a later, explicit hardware
   profile contract maps physical hardware to it.

## Screens & playback configuration

Extend **Show > Desk Setup > Screens & playback** with one clearly labelled
**Programmer control surface** section.

### Desk-level placement

- **Show controls on:** `Main screen` or one named optional screen.
- The selector identifies the stable screen configuration, not a physical display index or a
  transient browser connection.
- The selected screen shows the lower control surface. Every other screen omits that surface and
  gives no invisible touch, keyboard, wheel, or encoder targets in the reserved area.
- Removing the owning optional screen requires explicit reassignment to the main screen as part of
  the same confirmed action. A saved desk must never reference a deleted control owner.
- If the owning optional screen is disconnected, the main screen shows a visible
  **Programmer controls unavailable — assigned to _Screen name_** status and an explicit
  **Use controls on this screen** action. Do not silently create a temporary second owner, because
  a browser may reconnect while the operator is using it.

### Encoder count

- **Visible encoders:** `4` or `6`.
- Store the choice with the screen that owns the control surface so that moving the surface also
  selects that destination's intended physical layout.
- Default existing desks to `6`, which preserves the current six-card presentation.
- The setting changes the number of encoder positions, grid geometry, and derived pagination. It
  does not delete attributes or create separate fixture mappings.
- If support for five encoders is accepted later, it is one additional validated enum value using
  the same packing algorithm—not another hand-authored registry.

### Optional-screen content

An optional screen's base content can be:

- **Desktop** — the existing configurable pane workspace;
- **Control surface only** — the lower interactive controls with no upper section; or
- **Fixed pane only** — the existing fixed, view-only pane across the available content area.

Selecting an optional screen as the Programmer-control owner normally selects
**Control surface only**, but these are separate settings. The UI must explain and validate the
combination rather than silently replacing a deliberately configured Desktop. A screen configured
as **Control surface only** cannot enable Dock, Desktop switching, or pane-layout controls.

Existing Playbacks/Page Controls settings need a deliberate consolidation during implementation:
the control-only surface renders the production lower-section modes and must not append a second
`ScreenPlaybackSection` beneath them. Playback controls appear once, using the screen's existing
page-mode and playback-row configuration.

## Control-surface rendering contract

The control-only screen contains:

- the shared command line and its authoritative feedback;
- the same keypad and command buttons as the main lower section;
- the same Programmer task/family navigation;
- the active encoder group, derived encoder pages, labels, values, indexed choices, direct input,
  push, turn, push-turn, and release interactions supported by the main application;
- the production lower-section mode controls, including its configured Playback presentation; and
- connection, desk-lock, loading, error, and ownership feedback required to use the surface safely.

It does not contain:

- the upper workspace, Desktop panes, Desktop tabs, or Desktop layout editing;
- the main Dock, Show menu, setup panes, or pane settings;
- a fixed-pane header or hidden pane interaction target; or
- a separate local Programmer, command buffer, selection, undo history, playback state, or encoder
  page registry.

Touch, mouse, wheel, keyboard-focus, OSC, and attached-hardware paths must retain their documented
semantics. Rendering the surface in a browser must not introduce polling, per-client value
authority, automatic action retries, or a second command processor.

## Browser-accessible secondary surface

The server exposes a stable screen URL for every configured optional screen. **Screens & playback**
offers **Open in browser** and **Copy browser link** for the selected screen; the operator must not
have to discover or type an internal UUID.

The browser URL resolves the saved screen configuration on the server and renders it with the
secondary-session role. It uses the normal desk alias/session context and normal authentication or
trusted-desk access rules. The URL itself must not be a bearer secret, and an unknown, removed, or
unauthorized screen identity produces a clear non-interactive state.

Opening or closing a browser surface must not create, replace, close, or take ownership of the
authoritative desk session. Several browser tabs may observe a configured screen, but only the
screen that owns the Programmer control surface renders writable lower controls. Server events
reconcile every successful action to all connected surfaces.

The existing native optional-window route and the browser route should resolve through one
`ScreenSurface` composition path. Tauri-only display placement, window bounds, and native
fullscreen remain desktop concerns; browser control surfaces neither emulate nor claim those
features.

## Fixed pane on the left or right

Refactor screen content as a composition rather than multiplying special content modes:

- **Base content:** Desktop, control surface, or none.
- **Fixed pane:** none, full, left, or right, with the existing typed pane configuration.
- **Side width:** a bounded percentage stored with the screen; propose `40%` as the default and a
  safe adjustable range such as `25–75%`, to be validated at representative desk resolutions.

`full` preserves today's **Fixed full-screen pane** behavior and has no base content. `left` and
`right` place the same view-only fixed pane beside the chosen base content. The fixed pane remains
limited to the finished allowlist, remains configured from the controlling screen, and gains no
selection, editing, navigation, pane chrome, or settings controls.

The split is a real layout region, not an overlay. Both regions must remain usable at the minimum
supported resolution, and neither may cover the lower controls, Playbacks, Page Controls, native
drag region, connection state, or desk-lock surface. If the available size cannot satisfy both
regions' minimums, show a clear configuration warning and reject the invalid width rather than
silently hiding controls.

The Dock remains incompatible with `full`. For a side-fixed pane, Dock belongs only to a Desktop
base region and consumes that region's space; it is unavailable with a control-only or empty base.

## Encoder layout and page identity

The attribute registry owns one semantic order. The four- and six-position presentations are
derived from that order according to Plan 79:

- paired or compound mechanisms stay together;
- a pair that does not fit moves to the next page rather than splitting across pages;
- applicability filtering omits wholly empty pages without changing canonical meaning; and
- custom attributes participate through the same ordered packing contract.

Do not persist two independent copies of the attribute registry or hand-maintain separate four-
and six-encoder tables. Generated pages may be cached, but the ordered semantic model is the
authority.

Numeric page indexes cannot be the only shared navigation identity because the same logical
attribute may land on a different page at different widths. Shared desk state should identify the
active encoder group plus a stable logical page/attribute anchor; each surface derives the page
that contains that anchor for its configured width. This also prevents an attached six-encoder
surface and a visible four-encoder surface from appearing to select unrelated content.

Application-specific decks such as Dynamics and future Timecode controls use the same width
contract. They may define semantic control blocks of their own, but cannot assume six DOM slots or
silently drop controls in four-encoder mode.

## Persistence and compatibility

This is desk-local configuration, not portable show content. The implementation must preserve
screen IDs, client registrations, OSC aliases, physical-display selection, bounds, fullscreen,
playback layouts, page modes, and existing Desktop layouts.

Introduce a typed screen-composition representation and migrate current desk data as follows:

- existing Desktop content becomes `base = desktop`, `fixed = none`;
- existing Fixed full-screen pane content becomes `base = none`, `fixed = full` with the exact
  current pane settings;
- existing desks gain `visible_encoders = 6`; and
- existing desks assign the Programmer control surface to the main/default screen.

Loading malformed or legacy data must normalize to one valid owner and one supported encoder count
without modifying portable shows. Update routes remain typed, sparse, replay-safe object-intent
operations. Live encoder, command, Programmer, and Playback actions continue over the established
authoritative event path and are never automatically retried.

## Implementation work

1. Finalize the Plan 79 semantic packing contract for four and six positions, including compound
   encoders and application-specific decks.
2. Add desk-local screen composition, control-owner, and encoder-count types; implement legacy
   migration and malformed-data normalization before changing the UI.
3. Rework `ScreenSurface` into shared base-content, fixed-pane, and lower-control regions while
   preserving the finished fixed-pane contract.
4. Make the production lower control components width-aware and render them from the selected
   native or browser secondary surface without duplicating application state.
5. Add the Screens & playback controls, compatibility validation, disconnected-owner recovery,
   and browser-link actions.
6. Add the authenticated browser surface and keep native screen-window responsibilities isolated
   behind the desktop bridge.
7. Update operator help and focused human-readable acceptance scenarios, then add executable
   coverage for the exact physical interaction paths.

## Acceptance criteria

1. A new or migrated desk starts with six visible encoders and the Programmer control surface on
   the main screen.
2. Screens & playback offers exactly four and six as production encoder-count choices.
3. Four-encoder mode renders exactly four usable positions and deterministically repaginates every
   ordinary and application-specific encoder group without losing an applicable control.
4. Six-encoder mode preserves the current six-position behavior unless Plan 79 deliberately
   changes a canonical grouping.
5. The operator can move the complete lower control surface from the main screen to one named
   optional screen and back, with exactly one configured writable on-screen owner.
6. A control-only secondary screen shows the real buttons, command line, keypad, encoder controls,
   and lower-section modes, but no Desktop, upper panes, Dock, setup navigation, or duplicate
   Playback section.
7. The same configured control-only screen works in a native Tauri window and an authenticated
   browser, with actions and feedback reconciled through the same authoritative desk state.
8. Browser URL discovery is available from Screens & playback; removed, unknown, or unauthorized
   screens do not expose an interactive surface.
9. Closing or reconnecting a browser/native secondary never resets the command line, Programmer,
   selection, encoder task, Playback state, or desk session.
10. A disconnected control owner is clearly reported on the main screen and can be explicitly
    reassigned without silent dual ownership.
11. Existing fixed full-screen pane configurations migrate without visual or behavioral change.
12. An allowed fixed pane can be placed full, left, or right. Left/right placement uses a bounded
    saved width, remains view-only, and never overlays or hides the interactive regions.
13. Dock, Playbacks, Page Controls, native fullscreen, and fixed-pane constraints remain valid for
    every supported composition.
14. Existing desk data migrates without changing portable shows, stable screen identities, OSC
    aliases, playback layouts, or physical-display assignments.
15. Attached hardware and OSC retain parity with software controls; changing visible encoder count
    does not falsely reclassify attached physical hardware.

## Planned verification

- Rust migration and normalization tests for legacy Desktop/fixed-pane content, one-owner
  enforcement, deleted-screen reassignment, and invalid encoder counts.
- Generated wire-contract verification and focused API tests for typed sparse screen updates and
  session/desk scoping.
- Component tests for Screens & playback labels, available choices, incompatible combinations,
  browser-link state, and disconnected-owner recovery.
- `ScreenApp` composition tests for Desktop, control-only, fixed-full, fixed-left, and fixed-right,
  including Dock/Playback/Page Controls geometry.
- Encoder-surface tests that assert exact four/six slot counts, stable logical navigation,
  pair/compound packing, applicability filtering, custom attributes, and Dynamics pages.
- Focused root Playwright coverage that configures a secondary control surface, opens its real
  browser route, operates keypad/buttons and touch/turn/set-value encoder paths, and verifies the
  shared command line and Programmer result on the main screen.
- Focused native-window coverage for the same saved screen plus representative left/right fixed
  pane layouts at minimum and normal screen sizes.
- The authoritative `npm run open` desktop path, readiness, runtime log inspection, and a real
  operator pass moving controls main → secondary → main.

## Out of scope

- Selecting five encoders in production.
- Defining new canonical fixture attributes beyond Plan 79.
- Replacing the dedicated Hardware Controls application or changing its OSC protocol.
- Allowing different browser tabs to own independent Programmers within one desk session.
- Making the finished fixed-pane allowlist interactive.
- Browser control of native monitor placement, window bounds, or operating-system fullscreen.
