# Media Server Integration and Rust Migration

## Status

**Specification only.** This plan records a future media-server migration and integration direction. It does not import the current C++ project, implement Rust code, change protocols, or modify the desk UI.

## Goal

Evaluate merging the dedicated media server project into this codebase after migrating it from C++ to Rust and aligning it with the shared ToskLight UI component and layout libraries.

The first implementation phase should be a faithful Rust migration and UI-framework alignment before the media server is deeply integrated with the desk.

## Protocol direction

CITP should remain supported for compatibility with other desks and for CITP-only testing against ToskLight.

In addition, define whether ToskLight should control the media server through a native protocol that avoids depending on CITP for first-party operation. A native protocol may allow tighter desk integration, richer status, and better control of media-server-specific concepts.

## Acceptance coverage

1. The existing media-server behavior is inventoried before migration.
2. A Rust migration plan preserves current playback, media, output, and testing behavior.
3. The media-server UI uses the shared component and layout libraries where applicable.
4. CITP compatibility remains available for third-party desk use and isolated testing.
5. Any native ToskLight media protocol is specified before replacing CITP in first-party workflows.
