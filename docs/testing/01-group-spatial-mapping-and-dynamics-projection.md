# Group Spatial Mapping and Dynamics Projection

These scenarios verify the Plan 01 operator contract through authoritative Group, Dynamic,
Programmer, Playback, persistence, and output state. Use a fresh working copy of a representative
show for every scenario. Record show/object revisions, WebSocket events, and DMX packet marks before
each mutation. Stage positions use the desk's X/Y/Z convention; unpatched fixtures stay in the show
and selection but emit no DMX.

## GROUP-SPATIAL-001 — Ranked Group spread reaches authoritative output

1. Patch centre, middle-ring, and outer-ring dimmers, including two coincident fixtures.
2. Record their deliberate source order as Group 4 and open Group settings.
3. In **Projection**, create a local Top mapping. In **Phaser**, choose outward Radial and close with X.
4. Select Group 4 live and enter dimmer `0 THRU 100` through the normal Programmer path.
5. Advance virtual time and inspect Programmer projection plus newer Art-Net and sACN packets.

The centre rank receives 0, the outer rank 100, and each intermediate radius one shared value.
Coincident fixtures remain individually visible but receive an identical value in one mutation and
one Undo step. Group source data is unchanged by evaluation.

## GROUP-SPATIAL-002 — Live references inherit until locally overridden

1. Create a mapped source Group and a derived Group that references it.
2. Change source membership and mapping; verify both changes flow into the derived preview and rank-aware spread.
3. Copy the inherited mapping as local on the derived Group.
4. Change source membership and mapping again.

Membership still flows into the derived Group, while its local mapping remains unchanged. Removing
the local mapping reveals the source's current mapping, not the values copied earlier. Nested
references deduplicate by first occurrence; mixed source mappings visibly fall back to source order.

## GROUP-SPATIAL-003 — Dynamic overrides never mutate Group settings

1. Bind a Dynamic to the saved live Group and open its **Projection** tab.
2. Verify the Group/provenance label and authoritative ranked preview.
3. Override projection only, shape only, then both; return each stage to **Inherit** independently.
4. Choose Random and verify the explanation that positions and projection are ignored.
5. Change the current Programmer selection, Group membership, and Stage positions while editing.

Preview continues to use the Dynamic's saved target binding and retains the unsaved draft while
reporting source changes. One Apply produces one revisioned Dynamic update. Group settings never
change. A stale Apply reloads authority and retains the draft for deliberate reapplication without
silently retrying.

## GROUP-SPATIAL-004 — Missing, empty, frozen, and unpatched cases stay explicit

Verify an intentionally empty Group has zero ranks while an absent Group is an error. Verify missing
Stage positions appear last with individual ranks, unpatched members remain selectable/ranked, and
logical heads are not duplicated. Frozen and targetless Dynamics show **Selection order (no Group
mapping)**; incomplete overrides are disabled, while Random remains valid without a projection.

## GROUP-SPATIAL-005 — Retired Layout authority does not return

Open a migrated Desktop that formerly contained Layout beside other panes. The Desktop opens, keeps
the remaining panes, and exposes no Layout built-in, pane, selection grid, reorder gesture, or
hidden layout state. Verify Group and Dynamic Projection workflows in software-only and
hardware-connected desk layouts.

## GROUP-SPATIAL-006 — Group settings contain only their owned controls

Open Group 4 through right-click, touch-hold, direct SET-click, and `SET GROUP 4 ENTER`. Every route
opens the same X-only modal with exactly **General**, **Projection**, and **Phaser** tabs. General
contains only name, icon, and color. No Master, selection, membership replacement, or Undo action is
present. Plain click selects live; double quick press selects frozen and opens no settings.

## GROUP-SPATIAL-007 — Typed SET routes explicit sources and destinations

With unrelated fixtures and Group 4 selected, press SET then Playback 2; Playback Configuration
opens and no assignment changes. Press SET, choose Group 4, verify `SET GROUP 4`, then press physical
Playback 2 without Enter; that Playback becomes a Group target. Repeat with an explicit Virtual
Playback. Clear, Cancel, desk replacement, and show replacement discard a pending Group source.
Stale Group, Page, physical slot, or Virtual Playback revisions reject atomically.

## GROUP-SPATIAL-008 — Group Masters are Playback-owned and shared by Group ID

Assign two Playbacks to Group 4 and one to an overlapping Group 5. The two Group-4 controls read and
write one shared level; Group 5 remains an independent HTP contribution. Exercise level, Flash and
release, page changes, restart, reassignment, and physical-fader pickup. Clearing one Group-4
assignment leaves its master active; clearing the final assignment removes its contribution without
changing the Group. Record → Group → Override remains the only membership-replacement workflow.

## GROUP-SPATIAL-009 — Legacy shows migrate without hidden authority

Open representative legacy explicit, derived, empty, gridded, mapped-Dynamic, physical-Group-
Playback, and Virtual-Group-Playback data. Preserve the original recovery copy and verify output is
unchanged. Canonical saves contain no Group-owned `master` or `playback_fader`; initial legacy level
is preserved on the deterministically chosen Playback target, including 0. Malformed retired fields
are stripped tolerantly, while malformed still-authoritative data follows normal recovery behavior.
Reopen the migrated show and prove a second migration is byte-stable.
