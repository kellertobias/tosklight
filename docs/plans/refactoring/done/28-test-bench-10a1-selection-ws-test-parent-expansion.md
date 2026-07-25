# 10a1 — Selection WebSocket test parent-expansion isolation

## Outcome

Restore the server unit gate without changing production selection semantics.

## Problem

`selection_action_ws_uses_typed_service_and_request_identity` chooses
`snapshot.fixtures[0]` and assumes a Replace action returns exactly that one ID.
The current Default Stage ordering can place a parent fixture with logical heads
first, so the production selection service correctly expands it and the assertion
receives the child identities.

## Work

- Select an explicitly leaf fixture for the one-fixture request-identity contract,
  or assert the complete parent expansion if the parent behavior is intended.
- Keep the sibling action-union coverage intact.
- Run the focused server test and the full unit gate.

## Done gate

- The request-identity test is deterministic under current fixture ordering.
- Production fixture-parent expansion is unchanged.
- Focused and full unit gates pass.

## Result

Completed on 2026-07-25.

- The request-identity test now explicitly chooses a leaf fixture from the
  current Default Stage snapshot instead of assuming its first fixture has no
  logical heads.
- Production selection code and its intentional parent-to-head expansion are
  unchanged.
- Focused server test passed.
- Full unit gate passed, including 470 server tests (one intentional ignore) and
  2,007 Control UI tests across 283 files.
