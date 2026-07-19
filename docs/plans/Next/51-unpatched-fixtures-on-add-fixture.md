# Unpatched Fixtures on Add Fixture

## Status

**Specification only.** This plan records a Show Patch setup improvement. It does not implement runtime behavior, persistence changes, UI changes, or executable tests.

## Goal

Let an operator add a fixture from **Add Fixture** without assigning a DMX address by explicitly selecting an **Empty** address. The fixture remains part of the show immediately, can be selected and programmed, can be stored in Groups, Presets, Cues, and Stage layouts, and simply produces no DMX output until it is patched later.

This must use the same unpatched-fixture semantics already expected elsewhere in the desk: unpatched means "no output address", not "removed from the show".

## Operator workflow

In the Add Fixture placement step, the address control must offer **Empty** as a deliberate address choice beside concrete patch addresses. Selecting **Empty** clears the universe/address assignment for the new fixture and keeps the rest of the Add Fixture flow available: fixture number, label, layer, mode, quantity, physical location, and other non-DMX setup fields remain normal.

The placement summary and resulting Show Patch row must make the fixture's state obvious. The address column should show **Empty** or the existing unpatched label rather than a misleading address, disabled row, or error state.

Changing from **Empty** to a real address before confirming uses the normal footprint validation. Changing from a real address back to **Empty** releases any reserved footprint in the preview before the fixture is added.

## Behavior contract

Adding an unpatched fixture creates the same fixture object and stable fixture identity as adding a patched fixture, except its DMX placement is absent. It must:

- allocate or accept the requested fixture number normally;
- preserve the selected fixture profile, mode, layer, name, geometry, and logical-head information;
- include the fixture in selection, Fixture Sheet, Stage, Groups, Presets, Cues, Highlight stepping, and value spreading;
- suppress only DMX output until a later patch address is assigned;
- remain visible in Show Patch with an actionable way to patch it later; and
- persist and reload without inventing a default universe or address.

Bulk add must support **Empty** consistently. If multiple fixtures are added with **Empty**, each fixture is created unpatched rather than auto-advancing addresses or failing footprint validation.

## Surface and compatibility requirements

The Add Fixture UI, any command/API path that shares the fixture-add operation, and imported-show conflict resolution should use compatible vocabulary for an intentionally unpatched fixture. Existing shows with unpatched fixtures must continue to load unchanged.

Implementation must keep optional address data explicit across Rust and TypeScript boundaries. Empty placement is valid input, while malformed concrete addresses, collisions, and out-of-range footprints remain validation errors.

## Acceptance coverage

1. Add Fixture can create one fixture with address **Empty**.
2. The new row appears in Show Patch with the selected profile/mode and an unpatched address label.
3. The unpatched fixture can be selected, programmed, recorded into a Group, recalled from that Group, and shown in Fixture Sheet and Stage.
4. DMX output excludes the unpatched fixture until it is patched later.
5. Repatching the fixture to a valid concrete address starts output without changing the fixture identity or stored programming.
6. Switching between **Empty** and concrete addresses in the Add Fixture flow releases and reacquires footprint preview reservations correctly.
7. Bulk add with **Empty** creates every requested fixture as unpatched.
8. Save/reload preserves the empty address exactly and does not assign a default universe/address.
9. API or command-line fixture creation rejects malformed addresses but accepts the same explicit empty/unpatched placement used by the UI.
