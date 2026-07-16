# Update with Shift+Record

## Status and sequencing

This is a deferred feature. Implement it after the other recordable objects and their storage and merge behavior are complete. Until then, storing again with Merge provides the closest equivalent workflow.

Do not add an acceptance test or Playwright coverage for Update yet. Test coverage should be designed only after the supported update targets and their final storage semantics exist.

## Operator workflow

- Pressing `[SHIFT] [REC]` invokes **Update**.
- Invoking Update opens a modal. It must not immediately change the show.
- The modal shows every item in the current context that can validly be updated, including the available target and the values or attributes eligible for that target.
- The operator can choose which eligible items to update before confirming.
- Confirming applies the selected changes using the same underlying storage and merge semantics as the corresponding recordable object.
- Cancelling closes the modal without changing anything.
- If nothing in the current context can be updated, the modal explains that there are no eligible updates and does not offer a destructive confirmation.

## Design intent

Update is a guided shortcut for applying current changes to existing stored data. It must reuse the established storage model rather than introduce a separate kind of stored data. The modal exists to make the target and scope explicit before any existing data is changed.

Once the other recording features are complete, define the precise list of supported targets, the target-selection rules, and the tracking implications before implementing Update or adding its tests.
