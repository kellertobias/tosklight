# Public Release Acceptance Runbook

This is the repeatable acceptance procedure for ToskLight Control, ToskLight Pixel and ToskLight Architect. It specifies testing, evidence and reporting; it does not authorize product fixes, calibration implementation or release publication. The initial campaign is [TL-666](https://plans.tokenet.de/TL/666). A previous run passing does not validate a new build.

## How to run this again

Give an agent this instruction, replacing the candidate and optional platform list:

> Execute docs/testing/42-release-acceptance.md against release candidate <tag/commit/artifact>. Create a fresh dated evidence folder using the canonical artifact helper. Reuse or create PLAINER acceptance tracking, read current contracts, and test through the actual applications' UI. Capture every action with intention, expected and actual results baked below the screenshot. Repeat supported operations through OSC and verify emitted DMX and both network protocols. Keep an execution ledger, file confirmed bugs in Defined with screenshot briefings, and leave blocked/untested and human-only checks explicit. Do not implement fixes or calibration features. Finish the required acceptance gates before preparing the licensed marketing gallery. Report remaining release blockers and the paths to results.

Run the phases below in order. Resume only from an identified run's ledger, on the same candidate and saved test-show revision. If candidate, profile, media content or configuration changes, record the change and rerun affected cases and downstream consumers. Preserve earlier failures and link retests; never overwrite their evidence.

## Release gates and tracking

| Stream | Initial issue | Cases |
| --- | --- | --- |
| Campaign, environment, usability, persistence, OSC | TL-666 | ENV, UX, SAVE, OSC |
| Fixture import, patch, intent, replacement, network | TL-667 | SET, MAP, NET |
| Programmer, groups, Dynamics, playback | TL-668 | PRG, GRP, DYN, PB |
| Pixel–Control integration | TL-669 | MED |
| Architect, CAD and visualization | TL-670 | VIZ |
| Licensed marketing gallery | TL-671 | MKT |
| Guided calibration design only, Defined | TL-672 | design proposal below |

Read current PLAINER ownership before starting any stream. Testing children remain independently claimable; serialize live desk mutations. A completed run goes to Review / test; only explicit human acceptance approves the release. Link existing defects instead of duplicating them. Repeated runs should have their own run record, while retaining the stable case IDs here.

Required release gates: all requested supported UI cases pass on the candidate; supported OSC variants pass separately; actual transmitted DMX and received visualization pass for Art-Net and sACN independently; persistence/recovery pass; no unresolved critical failure or unusable required workflow; unfamiliar experienced-operator sign-off exists. For this equipment-free campaign, independently documented packet assertions substitute for physical equipment in software-output acceptance; physical-rig sign-off is recorded separately as unavailable, not a gate preventing software acceptance. Unsupported requested behavior is a documented acceptance gap, not a pass. Marketing captures follow acceptance of the views they depict. A test environment blocker is not automatically a product defect.

## Environment and reusable test show

1. Record date/time in Europe/Berlin, source commit, dirty diff, release version, archive checksum, executable and bundle paths, OS/GPU, display pixel dimensions, scale, UI viewport, firmware and connected equipment. Prefer the packaged release candidate. A mixed old frontend/new backend is diagnostic evidence only.
2. Preserve the user's active show, named revision, desk settings, profile library and media configuration through normal recovery/export facilities. Create a separate show named `Release Acceptance <run ID>` through the UI. Do not modify an existing production or demo show in place. Restore the original active show and changed desk settings after the run.
3. Build a small rig with eight fixtures: IDs 1–4 RGBW moving washes and 5–8 another moving type with color and Zoom, on two patch layers. Record exact manufacturer, mode, profile revision/UUID, universe/channel ranges and capabilities in the run manifest. Include 16-bit Pan/Tilt and one multi-patch copy. Use real profiles/models, not generic artwork. Plan replacement types: wheel-only movers for 1–2 and RGBWA fixtures for 3–4, with their real capabilities and sourced data. Unsupported axes/optics must be reported, not invented.
4. Place lamps on a documented 4×2 layout with nonzero height, mounting orientation and a saved target Point. Define Group A (1–4), Group B (5–8), All and derived Odd/Even. Keep test IDs stable; do not delete/recreate fixtures to stand in for replacement. Add intensity, red/blue/white Color, target/angle Position and Beam presets; use distinct values so scope leaks are visible.
5. Use an isolated lighting network or loopback receivers for software packet checks. Record Art-Net universe mapping and sACN universe mapping separately. If physical equipment is available, add receiver/fixtures for actual light/aim validation; otherwise execute the independently sourced software-packet variant and leave measured light/aim unavailable. Pixel must have a known output and at least eight enabled layers; add a second output/server for isolation tests where available.
6. For timings use 0 s, 0.2 s and 2 s fades, 0.3 s delays/follow and unequal in/out times. Record timestamped frame samples during transitions and interrupted transitions. Judge tolerance from configured output rate and profile resolution; do not choose a passing tolerance after seeing the result. Packet/frame checks and real light are distinct evidence levels.

For the initial run the user confirmed no physical equipment is available. Use a custom receive/validation script with expected slots, ranges, coarse/fine order and physical endpoints independently derived from the selected lamps' manufacturer manuals. Keep source URL, manual version/page, selected mode and calculation beside each assertion. Capture actual emitted Art-Net/sACN packets; neither ToskLight's fixture profile nor its own DMX monitor is an independent oracle. Validate representative values, transitions and replacement mappings. Physical light/color/aim remains untested and must not prevent software variant acceptance once its documented UI/packet assertions pass. Do not claim measured color closeness, beam angle, brightness or physical aim from these packets. Manufacturer-native byte output alone does not pass semantic intent: physical-unit mapping/replacement cases still require independent expected recipes/endpoints and actual emitted values. A starting fixture-documentation source is the [ROBE Robin 300 LEDWash manual](https://www.robe.cz/res/downloads/user_manuals/User_manual_Robin_300_LEDWash.pdf), version 1.62, DMX chart pages 17–19; confirm the exact mode before deriving expectations.

The reusable receiver is `tests/bench/release-dmx-acceptance.py`; run `--help` for its JSON schema and capture options. Its offline parser/assertion checks are `python3 -B tests/bench/release-dmx-acceptance-test.py`. Resolve `artifact-path -- test-results` for packet-output destinations (a fresh case subdirectory); link these from the visual evidence ledger. Set the stable look through UI, author expectations from the manual, then capture with explicit source IP, wire universe and a minimum frame count. Missing frames, wrong-source-only captures, malformed packets and any eligible mismatching frame must fail. The receiver excludes preview, termination and nonzero-start-code frames; it does not establish merged/synchronized rendered output. Fades/Dynamics require additional timestamped trajectory analysis, not only stable-look assertions.

## Evidence, result ledger and bug briefing

Resolve `visual-inspection` with `npm run --silent artifact-path -- visual-inspection`. Create `release-acceptance/<date>-<candidate>-<run>/` underneath it. Resolve scratch through `artifact-path -- tmp`. Confirm representative evidence paths are ignored with `git check-ignore` before capture. Never add images or downloaded files to Git. The reusable runbook is tracked; run-generated results are ignored.

Folder structure: `manifest.json`, `results.md`, `steps.jsonl`, `raw/`, `steps/`, `briefings/`, `protocol/`, `inputs/`, `marketing/`, `licenses/`. Every action has an ID such as `PRG-03-007`, timestamp and its parent scenario. Capture modal opening, input changes, confirmation, playback press/release, errors and recovery individually. Deterministic multi-key entry can be one described action; capture each decision or changed state, including failed attempts. Preserve raw screenshots unchanged. Append a readable footer below the image, never over controls, with **Intention**, **Expected**, **Actual**, status, case/step ID, candidate and time. Captions contain observations, not assumptions. Timing/Flash needs before/during/released frames plus timestamped output evidence.

Capture from a surface at least 1920×1080; 3840×2160 is preferred. Cropped programmer/detail pictures may be smaller if the original source meets the requirement. Do not upscale a small capture and call it Full HD. Below-resolution diagnostics may identify a failure but leave the capture gate blocked. Keep clean marketing images separately, without evidence footers, with caption metadata alongside.

Each result row records: case/variant, steps performed, expected, actual, status, exact show revision/fixture identities, raw and captioned links, DMX/protocol evidence, issue, limitations and next action. Status is **NOT RUN**, **RUNNING**, **PARTIAL**, **PASS**, **FAIL**, **BLOCKED**, or **UNSUPPORTED**. **RUNNING** means execution is currently active. **PARTIAL** means witnessed bounded outcomes exist with required variants still to run; it does not mean the case passed or a worker is still active. Record UI, OSC, packet, visualizer and physical variants independently. **PASS** requires every assertion in that variant and its evidence; passing setup does not pass the entire scenario. Tests already in the repository are supporting evidence and do not count as newly witnessed UI acceptance.

For each finding write `briefings/<finding-ID>.md`: impact/severity, candidate/environment, preconditions, numbered UI reproduction with a captioned image per step, expected versus actual, frequency, relevant logs/packet samples, recovery/workaround, source contract and PLAINER link. Confirmed bugs and unusable required flows become **bug** items in Defined. Uncertain behavior or a desired capability becomes a **plan** in Defined with the uncertainty explicit. P0: crash/data loss/uncontrolled output; P1: required workflow impossible or incorrect programming/output; P2: material usability or inaccurate feedback; P3: cosmetic. These severity labels are briefing conventions, not invented PLAINER priority values.

## Phase 0 — Candidate, safety and usability baseline

| ID | UI procedure | Expected and evidence |
| --- | --- | --- |
| ENV-01 | Launch Control via candidate package or `npm run open` for an existing development build; inspect connection and Running & Output. Record process/bundle identity and readiness. | One authoritative desk, connected UI, no pane error. Separate stale-bundle, sandbox and product failures. Time readiness and bootstrap separately if UI stalls. |
| ENV-02 | Open Pixel and Architect from supported launch paths; inspect startup, service/output state and recovery messages. | Each opens as intended; no silent missing helper or blank output. Same candidate identity recorded for all apps. |
| ENV-03 | Establish >=Full HD surface; inspect software-only and hardware-connected layouts, touch controls and fullscreen. | No clipping prevents required operations. Attached OSC control remains the same desk; different alias isolation follows current contract. |
| UX-01 | Ask a lighting operator unfamiliar with ToskLight to create a show, import/patch/place a lamp, make a colored Position preset, record/start/release a cue, then save/load, using normal help only. | Record time, missteps, unclear terminology, blocked controls and assistance. Agent familiarity cannot substitute for this human usability gate. |
| UX-02 | Exercise actual pointer drags, touch/hold, keyboard/keypad and supported hardware controls on required editors. End drags by release, blur, hidden window and cancellation. | Equivalent semantics, useful labels/units, touch targets, no stuck gesture or unexpected changes. |

## Phase 1 — Show setup, interchange, fixture UI and physical intent

| ID | UI procedure | Expected and evidence |
| --- | --- | --- |
| SET-01 | Create empty show; name it; add rig through Show Patch; create revision. | Autosaved independent show; expected fixture IDs, selected modes, counts and original show preserved. |
| SET-02 | Find and download real GDTFs from the Internet; open Fixture Library import preview, inspect modes/native channels and map verified unknown attributes, import and patch. Repeat for at least RGBW and wheel/RGBWA types. | Source URL, date, hash and chosen mode recorded. Coarse/fine slots/defaults/functions match source. Unsupported functions stop or warn honestly. Original source retained. Preview/cancel changes nothing. |
| SET-03 | Import a real MVR through New Show → Load from MVR; inspect preview and apply to a separate show. | Source fixture/mode identity, fixture counts, translations, rotation, layers and primary addresses match manifest. Conflicts/unresolved profiles/extra breaks are visible; Import unpatched leaves all outputs unpatched. |
| SET-04 | Exercise overlap, invalid address, missing profile/mode, unsupported scenery and cancel/repreview cases. | Actionable errors, no silent substitution/partial corruption. Correct and retry through UI. |
| SET-05 | Edit profile channels and physical data using Fixture Library forms, select revision in patch. | Position axis/function, color paths, angular Zoom and optional Iris can be understood/configured. Retained data and quality are honest. Embedded show revision is not silently replaced by unrelated library edits. |
| SET-06 | Place lamps via Stage and Architect CAD, numeric positions/rotation and drag; duplicate/multi-patch and move a copy. | Exact coordinates/orientation, undo, independent copy properties and show persistence. Native model/emitter beam placement matches data. |
| SET-07 | Spread values over ordered selections; reverse; change placement; use Group projection/phase grid, radial and radar. | First/last/ranks/order predictable; same-rank fixtures match; missing Stage position has documented fallback. No selection leak. |
| SET-08 | Export MVR through Save As → Export MVR; inspect export summary; reimport as new show and compare, using external reader where available. | Fixture identities, supported modes, patch/layers/placement preserved; limitations explicit. Unedited retained GDTF and edited generated subset distinguished. MVR does not promise programming portability; use `.show` for that. |
| SET-09 | Export/reimport `.toskfixture` and save/reopen `.show` through UI. | Profile source/physical data and complete show programming survive their correct portable formats. |
| SET-10 | Unpatch 1–4, then repatch them before replacement. | Fixtures stay selectable, in groups/presets/cues/Stage; only their DMX is suppressed. Deliberate deletion is a separate operation. |
| MAP-01 | Configure Pan/Tilt function units, direction/range and installed zero/axis correction; program angles and saved Target. Compare monitor, packet and Stage. | Correction applied once; mounting separate; signed/multi-turn angles preserved; target aims using real geometry. Root and copy independent. Physical aim needs measured rig proof. |
| MAP-02 | Configure color model; use Color UI for red, blue, white blend and out-of-gamut color across mixed rig. | Understandable requested color, approximation/uncalibrated/wheel-limited states, correct neutral controls, intensity independent. Compare decoded DMX and physical light, not raw RGB equality across different lamp types. |
| MAP-03 | Configure Zoom endpoints/samples and Beam/Field convention; set two angular Zoom values, independent Focus, and Iris where supported. | Zoom in degrees, Focus as travel percent, Iris separate. Unknown convention/unsupported hardware reports limitation without inventing physical intent. No hidden Focus edit when Zoom changes. |
| MAP-04 | Clear programmer; run color/Position/Zoom cues. Replace 1–2 with wheel movers and 3–4 with RGBWA types using identity-preserving UI replacement; retain location/mounting and valid addresses. | Group/preset/cue references and requested intent remain. Actual channel recipes are correct for new profiles; feasible color/aim close, limitations explicit. Stale calibration retained inactive/reviewed, not blindly applied. |
| MAP-05 | Recall/run original presets and cues after replacement, repeat Dynamics and save/reopen. | Same intent, supported aim/Zoom, compatible source behavior and no deleted reference. Record before/after DMX and visible/physical evidence. Compare numerical resolution and attainable gamut, not invented exactness. |
| MAP-06 | Inspect current calibration forms, invalid data, cancel, independent copies and stale profile/mode/geometry measurements. Write guided-calibration design proposal. | Honest provenance/quality and saved-state behavior; identify live versus save-only calibration using witnessed output. Design remains TL-672 in Defined; no implementation. |


### Replacement history and explicit ownership regression

Use an isolated show and a real replacement whose master attributes move to two independently controlled heads. Record the exact old/new profile revisions, fixture UUIDs, source attributes and operator choices. Choose the replacement projection explicitly in the UI; missing decisions must reject before persistence and leave patch, programming and output unchanged. Fresh master programming after replacement must follow the new profile contract, without inheriting the old fixture root's consent.

Repeat these independent sequences with actual Art-Net and sACN captures:

1. Program the old root at 40%, replace, then Undo and Redo. Replacement creates no separate Programmer edit: Undo reverses the prior value edit while physical topology stays installed; Redo restores the consented intent to both destination heads.
2. Program at 40%, Clear, replace while the programmer is empty, then Undo Clear. The restored historical intent reaches both heads. Repeat when a historical-only Color or Position attribute requires another decision; the preview must request that consent or reject safely rather than guess.
3. Program 40%, change to 80%, replace, then Undo and Redo. Values follow the ordinary edit history while consented destination ownership remains correct.
4. Repeat for pending and active Preload, including Undo/Redo and Clear. No unrelated normal, pending or active values change.
5. After replacement, make a fresh explicit master edit. It follows only the new profile's documented master behavior. Undo restores the prior mapped intent; Redo restores the fresh edit without a stale projection envelope.

Save a named revision and reopen through the UI. Repeat stored preset/cue/group output and the existing repeated-replacement tests. Keep transient Programmer history proof separate from persisted programming proof. Neither equal values nor fixture numbers establish source identity.

Downloaded test inputs must have provenance and checksums. Initial research leads: [GDTF Share](https://gdtf-share.com/) and its [download/revision guide](https://gdtf-share.com/help/users/gdtf_share/). A source discovered online is not yet a downloaded or imported fixture. Never execute scripts from a fixture archive.

## Phase 2 — Settings and network output

| ID | UI procedure | Expected and evidence |
| --- | --- | --- |
| NET-01 | In Desk Setup configure Art-Net interface/destination/universe/rate; enable, vary fixture values, disable and re-enable. | Actual UDP receiver sees correct slots/universe, intensity/color/Pan/Tilt and fine bytes. Disabled output stops as documented; UI settings persist. Monitor alone does not prove sending. |
| NET-02 | Repeat separately with sACN, including selected multicast/unicast, universe and priority where offered. | Actual received frames match values and mapping; no accidental Art-Net dependency. Document receive/send source and sequence. |
| NET-03 | Exercise invalid interface/destination, address conflict, receiver/network loss and recovery; inspect other settings, screen assignments, preferences and defaults. | Actionable errors/progress, intended setting persistence, no stale connected status, no programming changes. Restore changed desk settings. Grand master/blackout affect actual output and restore correctly. |

Before leaving Settings, inventory every currently exposed settings page/control and record which were exercised, intentionally unavailable or still untested. For the release-relevant settings, open the page, verify current values and units, make one reversible supported change, apply/reopen to verify persistence, then cancel or restore it. Art-Net/sACN output acceptance remains NET-01/02; a page merely opening does not pass its behavior. Preserve original desk settings and capture each action. This covers the user's general “settings work” request without inventing hidden settings or claiming all pages from two protocol checks.

## Phase 3 — Linear programming, presets, groups and timings

Start without Dynamics, loops, chasers or follow chains. Snapshot resting DMX first.

| ID | UI procedure | Expected and evidence |
| --- | --- | --- |
| PRG-01 | Set color and Position, record presets, record a cue from preset recall. Clear programmer with playback stopped. | Output returns to original resting values; merely storing a cue does not start it. Preset content/count/preview correct. |
| PRG-02 | Recall preset A on fixture 1 only, preset B on fixture 2 only; inspect all others. | Exactly selected fixtures receive compatible values, no stale selection or universal-preset scope leak. |
| PRG-03 | Store this as cue 1 of list 101; clear; assign list to physical playback; start; clear again. | First clear returns rest until playback starts; second clear leaves playback DMX unchanged and ownership from cue. |
| PRG-04 | Run the cue, select fixture, edit recalled preset color/Position, use Record Merge into preset, then clear the programmer without GO or reload. | Merge keeps unrelated preset values. The already running linked cue adopts the changed source after Clear, proven by actual Art-Net and sACN. A restart-only change does not pass live propagation. |
| PRG-05 | Repeat independently with Update → Targets while the linked cue runs; inspect offered targets, choose preset and commit/cancel, then Clear without GO or reload. | Correct source targets offered; only chosen objects change; cancel unchanged. Committed source changes reach the already running linked cue after Clear. Test Smart vs Merge/default modes per scenario 24. |
| PRG-06 | Add cues 2–4; unequal In/Out fade and delay, per-attribute time where supported; GO, GO-minus, pause, load next, interrupt and release. | Correct tracked values/ownership, timed DMX frames and live state. No loops required. |
| PRG-07 | Add Follow/TIME triggers after linear baseline; start/stop/restart list. | Exact trigger origin, no stuck clock or runaway sequence. Existing release/Temp contract remains valid. |
| PRG-08 | Repeat recording by target-pool touch, explicit cue address and playback target, on software/hardware layouts. | Recording consumes target interaction instead of also firing playback. Record Merge and Update preserve their own semantics. |
| PRG-09 | Inspect output with two simultaneous sources and programmer edits. | Programmer LTP remains distinct from cue HTP/LTP; Clear reveals underlying owner, not stale programmer data. |
| GRP-01 | Record values while Group A is a live reference; store group-based preset/cue; change A membership to include fixture 5. | Stored source is Group, not flattened list; cue/preset intended group membership updates and ordering/spread follows. |
| GRP-02 | Repeat with Group B, overlap A/B, then remove member. | Correct precedence/scopes; excluded member no longer receives the group's programming, independent sources unaffected. |
| GRP-03 | Create Odd/Even derived Groups; change source order/membership and run cues. | Derived subsets update predictably, disjoint coverage where intended, phase/rank ordering preserved. |
| GRP-04 | Store deliberately empty Group; address range with a missing number; double-press Group to freeze selection. | Empty differs from absent; missing skipped; frozen selection does not later gain members. |
| GRP-05 | Repeat group cues after unpatch/repatch and MAP-04 replacement. | Group identity/membership and stored intent survive; DMX only on patched supported destinations. |

## Phase 4 — Dynamics, Speed Groups and playback arbitration

| ID | UI procedure | Expected and evidence |
| --- | --- | --- |
| DYN-01 | Create intensity/Position/Color Dynamic on fixtures with static values; record into cue/list; clear and run. | Correct baseline, size/speed/phase, output waveform and stored references. |
| DYN-02 | Apply reusable Dynamic with no prior fixture values to A and separately B; run both and release one. | Defined behavior without hidden programmer baseline; target scopes/instances independent. |
| DYN-03 | Edit Dynamic definition while its cue/list plays, then stop/restart. | User expectation: cue references new definition; output and scope update without unwanted clock reset. Record evidence of reuse versus snapshots. |
| DYN-04 | Assign Dynamic directly to physical and virtual playback, with no cuelist wrapper authored. | Direct assignment works, release independent from programmer/cue instances. Unsupported UI route is a tracked gap. |
| DYN-05 | Assign two independently running Dynamics/chasers to Speed Groups; change group speed, tap, pause and restore. | Correct members only, synchronized intended timing, unrelated clocks unchanged; UI/OSC feedback agrees. |
| PB-01 | Configure white base cue to remain active; start red overlay then release it. | Base stays active while overwritten and white returns to output after overlay release. It does not repopulate programmer; inspect owner separately. |
| PB-02 | Separately enable full-takeover Auto-off on base, start full replacement; repeat partial replacement. | Full normal takeover turns base off only under configured contract; partial takeover does not. |
| PB-03 | Repeat full replacement with literal held Flash and Temp press/release/cancel. | Underlying playback preserved; documented Temp timing follows/release behavior, no stuck pressed state. Capture real hold, not adjacent Toggle handler. |
| PB-04 | Populate virtual cells with red/blue cues, make Solo Region, set master fade 2 s and switch. | Only winning steady-state playback; both outgoing/incoming output contributions across fade, no blackout/jump. Logical Off may still have release contribution; two On cells are not the sole oracle. |
| PB-05 | Repeat with nonmember region/cell, different panes/pages and independent Solo Region. | Exclusivity limited to region; stable numbers shared across views; unrelated playback continues. |
| PB-06 | Change pages while using physical/virtual playbacks and OSC current/explicit addresses. | Current-page commands follow page; explicit page retains target; wrong virtual bank rejected without changing output. |

## Phase 5 — Pixel integrated with Control

This phase is focused integration acceptance, not a wholesale rerun of every Pixel feature.

| ID | UI procedure | Expected and evidence |
| --- | --- | --- |
| MED-01 | Discover Pixel, patch suggested master/layers, upload licensed content through UI; select/control from Control Media pane. | Correct server/output/layer identity, playable actual output, progress/errors visible. |
| MED-02 | Change file, layer, effect and output; watch library/live/cue thumbnails. | Correct thumbnail invalidation, server/layer isolation; no stale image. Record transition and resulting preview. |
| MED-03 | Record cue containing only media server Program/master, clear and play. | Cue thumbnail is that Program output, never Stage or arbitrary library image; programmed playback matches preview. |
| MED-04 | Record only layer 2 with alpha/mask, clear and play. | Thumbnail contains that layer only, retains transparency/checkerboard and effects; another layer/server never leaks in. |
| MED-05 | Record effects-only cue with no file/media-selection changes, then run over two different existing files. | Each file continues while effect changes. Cue does not replace media or restart it unexpectedly. Check stored scope and received DMX. |
| MED-06 | Media color intent, transition/transport, offline server/missing output/empty file and reconnect; operator image override and one→two-cue virtual preview behavior. | Honest fallback labels and refreshed previews, per scenarios 14/25; same content after recovery. |
| MED-07 | Repeat supported programming/control/playback over desk OSC and playback via guest; compare screenshot/output/frame evidence. | Same scoped result and feedback; unavailable authoring routes marked unsupported. |

### Repeatable media authoring and OSC replay

Use a separate test show and licensed sources assigned through Pixel's UI to folder 3/file 2 (background) and folder 3/file 1 (an alpha-bearing foreground). Patch Pixel's canonical eight-layer personality: 59 slots per layer plus 40 master slots, 512 total, starting at address 1 on an isolated configured universe. Record the actual fixture number and output identity in the run ledger. The values below use that layout; adapt slot offsets explicitly when testing another patch.

1. Keep the native Control application open with the Programmer's **Media** family mounted and its visible encoder labels showing **Folder** in slot 1 and **File** in slot 2. Select the Pixel fixture's logical layer 1 using the native UI or supported keypad selection. Start from blank folder/file values, with no earlier playback contributing media values. Encoder events act on this mounted surface; they do not prove standalone headless media authoring.
2. Subscribe `/light/subscribe` with a fresh client ID, `desk` and a feedback port, from the socket that will send the commands. Require subscription feedback before authoring. Send `/light/desk/encode/1` with string `up` three times, then `/light/desk/encode/2` with string `up` twice. Each detent advances the normalized media address by `1/255`, so the resulting bytes must be folder 3/file 2. Optional request IDs are distinct strings following the direction argument. Allow each action to reach the native display and verify the selected source rather than sending an unchecked batch.
3. Set layer 1 intensity using native or OSC keypad **AT 100 Enter**. OSC keys use `/light/desk/programmer/{key}`: `at`, `digit-1`, `digit-0`, `digit-0`, `enter`, each with pressed `1` then released `0`. Select logical layer 2, keep the same mounted Media page, advance Folder three detents and File one detent from blank, and set its intensity to 100. Select the Pixel master/logical head 0 and set intensity to 100. Verify both sources and the actual composite before recording.
4. Record a fresh independent Cuelist using literal **Record, Cue, Cue, list-number digits, Enter**, through native keys or `/light/desk/programmer/record`, `/cue`, `/cue`, `/digit-*`, `/enter` pressed/released pairs. Confirm Cue 1 contains exactly the two folder/file pairs, two layer intensities and master intensity, without unrelated lighting or effect changes. Clear through native or OSC `clear` twice and verify zero programmer values and selection.
5. Through native UI, assign the new Cuelist temporarily to explicit page 2/playback 7 after preserving that slot's original assignment. Subscribe a separate guest on `remote`; send `/light/playback/2/7/off` and `/light/playback/2/7/on`, each pressed `1` then released `0`. Observe native playback state, Pixel's saved output image and actual transmitted/received DMX. Changing the current page must not change this explicit target. Unsubscribe each client and close its socket; restore the previous assignment and network routes during cleanup.

For the layer-only variant, clear before authoring, select only logical layer 2, set its folder/file and intensity, and record a separate fresh cue. For the master-only variant, run full Cue 1 with an empty programmer, select only logical head 0, set **AT 50 Enter**, and record Cue 2 in the same Cuelist through the native Record workflow. Clear again, then start Cue 1 and advance to Cue 2 so tracking has an explicit source. Inspect the stored changes before accepting either thumbnail; a convenient image alone does not prove cue scope.

Media Folder/File absolute numeric entry uses percent, not a folder/file index. Verify the UI explains this and record any ambiguity as a usability finding: entering 3 percent maps to byte 8, 2 percent to byte 5, and 1 percent to byte 3. Index 3 corresponds to approximately 1.17647 percent, index 2 to 0.78431 percent, and index 1 to 0.39216 percent. Named browser selection and encoder detents provide independently checkable source choices. Compare persisted normalized values by their resulting address bytes; small decimal rounding differences do not establish a media selection difference.

Do not infer an absolute media-writing route from `/light/desk/programmer/fix-at`. It is the documented Dynamics FAT path, whose attribute support must be checked separately; sending `media.folder`, `media.file` or ordinary intensity there is not a substitute for this encoder/keypad procedure. Supported mounted encoder authoring, guest playback and unsupported absolute routes receive separate ledger entries.

| Variant | Exact wire expectations for the address-1 eight-layer patch | Persisted scope and thumbnail checks |
| --- | --- | --- |
| Full two-layer Cue 1, after Clear and ON | Slots 1/2 = 3/2; 15 = 255; 60/61 = 3/1; 74 = 255; 473 = 255. Capture the complete 512-slot payload and compare native-authored versus OSC-authored replay, not only these seven endpoints. | Exactly seven media/master changes. The cue thumbnail shows the Program composite, including both layers, rather than Stage or an arbitrary library still. Actual Pixel output matches it. |
| OFF | Layer dimmer slots 15 and 74 = 0 when no underlying source is active. Check master and source addresses against the declared base state rather than assuming every slot becomes zero. | Cue data remains stored; playback releases its contribution. ON restores the full composite without programmer values. |
| Layer-only foreground cue | Its contribution changes layer 2 only: 60/61 = 3/1 and 74 = 255. Underlying layer 1 and master retain their independently declared values. | Only layer 2 changes are stored. Its thumbnail isolates the alpha-bearing foreground/checkerboard and its effects; background, other layers and other servers do not leak into it. |
| Tracked master-only Cue 2 | Slots 1/2, 15, 60/61 and 74 retain Cue 1's values; master slot 473 = 128 for stored intensity 0.5. Clear leaves these playback values unchanged. | Exactly one master intensity change, no media selection or layer intensity. Its Program thumbnail shows the tracked composition at half master, not a blank image or isolated source. |

Save a named revision, reopen it through the UI and repeat the full, layer-only and tracked master-only replay/thumbnail checks. Inspect stored cue scope read-only and wait for regenerated live previews; persistence of programming and correct thumbnail resolution are separate assertions. Capture the exact saved screenshot file and inspect those same pixels before captioning an output failure. Repeat packet and received-output checks independently for Art-Net and sACN under MED-07/OSC-01.

## Phase 6 — Architect CAD and received-DMX visualization

| ID | UI procedure | Expected and evidence |
| --- | --- | --- |
| VIZ-01 | Open → Load from ToskLight Control; select acceptance show and open it. | Correct show/profile/patch/placement, association and sync status; no accidental different desk/show. |
| VIZ-02 | Enable Architect Art-Net receive; run fixed colors/angles/target/Zoom and transitions from desk; change received channels independently. | Receiver actually receives packets; Stage derives appearance from native DMX/profile data, not solely desk semantic state. Correct wheels/emitters/axis/beam convention. |
| VIZ-03 | Disable Art-Net and repeat using sACN receive alone. | Same intended visualization and universe mapping with actual received evidence; loss/recovery status correct. |
| VIZ-04 | Repeat after MAP-04 replacement, with mounting/calibration, copies and limits. | Visualized emitted output agrees with desk/packet physical model; approximations/unsupported capabilities remain honest. |
| VIZ-05 | CAD plan: place/move/rotate, group/ungroup, dimensions/annotations, patch, library/profile and fixture management. Save/reopen. | Exact position/units/undo, usable real interactions and correct persisted data; screenshots for each named screen. |
| VIZ-06 | Test bidirectional desk sync, offline edits/conflict/reconnect and changed active show using scenario 31. | No lost/duplicated edit or unintended output restart; wrong show blocked, recoverable draft preserved. |

## Phase 7 — OSC parity and persistence/recovery

| ID | Procedure | Expected and evidence |
| --- | --- | --- |
| OSC-01 | Subscribe desk path, selection/keypad/record/update/encoders/family values and spreads; reconnect. Repeat representative PRG/GRP/DYN/PB/MED cases. | Shared command line and authoritative desk state; exact typed units/selection order; feedback captured. Record outgoing messages and incoming feedback beside UI images. |
| OSC-02 | Subscribe guest remote path; trigger playback while desk Record armed; attempt programmer/record/update. | Playback unaffected by armed Record; forbidden programming refused; no extra desk/programmer created. |
| OSC-03 | Current/explicit physical page and virtual stable-bank addressing; bad arguments, unsubscribe/reconnect and release. | Documented routing and errors without mutation; no stuck Flash or unintended cross-page action. Definition editing has no OSC route and is recorded unsupported. |
| SAVE-01 | Save named revision/export `.show`; restart Control/Pixel/Architect; load latest and revision-as-copy. | Complete programming, references, profile evidence, media assignments and intended separate desk settings survive; original autosave not rewound. |
| SAVE-02 | Open supported old show, declared contract-0 rejection and malformed show copies. | Explicit migration or current documented refusal; preserve source, actionable recovery and separate new show; application still starts. Never corrupt production input to perform this test. |
| SAVE-03 | Run demo-sized show (~300 fixtures), switch Stage/Fixture Sheet/programming views while output runs; extended 1,000-fixture test where available. | Operator controls and output remain responsive. Measure full interaction→DMX path; bounded Stage load does not starve desk. Report measured values, dropped frames and soak duration. |

OSC routes come from current `docs/help/90-Protocols/01-osc.md`; command semantics from `docs/help/10-Desk/20-Programmer-and-Cues/01-command-line.md`. Enumerate each UI operation's supported route before attempting parity. No API setup/programming may substitute for an acceptance UI action. Read-only inspection and packet capture may substantiate it.

## Phase 8 — Marketing show and gallery

| ID | UI setup and deliverables | Acceptance |
| --- | --- | --- |
| MKT-01 | Create independent polished demo-show copy: purposeful symmetrical/asymmetrical rig, scenery, warm/cool looks and beam layers. Populate visible physical and virtual playbacks, including Speed Groups; meaningful names and no debug errors. | Real models, actual output and valid programming; arrange camera/UI deliberately. No fabricated app view. |
| MKT-02 | Architect: hero PreViz, Patch, CAD, Fixture Library and Fixture Management. | Five clean images, correct geometry, intentional light design and readable relevant controls. |
| MKT-03 | Control: Presets, populated cuelist, desktop containing several preset families, Stage preview, virtual grid and populated physical playbacks/media. Optional programmer detail crops. | At least three complete clean views from >=Full HD source; active playbacks visibly intentional, speed control shown. |
| MKT-04 | Pixel: media management and Visualizers management; enumerate every runtime visualizer and effect and create a labelled checklist. Render each visualizer attractively and capture one image per effect over licensed content. | Inventory count equals captures; effect parameters visibly differ from baseline; deterministic time/audio/source logged. No guessed static list or missing presets. |
| MKT-05 | Build eight active content layers: top black→transparent mask/overlay, application logo, blue bar behind logo, CC0 media, then four distinct background/foreground content layers with meaningful alpha/masks/effects. Capture composite and layer-management view. | Eight real contributing layers, compositing order and isolated previews prove each contribution. Count eight source-bearing layers with independently visible contributions; the requested source-bearing alpha/black mask, logo and authored blue bar are valid contributions. An effect-only configuration with no source content does not create an extra content layer. Retain clean outputs and setup revision. |

Find CC0 assets online and record exact asset URL, author, license page/text snapshot, download date/hash and any derivative steps in `licenses/manifest`. Royalty-free alone is not CC0. Logo uses repository branding under its own provenance. Research leads only: [Blender Institute CC0 texture archive](https://download.blender.org/archive/textures/) and individually labelled [Blender demo resources](https://www.blender.org/download/demo-files/). Do not assume all Blender movies or media labelled free are CC0. Capture chosen license evidence before use. UI-upload sources to Pixel and retain the reproducible eight-layer configuration.

## Guided calibration design to deliver, without implementation

Evaluate current forms before proposing new UI. Proposed flow: choose physical fixture/copy and actual profile/mode; explain safe commissioning; record mounting/reference frame; guide known Pan/Tilt reference points and direction/zero with residual errors; measure Color emitter gains or complete optical recipes using colorimeter/spectrometer evidence; measure Zoom beam/field angles at several native positions; keep Focus travel and Iris separate. Preview before/after predicted DMX/Stage and measured light; explicitly save or cancel. Attach source, instrument/date, quality, uncertainty and revision to observations. Handle out-of-range/contradictory samples without inventing precision. Copies need separate measurements, and hardware/profile/mode/geometry changes need calibration validity review. Specify permissions, interruption recovery, portable storage and acceptance tests in TL-672. No new feature is implemented in this campaign.

## Source contracts and acceptance gaps

Read the relevant numbered help pages immediately before each phase. Existing focused contracts include testing scenarios 14, 16, 18, 24–41, particularly `31-fixture-physical-mapping.md`, `31-control-architect-show-sync.md`, `33-semantic-intent-persistence-coverage.md`, `34-position-operator-controls.md`, `35-focus-zoom-operator-controls.md`, `36-semantic-color-controls.md`, `39-dynamic-editor-family-lanes.md`, and `41-mvr-export-summary.md`. `docs/acceptance-criteria.md` defines supported persistence and the declared programming-contract break.

Resolve these with actual observation; do not weaken the user's request silently:

- Older initiative notes deferred general cue→preset live linking. The release acceptance contract requires live propagation in PRG-04/05; verify the current build without restarting or reloading the running cue.
- Group selection can be live or frozen; use live references for propagation tests.
- Solo steady-state exclusivity and ongoing outgoing fade contribution are different assertions.
- Physical color closeness and aim cannot be signed off by software simulation alone; TL-523 covers real rig acceptance.
- Iris is a separate control; portable semantic intent is not assumed for it.
- MVR help has apparently different descriptions of generated GDTF optical fidelity. Check actual export and make a documentation finding if the pages disagree with each other or output.

## Import progress acceptance

Long MVR/GDTF inspections show a progress bar with phase, completed/total work where measurable, and elapsed time. Use indeterminate progress when percentage cannot be measured; never invent a percentage from a timer. Cancel must preserve the current show and prevent late application. Record request-to-preview duration and responsiveness during a demo-sized import. The initial 312-fixture MVR run exposed only static `Inspecting…`; track its performance and progress feedback separately.

## Error severity and uninterrupted programmer acceptance

User clarification, 2026-10-08: disruptive red errors are reserved for loss of DMX output, loss of the show, or critical system danger. A recoverable error must leave the programmer accessible. Selecting a valid group and doing nothing further is a legitimate no-op. Invalid commands show local programmer feedback that clears when typing resumes.

| ID | Procedure | Expected and evidence |
| --- | --- | --- |
| ERR-01 | Trigger a recoverable pane/request failure in a disposable show; continue selecting and programming while observing captured DMX. | Local actionable feedback; no programmer removal or disruptive critical red surface. Output continues. Record screenshot and packet evidence, classify actual lost capability. |
| ERR-02 | Type an invalid programmer command, submit, then resume typing a valid command. | Error stays in programmer; clears on the next edit. Valid command works without dismissing a global error. |
| ERR-03 | Select a valid group without applying values; empty programmer and repeat. | Selection/no-op succeeds without error or unintended output change. |
| ERR-04 | In an isolated test session exercise actual output-service/show-loss failure and recovery. | Critical red indication accurately identifies lost capability, keeps recovery actionable, and clears once recovered. Do not simulate hazardous system conditions. |

## Run completion and initial execution index

Create the full case/variant ledger before testing. Include every row above, even if prerequisites block it. Finish with: tested and passed variants; failed cases/Defined issues; blockers and required equipment/decisions; untouched cases; license inventory; gallery index; saved test-show revision; settings restored; human sign-off still needed. Link candidate retest and supporting automated checks. No marketing readiness or public-release recommendation while required gates remain blocked.

Initial run: `2026-10-08-initial`, evidence under canonical `visual-inspection/release-acceptance/2026-10-08-initial/`. This run begins with environment/UI reconnaissance; consult its `results.md` for actual observations. No behavior is marked passed solely from this plan or historical tests.

## Regression gates discovered during the first acceptance run

Repeat these on every applicable rebuilt candidate. Keep seeded compatibility inputs distinct from objects authored through the current UI. Preserve failed captures; add new step IDs for reruns.

| ID | UI procedure | Expected and evidence |
| --- | --- | --- |
| SET-09 | Import an MVR containing genuine immutable profile UUID/revision collisions. Inspect unchecked default, explicitly choose copy, create a separate show, save/reload. | Apply requires explicit consent; original library/show profiles remain unchanged. New identities preserve modes/placement and retain source/calibration revalidation warnings. XML names decode once, including ampersands, quotes and Unicode. Compare all fixture counts and copied bindings. |
| PRG-10 | Load a prepared legacy derived Aim preset, recall it, record/run a linked cue, move its target, then Clear without GO. Separately convert the same source through supported modern RECORD AT FIXTURE, save/reopen. | Running cue follows the derived source and target geometry; source identity remains stable. No false missing-literal warning. Capture documented pan/tilt slots and account for equivalent continuous aim solutions; nominal geometry does not prove physical calibration. |
| CAD-09 | Load a supported legacy show copy, select Back Truss Segment1 through Elements, enter X−3 then−2.5, Undo/Redo. Repeat rotation, ordered multi-selection, explicit copy pose, gizmo interleaving and reopen. | Loading completes canonical migration before editing, retains recoverable original backup, refuses unsupported/corrupt inputs safely. Each successful numeric Enter is one atomic exact-pose Undo step; metadata/bindings remain intact. Failed/no-op edits do not corrupt history. |
| PB-10 | Reassign an already used physical playback to another cue list without GO or reload between assignments; then GO, Clear, Stop. Repeat virtual/OSC/hardware paths where supported. | The playback resolves the newly assigned source immediately. Clear retains cue output; exact-owner Stop releases it and restores underlying values. No stale address or unrelated owner stop. |
