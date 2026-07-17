# Fixture Address Screen

## Completion

Implemented. Set Address now opens one desk-sized Fixture Address screen with the selected fixture, complete mode footprint, current and pending address, every split assignment, an integrated number block, and the authoritative 512-slot universe map. The fixture's own ranges are excluded while moving it, all split drafts are validated together, and confirmation remains one atomic patch update. Clear explicitly unpatches the active split; Cancel, Close, and Escape make no change.

## Status and scope

Redesign the **Set Address / Fixture Address** screen so the operator can choose from authoritative available DMX addresses and enter an address with an on-screen number block in one place. Similar design than in the New Fixture screen.

## Operator experience

The screen shows the selected fixture or logical-head footprint, target universe, current address, split-patch allocations where applicable, and a live view of available contiguous ranges. Occupied, reserved, out-of-range, and available addresses must be visibly distinct. Availability is calculated from the authoritative current patch and the fixture mode's complete slot footprint, not just its first channel.

A desk-style number block is present directly in the screen. It supports universe/address entry, correction, confirmation, and cancellation without opening a second generic text-input dialog. Selecting or dragging to an available range populates the same pending value as number-block entry. Clearing the address remains the explicit unpatch gesture. Nothing changes until explicit confirmation.

Changing universe, mode, split, or fixture selection refreshes availability and revalidates the pending address. Concurrent patch changes or stale revisions must fail visibly and atomically. Unpatched fixtures remain show objects if the operator cancels or intentionally removes their address.

## Acceptance criteria

1. Available ranges account for complete contiguous and split footprints across universe boundaries; every split is visible and editable in the same screen.
2. The current fixture's own occupied slots are handled correctly during a move.
3. Touch selection and number-block entry produce the same validated pending address.
4. Invalid, overlapping, overflowing, stale, and unsupported split addresses cannot partially patch.
5. Confirmation updates the patch once; Cancel, close, and Escape leave it unchanged.
6. The full availability list and number block remain reachable at supported desk sizes.

## Verification

- Component coverage exercises full split editing through the integrated number block, the 512-slot map, own-slot exclusion, occupied-footprint rejection, one atomic update, and Escape cancellation.
- `FIXTURE-ADDRESS-001` opens the production Show Patch workflow, checks the integrated map and number block, verifies supported viewport bounds, and proves Escape preserves the address.
- Focused component tests, the production frontend build, full unit coverage, focused Playwright coverage, and the generated manual pass.
