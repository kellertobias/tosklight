---
slug: testbench
title: Testbench
summary: "Layered verification from cargo/vitest up to paired Playwright scenarios driven through real operator surfaces."
order: 70
---

# Testbench

Tests act through the same public surfaces an operator uses: visible UI, exact OSC, the command-line
HTTP API, or explicit deterministic bench controls. Not implementation objects.

Authorities: `docs/testing/README.md` for the acceptance contract and notation,
`docs/engineering/test-map.md` for which boundary proves what.

## Layers

| Layer | Where | Command |
| --- | --- | --- |
| Rust unit and integration | each crate's `tests/` or feature-local modules | `./test unit` |
| TS unit and component | `apps/control-ui/src/**/*.test.ts(x)`, jsdom and Testing Library | `./test unit` |
| Type and build gate | `tsc --noEmit && vite build` | `./test unit` |
| Architecture | `tools/check-architecture.mjs`, `tools/check-source-size.mjs` | `./test architecture` |
| e2e-api `@api` | root `tests/`, no browser | `./test e2e-api` |
| e2e-ui `@ui` | root `tests/`, real Chrome | `./test e2e-ui` |
| e2e-supplemental | `@osc`, `@wire`, `@restart`, `@desktop`, `@bench` | `./test e2e-supplemental` |
| desktop-smoke | `tests/05-desktop-process-integration.spec.ts` | `./test desktop-smoke` (macOS) |

Start with the smallest relevant check and widen by risk.

## pairedScenario

`apps/control-ui/e2e/bench/pairedScenario.ts`:

```ts
pairedScenario({ id, title, arrange, api, ui, assert })
```

Registers `"<ID> @api › …"` and `"<ID> @ui › …"` with the same arrangement and the same assert
oracle, each on its own fresh show. That is how surface parity is proven rather than assumed.
`docs/testing/README.md` mandates it over a lone `test(...)`.

## The bench

`apps/control-ui/e2e/bench/`:

| File | Provides |
| --- | --- |
| `lightBench.ts` | Per-worker temp data dir, free TCP/UDP port allocation, spawns `light-server`, `restart()`, graceful shutdown via `POST /api/v1/shutdown`, abrupt SIGKILL, Art-Net and sACN receivers, OSC hardware factory, virtual clock cursor, `createTwelveDimmerShow()`, failure artifacts |
| `api.ts` | `ApiDriver`: login, revision and ETag validation, `getCommandLine`, `replaceCommandLine`, `sendCommandKey`, `executeCommandLine`, typed `command<T>()` over the versioned command WebSocket |
| `desk.ts` | `DeskDriver`: browser desk facade. `open(baseUrl)` waits out the connection cover and banner and pins the desk alias. Also the recording and narration overlay |
| `protocols.ts` | `DmxReceiver` (`bind()`, `nextAfter(mark, "artnet"\|"sacn", universe)`), `OscHardware` (`connect`, `subscribe`, `send`, `mark()`, `expectAfter`), `encodeOscMessage` |
| `fixtures.ts` | Playwright `test.extend` providing `bench`, `baseURL`, `show`, `api`, `desk` |

Three determinism rules from `docs/testing/README.md`:

- **Canonical shows.** `tests/fixtures/compact-rig.show` and `tests/fixtures/default-stage.show`,
  loaded through `loadCanonicalCopy(...)`, which resets the virtual clock and DMX receivers and
  produces an isolated working copy. Every scenario starts from one.
- **Virtual time**, never `sleep`.
- **Receiver marks.** `mark()` then `nextAfter(mark, …)`, so you assert on the frame that followed
  your action rather than whatever was in flight.

## Operator DSL

`tests/support/operator/programmer.ts` drives one intent through three surfaces:

```ts
type ProgrammerSurface =
  | { via: "command-line"; api }
  | { via: "software"; page }
  | { via: "osc"; api; hardware }
```

- `programmerKeysForCommand("GROUP 3 AT 50")` parses operator text into logical `SoftwareKey`s.
  Numeric tokens explode into digits; `TOKEN_ALIASES` maps `GROUP→[GRP]`, `DEGRP→[GRP,GRP]`,
  `THRU→[TRU]`, `RECORD→[REC]`. Unknown tokens throw.
- `executeProgrammerCommand(surface, command, { reset, expectedCommandLine, expectedCompletion })`
  resets with ESC, types, then ENTER.
- The OSC surface is real OSC: `withOscProgrammer` owns the subscribe and unsubscribe lifecycle
  against `session.desk.osc_alias`, `tapOscKey` sends explicit `true` then `false` phases, and each
  phase waits for the command-line feedback message after a receiver mark. No sleeps. Action names
  come from `oscProgrammerActionForKey` in `apps/shared/programmerKeypad.ts`, the same mapping the
  app uses.

Also `groups.ts` (`storeGroup` via pool or programmer) and `patch.ts` (drives the real Fixture
Address dialog, typed against generated wire DTOs).

## Scenario markdown

`docs/testing/` holds acceptance contracts with stable IDs: the operator notation (`[KEY]`,
`[GRP][GRP]` for DEGRP, `[REC][+]`), the API/UI pairing rules, the DMX conversion table, and the
8-step execution template.

`docs/help/99-Development/02-test-bench-coverage.md` is the scenario-ID catalog.

When implementing one named scenario, keep coverage focused rather than expanding to every scenario.

## Architecture checks

`tools/check-architecture.mjs` runs five checks: Rust dependency directions, thin server entry point,
active-show mutation direction, closed Playback ownership, TypeScript wire-boundary imports.

`tools/check-source-size.mjs` enforces file ≤1200 and function ≤150 lines (goals 400 and 20). The
ratchet baseline is empty, so any new violation fails immediately. Tighten after reductions with
`node tools/check-source-size.mjs --ratchet`.

## Other commands

```sh
./test artifact-paths   # artifact path bindings across bash, node, python
./test app-icons        # Tauri icon completeness
./test record           # serial narrated video of the catalog
./test demo             # product walkthrough; refreshes assets/demo.show
./test e2e tests/<focused-spec>.spec.ts
```

CI: `.github/workflows/test.yml` — `unit` on ubuntu, sharded `e2e` matrix over api/ui/supplemental,
`desktop-smoke` on macos-14. Manual and release CI run on Forgejo.

`docs/engineering/build-and-test-commands.md` covers every subcommand and what `./test architecture`
enforces.

## Read first

1. `docs/testing/README.md`
2. `apps/control-ui/e2e/bench/pairedScenario.ts`
3. `apps/control-ui/e2e/bench/fixtures.ts`
4. `apps/control-ui/e2e/bench/lightBench.ts`
5. `tests/support/operator/programmer.ts`
6. `tests/support/catalog.ts`
7. `playwright.config.ts`
8. `tools/check-architecture.mjs`
