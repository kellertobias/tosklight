# Acceptance Criteria

Until ToskLight reaches v1 (which will be stated in this document), a feature may
deliberately break persisted-show compatibility. Such a break must be called out
explicitly in the feature plan and result, and every repository-owned demo,
benchmark, test, and example show affected by it must be regenerated.

For a change that does not explicitly declare such a pre-v1 break:

- Existing valid files from supported earlier versions must continue to load, using an explicit migration or backward-compatible reader where necessary.
- A change that cannot safely infer a migration must stop and ask whether old files need to remain supported before the persisted schema is changed.
- Migration behavior must have a regression test containing representative legacy data.
- A failed file migration or invalid active show must not prevent the application from starting.
- Recovery errors must be visible and actionable. The application must preserve the original file and offer creation of a separate empty show instead of silently overwriting or deleting data.
- New-file initialization and successful migration must both be verified through the real server startup path.

A clean-install happy path is never sufficient evidence for a persisted-data
change: verify either the declared pre-v1 break and regenerated owned data, or
the applicable compatibility and recovery path above.
