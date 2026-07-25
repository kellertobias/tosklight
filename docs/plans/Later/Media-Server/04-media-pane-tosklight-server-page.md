# Media Pane ToskLight Server Page

## Status

**Specification only.** This chunk records the ToskLight-specific Media pane page for connecting to the native Media Server integration. It does not implement native protocol requests, server administration, generated sources, text overlays, persistence, help changes, or executable tests.

## Goal

Add a special page or advanced view inside the Media pane for the native ToskLight Media Server integration.

The generic Media pane works against a patched media-server fixture and advertised capabilities such as CITP/MSEX. The ToskLight-specific page appears only when the selected server advertises the native Media Server integration. It exposes native capabilities that generic CITP cannot represent.

## Availability

The page is reachable through a separate title-bar action or similarly explicit pane-level control. It appears only when the selected patched media-server master advertises ToskLight Media Server capabilities.

A generic CITP server must not receive native ToskLight requests. If the selected server disconnects, loses the native capability, or is replaced by a generic server, the page shows an unavailable state and offers a route back to the generic Media pane.

## Page Responsibilities

The ToskLight Server page may expose:

- native connection status and endpoint details;
- generated sources;
- text overlays and text-source editing;
- native preview/source mappings;
- native capability diagnostics;
- media-server-specific library or renderer status that is not part of the generic pane; and
- direct routes to deeper Media Server administration when that exists.

The page should not duplicate generic folder/file browsing or layer selection unless native capability data materially changes the workflow.

## Connection Workflow

The page must define how the Light Desk connects to the native Media Server specifically:

- whether the endpoint is discovered from the patched fixture, entered manually, selected from discovery, or inherited from Show Patch;
- how authentication or trust is represented if required;
- how connection errors are shown;
- how reconnect behaves;
- how multiple Media Server instances are distinguished; and
- how the generic pane and native page share selected server/layer identity.

## Acceptance Coverage

1. The ToskLight Server page appears only for servers advertising the native capability.
2. Generic CITP servers never receive native ToskLight requests.
3. The page exposes connection status, endpoint identity, and unavailable states.
4. Generated sources and text overlays are reachable only when advertised.
5. Editing text or native-only fields uses native server operations, not generic folder/file programmer transactions.
6. Switching server, losing connection, or losing capability returns to a clear unavailable or generic Media pane state.
