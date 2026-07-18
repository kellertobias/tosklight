# Completion Coverage and Release Verification

## Status and scope

**Completed 2026-07-17.** This verification feature closes the remaining executable-coverage and release-evidence gaps found while re-auditing completed Features 01, 04, 08, 11, 18, and 20. The underlying operator behavior was already implemented except for the focused fixture physical-metadata and GLB-preview fields required by `FIXTURE-002`; those production paths were added with the acceptance coverage. The work adds missing proof, strengthens existing acceptance cases, and corrects completion bookkeeping where prior evidence was incomplete or stale.

The final checkout passed the hermetic unit and focused browser gates, the real desktop smoke path,
the hardware-controls production build and browser harness, strict formatting and Clippy, the manual
build, and the opt-in host-network Matter transport smoke. The authoritative `./build open` runtime
also passed readiness, health, and bootstrap checks. Independent-controller interoperability was not
performed, and CSA production credentials/certification are not available or claimed; those remain
external production release gates under `RELEASE-023`.

Completion bookkeeping found 90 tracked Markdown files before this plan move, 17 completed plan
files, and 11 completed-plan references targeting eight unique files, all resolving. Moving this
file raises the completed-plan count to 18 without changing the total tracked Markdown count. The
Feature 20 completion file was added in commit `5d5bb9a`; its parent contains no tracked old-path
counterpart, so the repository history does not contain the deletion half requested by the original
checklist and no synthetic deletion was created.

This feature must not redesign or reposition the software Programmer keypad. The current software layout is an observed contract for these tests. In particular, do not move the HIGH/PREV/NEXT/ALL or GRP/CUE/TIME/DIV rows, do not change key dimensions or gaps, and do not alter the current two-column-by-two-row Programmer Fade while adding coverage. Hardware-simulator tests must verify the restored RECORD/PRELOAD GO and Programmer Fade/Cue Fade geometry without changing that geometry.

No production behavior should change merely to make a test pass. Prefer the real server, production React components, production CSS, the shared OSC dispatcher, real persisted databases, and packaged applications. Add a narrowly scoped deterministic test seam only where an external controller, operating-system service, or Tauri event cannot otherwise be driven in automation.

## Test organization

Extend the existing scenarios rather than creating parallel mock-only contracts:

- `SHOW-005` remains the named-revision acceptance scenario.
- `UPDATE-001` remains the Update acceptance scenario.
- `HIGHLIGHT-001` through `HIGHLIGHT-003` remain the Highlight scenarios.
- `FIXTURE-001` remains the fixture-profile creation and migration scenario; add `FIXTURE-002` for the asset and focused physical-metadata workflow.
- `MATTER-001` remains the bridge behavior scenario; add `MATTER-002` for desk persistence across shows and restart.
- Add a `RELEASE-023` verification record for non-automatable Matter certification evidence and final completion bookkeeping.

Paired API/UI scenarios must continue to start from independent working shows. Process and migration cases must use isolated temporary desk-data directories and real graceful or abrupt restarts. Tests must never modify the developer's active `light-data` directory.

## MATTER-002 — Desk persistence across shows and restart

### Missing proof

The bridge setting is stored in desk data, but current acceptance toggles it around one show and disables it before the test ends. No executable case proves that enabled Matter state survives a show change and server restart.

### Implementation needed

Add a process-backed test to `tests/11-update-highlight-fixture-profiles-and-matter.spec.ts`, or a supplemental restart case in `tests/05-virtual-time-persistence-and-recovery.spec.ts`, using the existing isolated bench. Do not add show-level Matter fields.

### Test case

1. Start an isolated server with Matter disabled and create two independent working shows, A and B.
2. Open show A and enable `matter_enabled` through the authenticated desk configuration endpoint.
3. Assign at least one playback and record its stable global Matter endpoint.
4. Open show B. Verify Matter remains enabled and the endpoint list reflects show B's assignments rather than retaining show A's runtime objects.
5. Return to show A and verify its assignment returns with the same global endpoint identity.
6. Gracefully stop and restart the real server using the same desk-data directory.
7. Log in again, verify `matter_enabled` is still true, and verify the active show's assigned playbacks are advertised.
8. Disable Matter, restart again, and verify the disabled state persists and advertises no lights.

### Pass condition

Matter enablement follows desk data across show switches and restart, while advertised playback membership follows the active show's assignments. No portable show object contains the desk setting.

## SHOW-005 — Complete named-revision-copy lifecycle

