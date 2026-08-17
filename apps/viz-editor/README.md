# ToskLight Viz Editor

The rig-planning window for a standalone visualizer session: the desk's patch sheet over a show
file, with no desk running.

```sh
npm run open:viz-editor
```

This opens the latest existing build. Use `npm run build:viz-editor:open` to rebuild it first.

## What it is for

The visualizer takes its live values from Art-Net or sACN sent by whatever console is driving the
rig — frequently not ToskLight. What it cannot get from the network is the rig itself: which
fixtures exist, what they are, where they stand, and how they are addressed. This window is where
that is decided, and the visualizer draws it.

So this is not a second desk. There is no encoder frame, no command line, no playbacks, no
programmer and no built-in renderer — the visualizer window is the picture. What is here is the
patch sheet, unchanged, and the files it reads and writes.

## What it shares, and what it does not

The patch sheet is `@tosklight/patch`, the same implementation the desk uses: addressing,
conflicts, splits, placement, layers and multi-patch behave identically because they are the same
code. Patch layers are stored in the document exactly as the desk stores them, so a show written
here opens there with its layers, and a show written there opens here with them rather than with
one invented default its fixtures do not belong to. It is composed here with a host that has no programmer selection and no `Set` key, so
selecting a row moves only this sheet's cursor and every cell is directly editable.

Underneath, `viz-document` runs the desk's own `ShowPatchService` against a show file with no
runtime attached. Nothing here starts a server, joins a desk session, or outputs DMX.

## Files

| Action | Result |
| --- | --- |
| **New** / **Open** | A `.show` file. The desk opens the same file directly — there is no separate planning format and no conversion. |
| **Save As** | A complete copy, including the fixture profile revisions it uses, so it reopens on a machine without the same fixture library. |
| **Import MVR** | Reads an MVR rig from another application through the desk's own import service. Nothing is written until the archive has been read and shown: how many fixtures and scenery objects it holds, which GDTF types no profile matches, and which addresses are already taken. Only the fixtures that need a decision are asked about — import unpatched, choose an address, skip, or replace — and a fixture nothing is wrong with is never a question. |
| **Export MVR** | Writes the rig as MVR. A profile with no retained source GDTF is referenced rather than embedded, and the count is reported. |

Every file action reports what it did; none of them happen silently.

## Reopening

The window reopens the show it had open last time, so a visualizer that launched it has a picture
immediately. A show that has since been moved or deleted is forgotten rather than reported as a
failure — finishing with a show is not an error. The path is kept in the operator's own
configuration directory, not in the repository or in any show.

## The fixture library

The fixture browser lists the transferable profiles from the packaged fixture library, or from
`LIGHT_FIXTURE_LIBRARY` when it is set. With no library available the browser is empty and an
existing show can still be edited, because a saved show carries the profile revisions it uses.

## Layout

- `src/document/transport.ts` — the patch authority over Tauri commands. One window over one file
  is the only writer, so the mutation outcome is the whole truth and there is no event stream.
- `src/document/session.ts` — file, library and MVR commands.
- `src-tauri/src/contract.rs` — the shape `@tosklight/patch` consumes.
- `src-tauri/src/session.rs` — the open document and its commands.
- `src-tauri/src/recent.rs` — the show this window had open last time.

## The icon

ToskLight Architect uses the approved `assets/branding/ToskLight Architect.svg` and `.png`
artwork. This application owns the generated platform set under `src-tauri/icons`; the visualizer
has no generator of its own and takes both its window mark and its macOS bundle icon from here.
See [`assets/branding/README.md`](../../assets/branding/README.md) to regenerate it.

## Serving the visualizer

Started with `--serve <address>`, the window also serves the open document to a visualizer on that
address, through [`crates/viz/planning`](../../crates/viz/planning). The visualizer picks the port
and passes it, because it is the one that has to connect. The window and the renderer read the
same document, so a fixture patched here is in the next snapshot the visualizer asks for.

That includes an event stream, on the same route a desk serves one. Every command that changes the
document announces it, and the renderer resynchronises within a frame or two. Without it the
renderer would have nothing to wait on: it would finish reading, reconnect on its retry interval,
and rebuild the scene and rebind its DMX receivers every couple of seconds for as long as this
window stayed open.
