# REST API

Control's HTTP API uses `/api/v2`. The desktop server normally listens at `http://127.0.0.1:5000`; Pixel has a separate API on its own server.

A client creates a session with `POST /api/v2/sessions` and uses the returned token as `Authorization: Bearer <session-token>`. A configured `LIGHT_DESK_TOKEN` adds the `x-light-desk-token` installation boundary. That header and the session token have separate purposes.

Use explicit snapshot reads to initialize a client. Keep authoritative state current through the corresponding event subscriptions, and send edits through the API's intent routes. The desk owns the shared Programmer; a connection does not create a private Programmer.

For a first connectivity check, request `GET /api/v2/readiness`. Supply the installation token when configured. A healthy readiness response confirms the service is ready, not that physical lighting output is correct.

The [HTTP and WebSocket contract rules](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/api-rules.md) define reads, intent mutations, subscriptions, errors, and compatibility. The [route entry point](https://github.com/kellertobias/tosklight/blob/main/crates/light/adapters/headless/src/runtime/http_router.rs) and [wire types](https://github.com/kellertobias/tosklight/tree/main/crates/light/contracts/wire/src/v2) are the source references for individual interfaces.

Pixel-specific contracts cover [Speed Groups](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/protocols/media-speed-groups.md), [generated-visual parameters](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/protocols/media-visualizer-parameters.md), and [Cue snapshots](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/protocols/media-cue-snapshots.md).