### Missing proof

The implementation supports the complete lifecycle, but current tests do not explicitly prove every visible and destructive boundary: the copy-created timestamp, an explicit Latest Autosave load after newer work, switching away and reopening the same copy, Save As to a new name, alternate-destination overwrite through the production UI, and the contents of the recovery backup.

### Implementation needed

Extend the paired `SHOW-005` state and assertions in `tests/05-virtual-time-persistence-and-recovery.spec.ts`. Add focused `QuickSetupModal` coverage only for branches that cannot be asserted efficiently in the paired browser path. Use the real show stores for backup validation.

### Test case A — Latest Autosave and visible provenance

1. Create original show A and store state `revision-state`.
2. Create named revision 1, then change A to `newer-autosave-state`.
3. Use **Load Latest Autosave** for A and verify `newer-autosave-state`, not revision 1.
4. Use **Load Revision as Copy** for revision 1.
5. Verify the dock and Show menu identify a revision copy and show the original name, revision number, revision name, and copy-created timestamp.
6. Compare the displayed timestamp with the server's provenance timestamp using the same documented display precision and timezone rules.

### Test case B — Durable independent copy and Save As

1. Modify the copy, create a named revision belonging only to the copy, and autosave it.
2. Switch to another show, then reopen the copy through **Load Latest Autosave**.
3. Restart the server and reopen the copy again.
4. Verify the copy keeps the same show identity, provenance, Latest Autosave, and its own revision list; verify the original remains on `newer-autosave-state` with only its original revisions.
5. From the revision copy, use **Save As** with a brand-new name.
6. Verify the new show contains the copy's current state and provenance policy, while the source copy and original both remain available and unchanged.

The test must explicitly document whether Save As preserves or clears revision-copy provenance in the newly named show. Assert the chosen product rule rather than accepting either behavior.

### Test case C — Alternate destination and recoverable overwrite

1. Create destination show B with a distinct show ID, named revisions, and unique object content `destination-before-overwrite`.
2. From the revision copy, choose B as the Save As destination.
3. Verify the first selection does not overwrite B and that a separate alert names B and explains Latest Autosave replacement.
4. Cancel and prove B is unchanged.
5. Repeat and confirm the destructive action.
6. Verify B keeps its show ID, name, and every named revision, while its Latest Autosave now contains the copy's state.
7. Locate the internal pre-overwrite recovery backup, open it as a real show database, and verify it contains B's `destination-before-overwrite` content, ID, and revision data required for recovery.
8. Verify the source named revision remains immutable and the separate revision copy still exists.

### Pass condition

Every visible provenance field and every independent-copy, switch, restart, Save As, overwrite-confirmation, revision-preservation, and recovery-backup claim in Feature 04 is proven through authoritative state rather than file-existence checks alone.

## UPDATE-002 — Legacy show and attached-hardware gesture coverage

### Missing proof

Old Update settings receive defaults, but no focused test opens a legacy show and completes an Update. The server tests OSC gesture timing, but the simulator's actual pointer path is not exercised from Shift and Record through the emitted control messages.

### Test case A — Legacy show migration followed by Update

1. Create or load a fixture representing the last supported show schema before Update settings and Update-specific stored metadata existed.
2. Open it through the real server startup or show-open path.
3. Select fixtures, create programmer values, and target an existing Cue, Preset, and ordered Group in separate subcases.
4. Enter Update using `[SHIFT] [REC]`, accept the documented migrated defaults, and complete the operation.
5. Verify exact per-fixture/per-attribute results, revision increments, retained programmer values, one-step undo, and no unrelated object mutations.
6. Close and reopen the migrated show and repeat one Update to prove the migration is persisted and idempotent.

### Test case B — Hardware simulator gesture exclusivity

1. Render or launch the production hardware simulator with Tauri `invoke` captured at the transport boundary.
2. Exercise a short Shift+Record gesture through actual pointer down/up events and verify exactly one normal Update action.
3. Keep Shift held and double-press Record; verify exactly one Update Update action and no normal Update action.
4. Exercise the documented long press and verify exactly one Update Settings action.
5. Repeat boundary timings around the short/double/long thresholds with fake time or a deterministic clock.
6. Verify releases are sent, no gesture dispatches twice, and the resulting action names and OSC paths match the production server contract.

### Pass condition

A supported legacy show can perform Update after migration, and the actual attached-hardware pointer path proves mutually exclusive single, double, and long Shift+Record gestures.

