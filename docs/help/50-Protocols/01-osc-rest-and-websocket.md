# OSC Protocol

Use this interface only on a trusted lighting network. One ToskLight application and its attached OSC hardware form one desk with one shared command line and authoritative desk state. A different desk alias remains an isolated control context.

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

Highlight and selection-step actions use the OSC subscriber's authenticated user/session and the desk named in the address. Send a pressed Boolean value to one of these addresses; releases and messages from an unsubscribed command socket are ignored. Highlight state is independent of the actual programmer selection and its step state.

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

Refresh all of these fields after reconnect instead of applying an old local index. An external authoritative selection event immediately replaces the old step basis and feedback with the resulting complete selection. An action rejected because another user owns live Highlight output leaves the authoritative state unchanged.
