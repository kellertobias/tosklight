---
slug: programmer
title: Programmer
summary: "The operator's scratchpad: ordered selection, semantic values, Preload, Highlight, and the shared command line."
order: 15
---

# Programmer

The operator's scratchpad. Values being set now live here, above any Playback, until they are
recorded into the show or cleared.

**The binding contract is `docs/help/30-Programmer/01-command-line.md`.** Read it as a
specification, not a description — it defines the command grammar and keypad layout, and both are
operator muscle memory. The rest of `docs/help/30-Programmer/` covers selection, value setting, and
programming cues.

## Ownership

Programmer state is **per user**, not per desk. Two users on one desk have independent selections
and values; one user on two desks shares them. Getting this backwards leaks one operator's work
into another's.

It is state lifetime #4 of six. It survives reconnect via a recovery checkpoint and is not part of
the portable show.

## What it holds

| | |
| --- | --- |
| Selection | The **ordered** set of fixtures and logical heads the next value change applies to |
| Values | Semantic attribute values under LTP, per fixture-or-head and attribute |
| Timing and modes | Fade and delay, capture mode |
| Preload | Values staged for the next GO, under a separate pending authority |
| History | Bounded undo/redo |

Selection order drives value spreading and reflects operator intent — preserve it. Selecting a
group expands to its resolved logical heads, not to a parent fixture.

## The command line

One desk has one shared command line, visible identically on the app and on attached hardware. UI
keypad, physical keys, OSC keys, and the HTTP command API feed the same parser and the same state.

Partial unexecuted text is desk-interaction state (lifetime #3): shared across the desk's surfaces,
never persisted as programming.

Cue ambiguity is runtime-only state, set solely by a `ChoiceRequired` execution after ENTER and
cleared by edits, reset, Cancel, an accepted choice, or show replacement. One desk shares one
authoritative choice across UI, OSC, and WebSocket; another desk stays isolated.

HTTP surface:

```
GET  /api/v2/desks/{desk_id}/command-line          text, target, pristine state, revision, pending choice
PUT  /api/v2/desks/{desk_id}/command-line          replace the shared line, requires If-Match
POST /api/v2/desks/{desk_id}/command-line/keys     one logical key, press or release phase
POST /api/v2/desks/{desk_id}/command-line/execute  execute atomically, return typed outcome
```

## Rules that break things when ignored

**Programmer LTP is not Playback HTP.** Programmer values use Latest Takes Precedence. Playback
arbitration uses HTP for intensity, LTP for other attributes, plus ownership. Generalising Playback
HTP into the Programmer is a recurring bug class. See
`docs/help/40-Running-a-Show/02-htp-ltp-and-ownership.md`.

**Highlight is never recorded.** It is a transient overlay on live output. It must not reach
Programmer values or cue data.

**Preload has its own authority.** Normal and pending-Preload values are separate projections with
separate revisions. A normal write must not cross into an active Preload capture; an atomic
revision precondition enforces this. Normal and pending views are mutually exclusive in the UI.

**Groups and Presets are show objects.** The Programmer records into them and recalls from them; it
does not own them. A stored empty group differs from an absent group, and a missing ID in a range
is skipped rather than materialised.

**Unpatched fixtures are selectable and programmable.** Only DMX output is suppressed.

## Where it lives

| Concern | Path |
| --- | --- |
| Programmer state, selection, values, history, command state | `crates/programmer/src/` |
| Command-line parsing | `crates/programmer/src/command_line/` |
| Group resolution | `crates/programmer/src/groups.rs` |
| Highlight registry | `crates/programmer/src/highlight/` |
| Typed use cases — the boundary every surface crosses | `crates/application/src/programming/` |
| Command-line HTTP adapter | `crates/server/src/command_http/` |
| Command line and keypad UI | `apps/control-ui/src/components/control/` |
| Shared keypad model | `apps/shared/programmerKeypad.ts` |
| Scoped frontend stores | `apps/control-ui/src/features/programmingInteraction/`, `programmerValues/`, `programmerPreloadValues/`, `programmerCaptureMode/`, `programmerLifecycle/` |

`crates/application/src/programming/` is one authenticated, ordered boundary. It serializes typed
commands per desk, so a UI keypress, an OSC tap, and an HTTP request cannot interleave into an
incoherent command line. Adapter-owned interactions cross the same gate.

## Testing

`tests/support/operator/programmer.ts` drives one operator intent through three real surfaces:

```ts
{ via: "command-line", api }   // HTTP
{ via: "software", page }      // DOM clicks on the keypad
{ via: "osc", api, hardware }  // UDP OSC, explicit true/false phases
```

`programmerKeysForCommand("GROUP 3 AT 50")` parses operator text into logical keys. `GROUP` maps to
`[GRP]`, `GROUP GROUP` to DEGRP, `THRU` to `[TRU]`. Unknown tokens throw rather than silently doing
nothing.

Use `pairedScenario` so the same assertion runs against both the API and the UI.

## Read first

1. `docs/help/30-Programmer/01-command-line.md`
2. `crates/programmer/src/lib.rs`
3. `crates/programmer/src/command_line/`
4. `crates/application/src/programming/`
5. `apps/shared/programmerKeypad.ts`
6. `tests/support/operator/programmer.ts`