## HIGHLIGHT-004 — Multi-user ownership conflict

### Missing proof

Different-user same-desk ownership exclusion is implemented, but no server test directly exercises `OwnedByAnotherUser` and its release lifecycle.

### Test case

1. Create users A and B with independent authenticated sessions on the same desk alias.
2. Give user A a non-empty selection and turn HIGH on.
3. Give user B a different selection and attempt HIGH On or Toggle.
4. Verify the server returns the documented conflict, names or identifies the owner safely, leaves user A's Highlight output unchanged, and does not activate user B.
5. Verify PREV/NEXT/ALL remain scoped to each user's actual selection and do not steal Highlight output ownership.
6. Close one of multiple sessions belonging to user A and verify ownership remains while another A session exists.
7. Close the last user A session or turn HIGH off, then verify user B can acquire ownership immediately.
8. Repeat with the same user on another desk alias and verify different desks remain isolated.

### Pass condition

Ownership conflict, non-mutation on failure, same-user session retention, final release, reacquisition, and different-desk isolation are all asserted in the real server registry and HTTP/feedback layers.

## HIGHLIGHT-005 — Production alert reachability and invariant geometry

### Missing proof

Current component tests prove that the alert uses a body-level portal and can be dismissed, but they do not prove production browser bounds, stacking, supported viewport reachability, or unchanged keypad geometry for every current error path in software-only and hardware-connected layouts.

### Implementation needed

Add a focused root Playwright case using the production control UI and stable test-bench error injection. If the bench cannot deterministically create a real ownership conflict, add a test-only authenticated error trigger to the isolated test router; do not expose it from production builds.

### Viewport and layout matrix

At minimum, test the repository's standard 1280×720 Playwright viewport and the 1600×1100 help-screenshot viewport in both:

- software-only controls; and
- hardware-connected main-desk controls.

Include every error currently returned by the Highlight action path, at minimum the different-user ownership conflict and a generic failed/rejected action surfaced by the client. When a new Highlight error variant is added, the matrix must fail until that variant is represented.

### Test case

For every matrix entry:

1. Record bounding boxes for the Programmer grid, HIGH, adjacent keys, and REC/Preload controls.
2. Open representative pane content and a modal surface that would normally overlap the lower desk.
3. Trigger the Highlight error through the production action path.
4. Verify exactly one alert is fully inside the viewport, readable, pointer-reachable, and above pane, modal, and neighboring control layers using bounding boxes, hit testing, and computed stacking behavior.
5. Verify focus can reach the dismiss control and that pointer and keyboard dismissal both work.
6. Verify the Programmer grid, HIGH, neighboring keys, and REC/Preload bounding boxes have not moved or resized from their recorded values.
7. Verify the HIGH label remains exactly `HIGH` and no command-bar status panel appears.

### Pass condition

Every current Highlight error is reachable and dismissible above production content at supported representative sizes without changing keypad geometry. This is an observation-only test and must not alter the accepted software layout.

## HIGHLIGHT-006 — Real hardware-simulator geometry and fader operation

### Missing proof

Current tests assert inline grid styles and matching classes, but do not measure the rendered simulator or operate both faders through the production transport boundary.

### Implementation needed

Launch the built hardware-controls frontend in a browser harness with deterministic Tauri `listen` and `invoke` adapters, or add an equivalent Playwright component harness that loads the production CSS. Do not duplicate the simulator layout in a test-only fixture.

### Test case

1. Open the production simulator at the documented supported viewport.
2. Verify there is no dedicated Highlight display, selection summary, or suppression panel.
3. Measure RECORD and PRELOAD GO. Verify they occupy adjacent equal-width columns, begin on the same row, and each spans the same two complete command rows.
4. Measure HIGH/PREV/NEXT/ALL against GRP/CUE/TIME/DIV and verify the fixed column pairs.
5. Measure Programmer Fade and Cue Fade. Verify they are adjacent, simultaneously visible, equal width, equal full height, and contained within the fader area without clipping.
6. Drag Programmer Fade from a known value to another value and verify exactly one or the documented bounded sequence of `programmer/prog-fade` control writes with the final normalized value.
7. Drag Cue Fade independently and verify the corresponding `programmer/cue-fade` write while Programmer Fade remains unchanged.
8. Exercise keyboard or pointer control at the top and bottom values and verify both faders remain operable.

### Pass condition

The actual rendered hardware simulator proves the restored geometry and sends independent authoritative control values from both full-height faders.

