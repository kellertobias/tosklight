# Acceptance Criteria

Every feature that changes persisted shows, desk state, fixture definitions, layouts, configuration, or other user files must address backward compatibility before it is complete.

- Existing valid files from supported earlier versions must continue to load, using an explicit migration or backward-compatible reader where necessary.
- A change that cannot safely infer a migration must stop and ask whether old files need to remain supported before the persisted schema is changed.
- Migration behavior must have a regression test containing representative legacy data.
- A failed file migration or invalid active show must not prevent the application from starting.
- Recovery errors must be visible and actionable. The application must preserve the original file and offer creation of a separate empty show instead of silently overwriting or deleting data.
- New-file initialization and successful migration must both be verified through the real server startup path.
