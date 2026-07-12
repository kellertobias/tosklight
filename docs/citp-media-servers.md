# CITP media-server semantics

## Patch ownership

Direct control is an explicit capability of a fixture definition. A patched fixture may configure
CITP only when its profile lists `citp` in `direct_control_protocols`. The endpoint is stored on the
portable patched physical fixture, not on the desk, fixture profile, or logical heads. All media
layers inherit the physical parent's IP address and port; per-layer overrides are deliberately not
supported because the parent remains authoritative.

## Transport and caching

The server implements bounded CITP/MSEX 1.2 TCP requests on the standard port 4811 by default:

- CInf version negotiation is sent immediately after connection.
- GETh/EThn retrieves configured media thumbnails.
- RqSt/StFr retrieves a live output preview.
- JPEG, PNG, and strictly sized RGB8 responses are accepted. Unknown formats, malformed lengths,
  oversized packets, invalid dimensions, inconsistent fragment sequences, Nack responses, and
  timeouts are surfaced as visible errors. Ordered CITP fragments are reassembled within the same
  overall size bound.

Requests begin only through an operator action and use a three-second connection/operation timeout.
While **Start live preview** is enabled, the browser requests a fresh single-frame preview once per
second; leaving the view or choosing **Stop live preview** cancels that polling. Thumbnail and
preview data use separate fixture-scoped least-recently-used caches (512 thumbnails and 32 previews
by default). Cached data for fixtures removed from the active patch is discarded during show
activation. The HTTP media endpoints require a valid operator session and never cause show upload
or activation by themselves to contact a media server.

## Offline behavior

Failed communication marks the server offline, retains the last error, emits a revisioned
`media_server_offline` event, and leaves the lighting output engine unaffected. A later successful
refresh clears the error and records the last-success time. The Setup UI shows a stable empty/offline
preview instead of silently replacing it or retrying without operator intent.
