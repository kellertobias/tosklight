# @tosklight/patch

The ToskLight patch sheet as a host-agnostic package: fixture browser, patch table, addressing,
conflicts, splits, placement, layers, multi-patch and the patch authority that drives them.

It exists so the lighting desk and the Viz planning application can be the same patch sheet
without either application depending on the other. Both patch the same shows against the same
fixture library; they differ only in what surrounds the sheet.

## What a host supplies

Two things, and nothing else.

**A transport.** `PatchTransport` is the port to whatever owns the patch — snapshot, mutation and
an event stream. The desk implements it over its HTTP/WS API; a planning application implements it
against its own document core. The package contains no transport of its own.

**A host.** `PatchHost` describes the surrounding product:

| Member | Desk | Planning application |
| --- | --- | --- |
| `library` | Desk fixture library and patch layers | The document's library and layers |
| `selection` | The shared programmer selection, so the patch sheet, fixture sheet and command line stay on the same fixtures | `noPatchSelection` — there is no programmer |
| `editArmed` / `setEditArmed` | Follows the one-shot `Set` key, so a stray tap cannot change an address mid-show | Always `true`; the no-op setter |

`PatchDiagnostics` is optional instrumentation for one patch mutation. A host that measures
nothing passes nothing.

```tsx
<PatchHostProvider value={host}>
	<PatchViewProvider showId={showId} definitions={definitions} transport={transport}>
		<FixturePatchSetup />
	</PatchViewProvider>
</PatchHostProvider>
```

A null `orderedFixtureIds` means the host has no live selection: selection writes are dropped
rather than queued against a selection nobody can see.

## Styling

`styles.css` carries patch layout and control geometry only. Window chrome belongs to the host
application, and each one themes the sheet with its own tokens. Rules scoped to the desk's window
shell and to its Stage preview overlay are deliberately absent.

## Status: not yet consumed by the desk

`apps/light-desktop` still contains its own copy of this code and is unchanged. This package was
built first so the Viz planning application can be developed against it without destabilizing the
desk; the desk switches over once that application works, and its copy is deleted then — not
maintained in parallel.

Until that switch-over:

- **The desk's copy is authoritative for desk behavior.** Fix desk bugs there, and mirror the fix
  here.
- **Test ownership is split deliberately.** `apps/light-desktop/src/features/patch/PatchFeature.test.ts`
  stays with the desk because it covers `HttpPatchTransport` and the wire decoders, which are desk
  adapters. This package covers the sheet, the host port and the patch state it owns
  (`npm run test:patch-package`).
- **The stylesheet has not been rendered in a host yet.** It was extracted from the desk's
  stylesheets by selector scope; expect to correct it when the first host mounts the sheet.
