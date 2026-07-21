---
slug: engine
title: Engine & Output
summary: "The real-time half: contributions, arbitration, fixture projection, DMX frames, and the timing loop."
order: 50
---

# Engine and Output

`crates/engine/`, `crates/output/`, `crates/playback/`, `crates/programmer/`, `crates/core/`,
`crates/fixture/`, `crates/control/`, and `crates/server/src/runtime/output_scheduler.rs`.

This is the real-time half of the system.

## Pipeline

```
Programmer values ┐
Playback cues     ├→ contributions → arbitration (HTP/LTP/ownership)
Preload           │      ↓
future dynamics   ┘   transitions (fade, delay, MIB, masters) + Highlight overlay
                         ↓
                  resolved semantic fixture values
                         ↓
                  fixture projection (mode, channels, fine bytes, splits, multipatch)
                         ↓
                  DMX frames  ─────→ Art-Net / sACN delivery
                  external intents ─→ device adapter seam
```

## crates/core

`AttributeKey`, `AttributeValue`, `AttributeDescriptor`, `MergeMode`, `TimedValue`, `Xyz`,
`ATTRIBUTE_REGISTRY`, the clock family (`ApplicationClock`, `SharedClock`, `SystemClock`,
`ManualClock`, `EngineClock`), and the newtype IDs.

## crates/engine

27 top-level modules. The ones that matter most:

| File | Role |
| --- | --- |
| `engine.rs` | The `Engine`. Every mutating method takes `&self` — interior mutability via `ArcSwap`, atomics, and `parking_lot` locks |
| `runtime_generation.rs` | The immutable, all-`Arc` generation swapped on show install |
| `lifecycle.rs` | The prepare/install typestate: prepare is fallible and side-effect free, install consumes and cannot fail |
| `render.rs`, `resolution.rs` | Deterministic render and arbitration |
| `contribution*.rs` | `ContributionBatch` — immutable samples for one render instant |
| `move_in_black*.rs`, `safety.rs`, `visualization.rs` | MIB, safety limits, stage preview |

The engine receives immutable compiled-show and contribution snapshots. It does not own animation
state, does not read the fixture library, and does not write persistence. CI bans
`pub fn playback(` from `crates/engine/src`.

## crates/playback

Cue models, tracking, live runtime, automatic transitions, phasers, HTP/LTP arbitration.
`PlaybackEngine`, `resolve`, `PlaybackMutation`, `PlaybackRuntimeEffect`,
`AutomaticPlaybackTransition`, `PlaybackTickResult`.

`PlaybackRuntimeEffect` carries the domain-owned `None` / `Transient` / `Durable` distinction that
makes no-change a first-class result.

## crates/programmer

User-scoped selection and Programmer state, shared across a user's sessions. `ProgrammerRegistry`
(clone shares state through 14 `Arc<RwLock<…>>` fields), `ProgrammerState`, `CommandLineState`,
`GroupDefinition`, `resolve_group`, `HighlightRegistry`, `ProgrammerCaptureMode`.

## crates/output

`DmxFrame` and `DMX_SLOTS`, `OutputRoute`, `Protocol`, `DeliveryMode`, `OutputHealth`,
`run_scheduler` and `run_scheduler_dynamic`, `ArtNetDriver`, `SacnDriver`, `NetworkOutput`, and the
`ExternalDeviceAdapter` seam.

`OutputDriver` is an `#[async_trait]` object with a default `terminate` implemented via `send`.

## crates/control

Normalized control input: MIDI, RTP-MIDI, OSC, Art-Net, timecode. `ControlInput` is an async trait.
The `native-midi` feature is on by default and pulls in `midir`; portable Linux builds omit it
because it depends on the target machine's ALSA library.

## crates/fixture

Profiles, modes, channels, packages, the desk library, patch models and validation, colour
calibration, DMX encoding. `PatchedFixtureCompiler<R>`, `FixtureProfileRevisionResolver`,
`PortablePatchedFixtureRecord`.

## Timing loop

`crates/server/src/runtime/output_scheduler.rs`. Each tick: render, leave the domain locks, publish
automatic semantic transitions, send encoded routes. Publishing after releasing the locks keeps a
slow subscriber from stalling a frame.

## Performance contract

| Level | Requirement |
| --- | --- |
| Hard floor | 32 fully packed universes at 100 Hz |
| Target | 64 fully packed universes at 120 Hz |
| Low power | 4–8 universes at 40 Hz on Pi-class hardware |

Benchmark: `crates/server/src/bin/light-benchmark.rs`, release builds only, documented hardware,
reporting p50/p95/p99, dropped ticks, CPU, allocation rate, and per-stage time split.

Never on the tick: full-show cloning, broad mutex contention, JSON serialization, frontend
projection work, fixture-library reads, persistence, blocking adapters.

## Read first

1. `crates/core/src/attributes.rs`
2. `crates/core/src/clock.rs`
3. `crates/engine/src/runtime_generation.rs`
4. `crates/engine/src/engine.rs`
5. `crates/engine/src/lifecycle.rs`
6. `crates/engine/src/contribution_batch.rs`
7. `crates/playback/src/arbitration.rs`
8. `crates/output/src/scheduler.rs` and `delivery/driver.rs`
9. `crates/server/src/runtime/output_scheduler.rs`

Most of the interesting Rust in this repository is in these crates — see the Rust by Example tour.
