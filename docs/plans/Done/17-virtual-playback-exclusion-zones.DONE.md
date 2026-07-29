# Virtual Playback Exclusion Zones

> **Superseded current contract (2026-07-29):** This file records the former
> page-slot implementation. The authoritative replacement is
> [Dedicated Virtual Playbacks and Exclusion Zones](../refactoring/doing/18-virtual-playbacks-and-exclusion-zones.md):
> zones are show-owned, store stable Virtual Playback numbers, and apply across desks
> and pane layouts. Pane or desk removal does not remove them; only explicit zone
> deletion does.

Named exclusion zones let an operator define a set of virtual playback cells in which at most one playback may be On at a time.

**Implementation status:** Complete for the current page-slot-mirroring Virtual
Playback model. The operator UI, authoritative server behavior, persistence, restart
recovery, help, and Playwright coverage described here are implemented. The semantic
contract is `VPB-007` in
`tests/56-semantic-virtual-playback-exclusion-zone.spec.ts`; the older
`tests/06-preload-modes-and-virtual-playbacks.spec.ts` remains supplemental
compatibility coverage.

The future dedicated Virtual Playback range and Follow Main/Pinned page modes are
specified in the final refactoring queue item,
[Dedicated Virtual Playbacks and Exclusion Zones](../refactoring/doing/18-virtual-playbacks-and-exclusion-zones.md).
That pre-alpha schema break deliberately does not migrate this old model; it
regenerates the repository-owned example shows instead.
This completed document records shipped behavior and must not be used to claim that
the dedicated-number model is implemented.

## Creating a zone

The operator holds Shift and selects multiple virtual playback cells, then chooses **Create Exclusion Zone**. The new zone receives an editable name. Creating a zone must not execute, stop, or otherwise change any selected playback.

Virtual Playback settings show the configured exclusion zones as a named list. From that list the operator can rename a zone, inspect its member cells, change its membership, or delete the zone without deleting the assigned playbacks or their source objects.

## Runtime behavior

When a playback in an exclusion zone turns On, every other playback in that zone turns Off. The newly activated playback wins even when another member was already active. Turning the active member Off does not automatically activate another member.

The zone applies to every control path that operates one of its virtual playbacks, including touch, mouse, keyboard/command workflows, OSC, and restored authoritative playback state. It must not be implemented as a browser-local visual toggle.

An exclusion zone is separate from the playback option that automatically turns a playback Off when all of its output is overridden. Automatic full-override release and mutual exclusion must remain independently configurable and testable.

## Persistence and implemented decisions

Zone names, membership, and ordering persist with the virtual-playback surface. Loading an older layout with no zones produces an empty zone list without changing cell assignments.

- A zone stores one-based cell positions, not copied playback IDs. It applies those positions to the control desk's current playback page. Changing page therefore keeps the same cell pattern and applies it to the playbacks currently assigned to those positions. Explicit-page actions against a page that is not current do not borrow the current page's virtual zone.
- Zone configuration is stored as one show-level snapshot partitioned by control desk and virtual-playback surface ID. Multiple sessions attached to the same control desk use the same partition. A different control desk used by the same user has independent zones, just as it has independent button and page interaction state; the user's programmer values remain shared separately.
- Moving a pane retains its surface ID and zones. Shrinking its grid does not delete out-of-range memberships: they remain visible as retained hidden cells in Settings and become active again if the grid expands. Deleting a pane does not delete playback assignments or Cuelists.
- One cell may participate in multiple zones. Activating that cell releases the union of all other members in every containing zone. Zone list order is persisted for display but does not change this union rule.
- All playback activation requests pass through one server-side serialized action boundary. The last activation processed by that boundary wins; a simultaneous pair can never leave two members On.
- Creating, renaming, editing, or deleting a zone is configuration-only and never operates a playback. If several new members are already On when the zone is created, they stay On until the next activation or restart.
- Restored authoritative state is normalized before output resumes. The member with the latest activation timestamp wins; an exact timestamp tie is resolved by the higher playback number so restart behavior is deterministic.
- Mutual exclusion is independent of automatic full-override release. Either playback option can be enabled or disabled without changing the other rule.
