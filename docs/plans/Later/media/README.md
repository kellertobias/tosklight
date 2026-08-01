# Media Server

## Status and intent

**Later — specification only.** This folder records the planned ToskLight Media Server rebuild and the Light Desk Media pane work. It does not implement a Media window, protocol changes, media fixtures, programmer mutations, ToskLight Media Core, renderer behavior, library administration, text-overlay services, persistence, help changes, or executable tests.

The existing C++/openFrameworks Media application remains the behavior reference where its behavior is intentional. The target implementation lives in `/Users/keller/repos/light` as a Rust/React workspace with a separate Light Desk product, a separate Media Server product, and narrowly owned shared crates/packages.

## Implementation order

1. [Media Pane Dummy UI](01-media-pane-dummy-ui.md) - build the Light Desk Media pane with dummy data first.
2. [Media Server Application Behavior](02-media-server-application-behavior.md) and [Media Server Rust Architecture](02b-media-server-rust-architecture.md) - rebuild the actual Media Server product.
3. [Media Pane Live Data and Workflows](03-media-pane-live-data-and-workflows.md) - connect the pane to real server data, fixture selection, previews, and media folder/file workflows.
4. [Media Pane ToskLight Server Page](04-media-pane-tosklight-server-page.md) - add the ToskLight-specific Media pane page for connecting to and managing the native Media Server integration.

The chunks are ordered deliberately. The first chunk is UI-only with dummy data. It must not require the Media Server rebuild or real CITP/native protocol implementation. The third and fourth chunks must not fake server behavior once they are marked implementable.

## Product boundary

The Media Server is a real-time media server controlled by lighting protocols and by its administration UI. The Light Desk Media pane is the operator surface inside ToskLight for a patched media-server fixture and its logical layer heads.

The Media pane is not the Media Server administration UI. It is a fast programming and monitoring surface for touch desk operation. Uploading, transcoding, reindexing, renderer administration, and native generated-source management belong to the Media Server product or to the explicit ToskLight Server page, not to the generic pane workflow.

## Cross-Repository Boundary

The migration from `/Users/keller/repos/media` is one-way. The target repository must not use Cargo path dependencies, npm links, runtime file lookups, Git submodules, or build steps that reach back into Media. Code, tests, fixtures, and assets are transferred into owned target locations with provenance and licensing recorded.

Before implementation begins, record the exact Light commit that is the approved Media integration baseline, create a dedicated Media integration branch/worktree, and keep the active Light refactor checkout untouched.
