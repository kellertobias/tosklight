# Update, Highlight, Fixture Profiles, and Matter

These scenarios cover the new operator-facing seams added by planned features 08, 11, 18, and 01. The paired Playwright cases prove a representative authenticated API action and the independent production UI adapter from separate working shows. Exhaustive tracking, rendering, raw encoding, migration, transport, gesture, and safety permutations remain in the focused Rust and component suites named below.

## UPDATE-001 — Update existing programming

**Priority:** P0  
**Primary layer:** Paired API and operator UI, backed by server/programmer unit tests

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `update-001-<surface>.show`, and use only that copy.

**Procedure:**

1. Choose a stored, non-derived Group with an existing ordered membership and select one valid fixture not already in it.
2. In the API variant, apply Group **Add New** through `POST /api/v1/update/apply` with the exact current revision.
3. In the UI variant, press computer-keyboard Shift+End for `[SHIFT] [REC]`, confirm the desk visibly arms **UPDATE**, touch that Group, choose **Add New** in the Update preview, and confirm **Update Group**.
4. Read the stored Group and programmer after confirmation.

**Assertions:** One revision-checked mutation appends the new fixture after every existing member, removes or reorders nothing, retains the programmer selection, and reports completion through the Update workflow.

**Pass condition:** Update uses the target's normal persisted model and ordered Group Merge semantics, while the software adapter and authenticated API produce the same authoritative result.

**Required focused coverage:** `crates/server/src/update.rs` covers all four Cue modes, exact fixture/attribute eligibility, authoritative tracked-source resolution, Preset Existing/Add New, ordered Group membership, ambiguity, no-op, stale revision, cancellation boundaries, and atomic plans. Server integration tests prove a touch preview rejects a changed playback/current-Cue context, a preview rejects changed shared programmer contents, Existing Only can update several source Cues in one object write, and one Undo restores that complete write. Server command/OSC tests cover current-page versus explicit-page playback resolution, mutually exclusive short/double/long Shift+Record gestures, same-desk versus different-desk armed state, and attached-hardware playback interception without operating the playback. `UpdateWorkflow.test.tsx`, `CommandLineBar.update.test.tsx`, and pool/playback tests cover touch modal/default behavior, Update Update filters, visible context, programmer-preview fingerprints, and software/keyboard routing. The hardware-controls production build verifies its shared armed feedback presentation and OSC target surface compiles against the transport contract.

## HIGHLIGHT-001 — Highlight and Step Through

**Priority:** P0  
**Primary layer:** Paired API and operator UI, backed by server/engine/fixture/control-surface tests

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `highlight-001-<surface>.show`, and use only that copy.

**Procedure:**

1. Capture three fixtures in authoritative selection order and program a normal Position value on the first fixture.
2. In the API variant, invoke Highlight On, Next, Next, and Previous through `/api/v1/highlight/action`, programming Position on the first two working fixtures between steps.
3. In the UI variant, press **HLT**, use **Next highlighted fixture** twice and **Previous highlighted fixture** once in the always-visible control beside the command line, and program Position between steps.
4. Inspect Highlight state and every programmer projection.

**Assertions:** Highlight returns to item 1 of 3, the original ordered capture remains intact after every programming change, Previous is unavailable, Next remains available, and the working selection contains only the first fixture. Normal Position changes for the first two fixtures remain programmer data; no Highlight value appears in the programmer.

**Pass condition:** Highlight is an authoritative transient output layer and step-selection tool, not hidden programmer data or client-local state.

**Required focused coverage:** Highlight registry, server identity, and OSC tests cover initial multi-selection output, explicit capture, authoritative-order duplicate removal, one parent identity for multipatch, independent selected logical heads, unpatched and no-intensity participation, no-wrap Previous/Next, removal of invalid fixtures including last-fixture ownership release, remembered-selection survival during programming, same-user/same-desk sharing, other-user ownership, reconnect/context clearing, synchronous Blind/Preview/Preload suppression, and one authoritative 150 ms repeat guard across software and hardware. Fixture, model, editor, and engine tests cover invert-aware conventional full, RGB/RGBW/calibrated-additive white, CMY/subtractive no-filter white, named Open/White wheel selection, unmatched-wheel fallback, preservation of authored raw values, a fixture-level blue identification override in exact rendered DMX and Stage color, a no-intensity fixture's configured non-intensity look, Off revealing the programmer value in the first normal frame without mutating programmer state, static channels, and Grand Master, Group Master, Blackout, and hazardous safe-value priority. UI/hardware tests cover keyboard, software, and attached-control actions and feedback.

## HIGHLIGHT-002 — Selection surfaces, store isolation, and lifecycle

**Priority:** P0  
**Primary layer:** Paired API and production Fixtures, Stage, Groups, and Presets UI

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `highlight-002-<surface>.show`, and use only that copy.

**Procedure:**

1. Capture two fixtures from the Fixture Sheet and turn Highlight on; turn it off without clearing the programmer.
2. Select a different fixture from Stage, explicitly **CAP** that selection, and turn Highlight on again.
3. Select a stored Group, explicitly replace the capture, and prove its authoritative ordered membership becomes the remembered selection.
4. While Highlight remains live, program normal Position Pan on one member and Record an ordinary Preset.
5. Reconnect the UI/session and prove Highlight remains authoritative for the same user and desk.
6. Reopen the show and inspect Highlight plus the recorded Preset.

