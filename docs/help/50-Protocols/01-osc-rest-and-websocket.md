# OSC, REST, and WebSocket Protocols

Use these interfaces only on a trusted lighting network. One ToskLight application and its attached OSC hardware form one desk with one shared command line and authoritative desk state. A different desk alias remains an isolated control context.

## OSC

An OSC client subscribes with `/light/subscribe` and the arguments `client ID`, `desk alias`, and feedback port. Unsubscribe with `/light/unsubscribe` and the client ID. A successful subscription returns feedback under `/light/{desk}/feedback/...`, including the current page, command line, keys, playbacks, Speed Groups, and lock state.

Keypad input uses `/light/{desk}/programmer/{key}` with a pressed value. Digits are `digit-0` through `digit-9`; command names include `group`, `at`, `plus`, `minus`, `time`, `shift`, `set`, `record`, `enter`, `clear`, and `backspace`. The [Command Line Reference](../30-Programmer/01-command-line.md) defines their operator semantics.

Playback addresses deliberately distinguish current-page and explicit-page operation:

| Address | Meaning |
| --- | --- |
| `/light/{desk}/page-playback/{playback}/{control}` | Resolve the playback number against the page currently selected on that desk or screen. |
| `/light/playback/{page}/{playback}/{control}` | Address a specific global page and playback, independent of every desk's current page. |
| `/light/cuelist/{number}/{action}` | Operate a Cuelist directly when a page playback is not the intended target. |

Changing a page in the application changes where the same `page-playback` packet is routed. It does not change the meaning of an explicit `/light/playback/{page}/...` address. Compatibility aliases may remain available, but new integrations should use the canonical forms above.

### Highlight and Step Through

Highlight actions use the OSC subscriber's authenticated user/session and the desk named in the address. Send a pressed Boolean value to one of these addresses; releases and messages from an unsubscribed command socket are ignored.

| Input address | Authoritative action |
| --- | --- |
| `/light/{desk}/highlight/on` | Turn Highlight on, capturing the current selection only when there is no remembered selection. |
| `/light/{desk}/highlight/off` | Remove Highlight output and leave the remembered selection available. |
| `/light/{desk}/highlight/toggle` | Toggle the same authoritative state used by software controls. |
| `/light/{desk}/highlight/capture` | Replace the remembered ordered selection with the current selection. `/reset` is a compatibility alias. |
| `/light/{desk}/highlight/next` | Enter Step mode at the first fixture or advance once without wrapping. |
| `/light/{desk}/highlight/previous` | Move back once without wrapping. `/prev` is a compatibility alias. |

Physical button bounce or a repeated identical action is accepted only once inside a 150 ms guard window. Aliases are normalized before this check, so `/previous` followed by `/prev`, or `/capture` followed by `/reset`, cannot advance or capture twice. A different action is accepted immediately. Software, REST, and OSC all call the same server state; an OSC client must not maintain its own step index.

Every normal feedback cycle includes:

| Feedback address | Value |
| --- | --- |
| `/light/{desk}/feedback/highlight/active` | Boolean Highlight on/off state. |
| `/light/{desk}/feedback/highlight/output` | Boolean indicating whether live Highlight output is currently allowed. It is false for Blind/Preview capture-only state. |
| `/light/{desk}/feedback/highlight/index` | One-based active step index, or `0` while all captured fixtures are highlighted or Highlight is off. |
| `/light/{desk}/feedback/highlight/total` | Count of valid fixtures in the remembered selection. |
| `/light/{desk}/feedback/highlight/can-next` | Boolean availability of a non-wrapping Next action. |
| `/light/{desk}/feedback/highlight/can-previous` | Boolean availability of a non-wrapping Previous action. |
| `/light/{desk}/feedback/highlight/fixture/id` | Active step fixture UUID, or an empty string. |
| `/light/{desk}/feedback/highlight/fixture/number` | Active fixture number, or `0` when absent. |
| `/light/{desk}/feedback/highlight/fixture/name` | Active fixture/head name, or an empty string. |

Refresh all of these fields after reconnect instead of applying an old local index. An action rejected because another user owns live Highlight output leaves the authoritative state unchanged.

## REST

REST is rooted at `/api/v1`. Health, readiness, version, session creation, and the desk-lock boundary are available before ordinary authenticated operations. Create a session with `POST /api/v1/sessions`, then send its bearer token for protected reads and mutations. A LAN deployment should configure `LIGHT_DESK_TOKEN` and must not expose the API directly to an untrusted network.

The main resource families are:

| Resource family | Examples |
| --- | --- |
| Service state | `/health`, `/readiness`, `/version`, `/diagnostics`, `/bootstrap` |
| Sessions and users | `/sessions`, `/users` |
| Shows and revisions | `/shows`, `/shows/{id}/open`, `/shows/{id}/revisions` |
| Show objects | `/shows/{id}/objects/{kind}` and revision-checked object mutations |
| Programmer, Highlight, and playback | `/programmer/set`, `/programmers`, `/highlight`, `/highlight/action`, `/playbacks`, `/playback-pool/{number}/{action}` |
| Desk and screen control | `/control-desks/{id}`, `/control-desks/{id}/page`, `/screens/{id}` |
| Output inspection | `/dmx`, `/dmx/override`, `/configuration` |

Treat response revisions as concurrency guards. A stale or invalid mutation must be rejected rather than partially applied. Use the current server response and audit/event stream as the authoritative result instead of assuming a successful local UI update.

For Highlight, `GET /api/v1/highlight` returns the current desk/user state. `POST /api/v1/highlight/action` accepts a body such as `{"action":"next"}`; the action is one of `capture`, `on`, `off`, `toggle`, `next`, or `previous`, and the response is the updated state. Step indices in REST are zero-based (or `null` while all captured fixtures are identified); the OSC feedback index is deliberately one-based with `0` for no active step.

## WebSocket events

Connect to `/api/v1/events` with the authenticated token subprotocol. WebSocket events carry live changes that would be inefficient or ambiguous to poll, including show revisions, authoritative desk/programmer state, `highlight_changed`, and typed control feedback. REST establishes or changes state; WebSocket confirms and follows the resulting live state.

Reconnect by refreshing the relevant REST/bootstrap snapshots before applying later events. Do not replay an old client-side snapshot over newer server state after a reconnect.