## FIXTURE-002 — Asset and focused physical-metadata revision

### Missing proof

Fixture-profile tests cover schema, modes, channels, color, geometry, migration, and minimal UI creation, but no acceptance case explicitly saves and reloads the Generic asset workflow: photograph preview/replace/remove, fixture icon, orbitable GLB model, and the focused physical fields.

### Implementation needed

Extend the production Fixture Profile editor tests and add a browser-level Fixture Library case. Use small deterministic PNG and GLB fixtures stored under the test fixtures directory. Validate stored server revisions rather than only local component state.

### Test case

1. Open **Create fixture** and populate manufacturer, name, model, device type, description, width, height, depth, weight, power consumption, color temperature, luminous output, and beam angle. Connectors, light source, CRI, and lens remain supported in persisted profiles but are not separate editor fields.
2. Add a valid fixture photograph and verify its preview, persisted media identity, and accessible replacement/removal controls.
3. Replace the photograph and verify the old preview and stored media are no longer referenced; remove it and verify the empty state survives save/reopen.
4. Select a fixture icon and save.
5. Add a deterministic valid GLB model, verify preview/metadata and pointer orbiting, replace it, and verify the stored revision references only the replacement. Notes, photograph, and visualizer occupy equal thirds in that order.
6. Save through the title-bar action and read revision 1 from the desk-wide profile API.
7. Reopen Edit fixture and compare every Generic, asset, and physical field with the authored values.
8. Change one physical field and one asset, save revision 2, and verify revision 1 remains immutable while revision 2 contains the complete new snapshot.
9. Patch revision 2 into a working show and verify the embedded portable snapshot retains the chosen asset/physical metadata without consulting a later library revision.
10. Reopen the show after restart and verify the snapshot remains loadable.

### Pass condition

Every Generic asset and physical field survives the production save/edit/revision/patch/restart path, old revisions remain immutable, and portable show snapshots remain independent of later desk-library edits.

## RELEASE-023 — Matter production interoperability and completion bookkeeping

### External Matter release gate

Automated browser and socket tests do not replace CSA credentials, certification, or an independent controller. Before a production release claims certified Matter support:

1. replace rs-matter development VID/PID and device-attestation credentials with the product's assigned production credentials through an auditable secret/build process;
2. commission a release-equivalent build with at least one independent certified Matter controller;
3. verify discovery, commissioning, fabric persistence, OnOff, Level Control, subscriptions, endpoint add/remove reconciliation, restart, factory-reset/removal behavior, and disabled-bridge disappearance;
4. repeat with a faderless playback and a normal fader playback on non-current global pages; and
5. retain controller version, application build identity, logs, and a signed test record outside the repository secrets boundary.

This remains a release/certification gate rather than an ordinary hermetic CI test. The repository should provide a checklist or opt-in harness, but must not commit production credentials or fabricate certification evidence.

### Completion bookkeeping

When every automated case above passes:

- update `docs/todo-completion-audit.md` with current, reproducible counts and the exact commands used;
- update the testing coverage indexes with `MATTER-002`, extended `SHOW-005`, `UPDATE-002`, `HIGHLIGHT-004` through `HIGHLIGHT-006`, and `FIXTURE-002`;
- ensure every completed planned-feature link resolves to a tracked `.DONE.md` file;
- verify the Feature 20 rename is included as both the deletion of the old path and addition of the `.DONE.md` path before committing;
- run `git diff --check` and a Markdown-link check over the changed documentation; and
- keep implementation/test changes separate from unrelated Feature 21/22 work where practical.

## Final verification gate

This follow-up was marked complete after all of the following passed from the same final checkout:

```sh
./test unit
./test e2e tests/05-virtual-time-persistence-and-recovery.spec.ts --workers=1 --grep 'SHOW-005|FIXTURE-001'
./test e2e tests/11-update-highlight-fixture-profiles-and-matter.spec.ts --workers=1
./test desktop-smoke
cargo test -p light-server --no-default-features --bin light-server matter::transport::tests::commissionable_network_transport_smoke -- --ignored --test-threads=1
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Also build the hardware-controls application, render its production-layout Playwright case, rebuild and verify the PDF/HTML manual when help or testing documentation changes, and run the authoritative `./build open` path with readiness, health, and bootstrap checks.

The final report must distinguish:

- hermetic automated completion;
- opt-in host-network Matter smoke completion;
- independent-controller interoperability evidence; and
- CSA production certification.

Green unit tests alone are not sufficient to close this feature.
