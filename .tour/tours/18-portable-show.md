---
slug: portable-show
title: "The Portable Show: Load, Migrate, Revise, Compile, Save"
components: [backend, engine]
order: 28
---

# The Portable Show: Load, Migrate, Revise, Compile, Save

Operator contract: `docs/help/20-Show-Setup/02-shows-revisions-and-mvr.md` and
`docs/help/20-Show-Setup/01-fixtures-and-patch.md`. SHOW-000 through SHOW-005 and FIXTURE-001 are
implemented in `tests/00-generate-show-files.spec.ts` and the split
`tests/05-virtual-time-persistence-and-recovery.*.ts` files.

The portable `.show` file is production content. `desk.sqlite` is installation state. Save As
copies the first and never the second.

## Raw authority and typed edits

`crates/shared/show/src/portable/document.rs` retains raw object bodies, unknown fields, metadata,
profile revisions, and portable revisions. Typed capability code computes an owned delta and merges
it into the raw body; serde is never allowed to erase data it does not understand.

`crates/shared/show/src/portable/transaction.rs` provides atomic CAS writes and undo rows. The server keeps
an in-memory document for the active show, while each accepted mutation retains one ordered SQLite
WAL commit. Recovery checkpoints are interval-gated.

## Load and migrate

`crates/light/src/show_compiler/prepare.rs` creates a candidate, runs owned migrations and
validation, resolves immutable fixture-profile snapshots, and builds an `EngineSnapshot`.
Malformed active data enters recovery without overwriting the original file.

Migration write-backs are explicit transaction riders. They publish typed object/route events when
they ride a mutation; opening a show instead replaces the complete authority and rehydrates clients.

## Commit and install

`crates/light/src/active_show/service.rs` owns the sequence:

1. begin from the current in-memory document and expected revision;
2. apply typed changes to a lossless candidate;
3. validate and prepare the new runtime;
4. create the required recovery checkpoint;
5. commit the portable transaction;
6. install the prepared runtime generation;
7. reconcile adapters, audit, and publish.

Preparation is fallible and side-effect free. Installation consumes the prepared value and cannot
fail, so persistence cannot get ahead of the engine.

## Save As, revisions, and selective import

Save As copies the complete portable file and gives the copy an independent show identity. Named
revisions remain durable copies. Selective import under
`crates/light/src/selective_import/` plans dependency closure and identity rewrites, then
applies one active-show revision; it never becomes a feature-specific copy route.

## Runtime generations

The compiler produces an immutable generation. Chunk 26 added dirty-subgraph compilation, so a Cue
edit can share untouched fixtures, Groups, Presets, and other projections while remaining
equivalent to a full compile.

## Exercise

Read `crates/light/src/active_show/tests_migration_riders.rs`. List the event order for a
requested object change that also migrates a Cue and an output route, then compare it with the
asserted order.
