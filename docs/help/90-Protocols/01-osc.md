# OSC Protocol

Use this interface only on a trusted lighting network. One ToskLight application and its attached
OSC hardware form one desk with one shared command line and authoritative desk state.

## Two paths

The path a client connects on decides what it may do.

| Path | Role | Command set |
| --- | --- | --- |
| `/light/desk/...` | Desk button | The full set, including Record, Update and the Assign keyword, exactly as the main window. |
| `/light/remote/...` | Remote control only | Playback only. No Record, no Update, no Assign, no keypad, no encoders. |

A remote-control surface is a **guest**. It does not get a Programmer, a user record, or a desk of
its own — it addresses the same singleton desk and is simply refused the commands that would change
what is programmed on it. Pressing a playback on the guest path operates that playback; it never
captures a Record, even while the operator has one armed. Ordinary fader pickup still applies,
because a guest is still a physical surface.

This is what makes it safe for somebody to work a playback while you record a cue.

A client must send on the path it subscribed on. A guest cannot reach a desk-button route by
addressing one.

## Subscribing

An OSC client subscribes with `/light/subscribe` and the arguments `client ID`, `path`, and feedback
port. Unsubscribe with `/light/unsubscribe` and the client ID. A successful subscription returns
feedback under `/light/{path}/feedback/...`, including the current page, command line, keys,
playbacks, Speed Groups, and lock state — on the same path the client connected on.

Saved hardware configuration naming a path from before these two existed keeps working: any path
other than `remote` connects as a desk-button surface. Those older names are a compatibility path
only, and can be retired once no saved configuration still uses one.

Keypad input uses `/light/{desk}/programmer/{key}` with a pressed value. Digits are `digit-0` through `digit-9`; command names include `group`, `at`, `plus`, `minus`, `time`, `delay`, `link`, `shift`, `set`, `record`, `enter`, `clear`, and `backspace`. The [Command Line Reference](../10-Desk/20-Programmer-and-Cues/01-command-line.md) defines their operator semantics. A successful Link transition is published through the ordinary playback event with cause `link`, previous/current stable Cue references, and transition ordinal. Playback `effective-next-cue` feedback resolves a current Link destination unless an explicit loaded Cue overrides it.

Playback addresses deliberately distinguish current-page and explicit-page operation:

| Address | Meaning |
| --- | --- |
| `/light/{desk}/page-playback/{playback}/{control}` | Resolve the playback number against the page currently selected on that desk or screen. |
| `/light/playback/{page}/{playback}/{control}` | Address a specific global page and playback, independent of every desk's current page. |
| `/light/{desk}/virtual-playback/{page}/{number}/{control}` | Address one show-owned Virtual Playback by its stable number. Page 1 owns 1001–1300, page 2 owns 1301–1600, and the server rejects a number outside the named page's 300-number bank. |
| `/light/cuelist/{number}/{action}` | Operate a Cuelist directly when a page playback is not the intended target. |

Changing a page in the application changes where the same `page-playback` packet is routed. It
does not change an explicit physical or Virtual address. A Virtual number is never an alias for
physical Playback 1–1000 or an old page slot. Its assignment, runtime, and exclusion zones are
shared across the desk; the path records the action source but does not select a separate copy.

### Dynamics

Dynamics OSC requires a desk-button surface; it is programming, so the guest path does not accept it. Pool actions take one pressed Boolean or numeric
value; release values are ignored:

| Input address | Arguments | Authoritative action |
| --- | --- | --- |
| `/light/{desk}/dynamic/{pool-number}/toggle` | Pressed Boolean or nonzero number | Start the resolved Programmer Dynamic instance, or apply Dynamic Off to its exact matching instance. |
| `/light/{desk}/dynamic/{pool-number}/off` | Pressed Boolean or nonzero number | Apply Dynamic Off to the matching Programmer instance without affecting Cue, Playback, user, or differently targeted instances. |
| `/light/{desk}/dynamic/instance/{uuid}/size` | One normalized number from `0.0` through `1.0` | Set the named runtime controller's Size. |
| `/light/{desk}/dynamic/instance/{uuid}/speed` | One positive number | Set the named runtime controller's speed multiplier. |
| `/light/{desk}/dynamic/instance/{uuid}/phase` | One finite number in degrees | Set the named runtime controller's phase offset. |
| `/light/{desk}/programmer/fix-at` | Attribute-name string, then one normalized number | Store FAT for the desk's current ordered selection. |