**Assertions:** Fixtures, Stage, and Group selection paths all feed the same ordered capture. The Preset contains the normal Position Pan value and no Highlight address or raw look. Reconnect retains live state for the same user/desk, while show activation clears active output and remembered state so saving or reopening cannot restore a diagnostic look.

**Pass condition:** Highlight is reachable from every required programming surface, never contaminates stored programming, survives only the intended reconnect boundary, and cannot resume unexpectedly after show load.

## FIXTURE-001 — Revisioned fixture profiles

**Priority:** P0  
**Primary layer:** Paired fixture-profile API and production Fixture Library UI

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `fixture-001-<surface>.show`, and use only that copy. The profile itself is stored desk-wide.

**Procedure:**

1. In the API variant, create a schema-v2 profile with a unique manufacturer/name, revision 0, one **Default** mode, one main head, one split, and geometry.
2. In the UI variant, open **Show > Enter Setup > Fixture library > Create fixture**, enter the same required identity fields in the shared profile editor, and press the title-bar **Save fixture** button.
3. Query the desk-wide profile list and immutable revision history.

**Assertions:** The server assigns revision 1, retains schema version 2 and the complete Default mode atomically, and returns exactly revision 1 from history. The show copy is not used as the library store.

**Pass condition:** Create and API import converge on one server-assigned, desk-wide, immutable profile revision suitable for portable show snapshots.

**Required focused coverage:** Fixture/server tests cover atomic revision conflicts, revision deletion, retained GDTF bytes, v1 library/show migration, failed-migration preservation, reserved built-in Generic regeneration without claiming user-authored `Generic` fixtures, and fresh startup. Fixture/model/component tests cover Generic/Modes parity, every dirty-close path, manufacturer lookup/keyboard, the server-supplied canonical attribute registry, touch and accessible reorder, heads/splits/accordion, exact u8/u16/u24/u32 slots and raw encoding, validation, channel functions/actions, opt-in presets, calibrated color/xyY/CMY/wheels/gamut, and geometry templates. Engine tests cover defaults, static, Highlight, snap, invert, LTP/function priority, virtual intensity, response curves, sequence/group/grand master application exactly once, split/unpatched output, MIB, and safety. Patch and Stage tests cover independent splits/multipatch overlap, selected-split editing through software SET, keyboard Home, and attached-hardware SET, and geometry/multi-emitter visualization.

## MATTER-001 — Desk-persistent Matter playback bridge

**Priority:** P1  
**Primary layer:** Paired configuration API and physical desk setup UI, with focused Matter transport tests

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `matter-001-<surface>.show`, and start with the desk-persistent bridge disabled.

**Procedure:**

1. In the API variant, enable `matter_enabled` through `/api/v1/configuration`, capture `/api/v1/matter/status`, and disable it again.
2. In the UI variant, open **Show > Enter Setup > Screens & playback**, find **Matter playback bridge**, enable **Enable this desk as a Matter bridge**, capture status, and disable it again.
3. Assign a one-button, faderless playback to an otherwise empty global page/playback slot and leave a neighboring slot empty. Inspect all advertised playback lights, write OnOff and Level Control through the focused Matter seam, then change or release that playback from the desk runtime.

**Assertions:** The enabled status is observed independently of the active desk page. Every assigned playback, including the faderless assignment, is advertised with a unique stable endpoint `1 + (page - 1) × 127 + (playback - 1)`, a concrete global playback number, and a Matter level from 0 through 254. The neighboring empty slot and unassigned playback-pool entries have no endpoint. OnOff/Level writes control the faderless playback's authoritative virtual master through the shared dispatcher, while its desk layout remains faderless; desk-side activation, tracking, and release are reflected bidirectionally. The final disabled setting is persisted and advertises no lights.

**Pass condition:** Matter is a desk-persistent, explicitly enabled bridge whose identity follows global page/playback addresses rather than any control desk's current page.

**Required focused coverage:** Matter adapter tests cover empty/unassigned omission, faderless exposure, every supported target family, stable topology, OnOff/Level writes, tracking-driven bidirectional updates, and the reserved level. Playback/server tests prove the Matter-only virtual master controls Master, Temp, and manual XFade runtime positions for faderless assignments through the shared dispatcher while normal UI/OSC fader input remains rejected for those layouts. Transport tests cover persisted node/fabric identity, commissionable status truth, UDP 5540 plus mDNS startup, subscriptions, endpoint topology reconciliation, failure reporting, and shutdown. `DeskSettingsModal.test.tsx` renders the physical-desk `MatterBridgeSettings` component and covers pairing code/QR/copy, zero-light and button-only states, and errors. The ignored socket smoke binds real host networking and must run serially; commissioning with an independent certified controller remains the final external interoperability check rather than being simulated by a browser.

## Commands

Run the paired scenarios with:

```sh
./test e2e tests/11-update-highlight-fixture-profiles-and-matter.spec.ts
```

Run focused implementation layers before the full suite:

```sh
cargo test -p light-server --no-default-features update::
cargo test -p light-server --no-default-features --lib highlight::
cargo test -p light-server --no-default-features --bin light-server highlight
cargo test -p light-server --no-default-features matter::
cargo test -p light-server --no-default-features tests::matter_
cargo test -p light-fixture
cargo test -p light-engine
cd apps/control-ui && npm test -- UpdateWorkflow.test.tsx HighlightControls.test.tsx FixtureProfileEditor.test.tsx DeskSettingsModal.test.tsx
```