OSC does not edit Dynamic definitions. Rejected inputs leave authoritative state unchanged and
return `/light/{desk}/feedback/dynamic/error <original-address> <message>`.

Every subscription snapshot publishes `global-paused`, `runtime-count`, and one
`runtime/{runtime-uuid}/{active|pool-number|name|target-count|controller-count|winning-controller|paused}`
family per running instance. Each controller publishes
`controller/{controller-uuid}/{runtime-instance|source|priority|size|speed|phase|paused|winning|releasing}`.
Programmer-owned summaries remain under `feedback/dynamic/instance/{uuid}` and
`feedback/dynamic/{pool-number}/active`. Treat `runtime-count` and the identities in each refresh
as authoritative replacements for a locally cached instance list.

### Highlight and Step Through

Highlight and selection-step actions require a desk-button surface and use the OSC subscriber's authenticated session. Send a pressed Boolean value to one of these addresses; releases and messages from an unsubscribed command socket are ignored. Highlight state is independent of the actual programmer selection and its step state.

| Input address | Authoritative action |
| --- | --- |
| `/light/{desk}/highlight/on` | Turn HIGH on for exactly the actual current selection without changing selection or step state. |
| `/light/{desk}/highlight/off` | Turn HIGH off without restoring ALL, clearing selection, or changing the remembered step source. |
| `/light/{desk}/highlight/toggle` | Toggle the same independent HIGH state used by software controls. |
| `/light/{desk}/highlight/next` | From the complete selection, remember its live source and select the first item; while stepped, advance and wrap from last to first. |
| `/light/{desk}/highlight/previous` | From the complete selection, remember its live source and select the last item; while stepped, move backward and wrap from first to last. `/prev` remains an alias. |
| `/light/{desk}/highlight/all` | Re-resolve the remembered live source, restore its complete current ordered membership as the actual selection, and leave the single-step position. |

There is no Capture action: `/highlight/capture` and `/highlight/reset` are not inputs. Any selection operation outside PREV, NEXT, and ALL replaces the remembered source with the resulting actual selection and returns the selection state to complete. Programmer-value changes do not reset it.

Physical button bounce or a repeated identical action is accepted only once inside a 150 ms guard window. Aliases are normalized before this check, so `/previous` followed by `/prev` cannot advance twice. A different action is accepted immediately. Software, keyboard, and OSC all use the same server state; an OSC client must not maintain its own selection, step index, or Highlight state.

Every normal feedback cycle includes:

| Feedback address | Value |
| --- | --- |
| `/light/{desk}/feedback/highlight/active` | Boolean HIGH on/off state, independent of selection mode, an empty selection, or output suppression. |
| `/light/{desk}/feedback/highlight/output` | Boolean indicating whether live Highlight output is currently allowed. It is false while Blind, Preview, Preload, or another safety boundary suppresses the transient output. |
| `/light/{desk}/feedback/highlight/mode` | `selection` for the complete actual selection or `step` for one stepped item. |
| `/light/{desk}/feedback/highlight/index` | One-based active step index, or `0` in complete-selection state. |
| `/light/{desk}/feedback/highlight/total` | Count of valid items in the currently resolved remembered live source. |
| `/light/{desk}/feedback/highlight/can-next` | True whenever the remembered live source resolves to at least one valid item because NEXT wraps. |
| `/light/{desk}/feedback/highlight/can-previous` | True whenever the remembered live source resolves to at least one valid item because PREV wraps. |
| `/light/{desk}/feedback/highlight/fixture/id` | Active stepped fixture/head UUID, or an empty string in complete-selection state. |
| `/light/{desk}/feedback/highlight/fixture/number` | Active stepped fixture/head number, or `0` in complete-selection state. |
| `/light/{desk}/feedback/highlight/fixture/name` | Active stepped fixture/head name, or an empty string in complete-selection state. |

Refresh all of these fields after reconnect instead of applying an old local index. An external authoritative selection event immediately replaces the old step basis and feedback with the resulting complete selection. A refused Highlight action leaves the authoritative state unchanged.
