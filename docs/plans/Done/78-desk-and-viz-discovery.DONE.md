# Desk and Viz Discovery: Load from Desk, Load from Visualizer

## Status

**Implemented, 2026-08-01.** Discovery lives in `crates/shared/discovery`; the desk advertises and
browses from its runtime and serves `GET /api/v2/discovery/peers`; the Viz editor advertises,
browses, and serves its open document to the network. Both entry points are in place and covered
by tests. What is written below is what was built.

## Goal

A rig planned in the Viz editor and a show running on a desk are the same rig. Today, moving one to
the other means finding a file. Both sides should offer the other directly:

- the **Viz editor** gains **Load from Desk** when a ToskLight desk is on the network; and
- the **desk** gains **Load from Visualizer** in its Load Show menu when a Viz editor with a
  document open is on the network.

Each button loads the other side's show. Neither appears when there is nothing to load, because a
button that is always there and usually fails is worse than no button.

## Decisions

**Discovery is mDNS/DNS-SD.** Each side advertises `_tosklight._tcp` and browses for the other.
This is what a professional product does on a lighting network, it works across a mixed Mac and
Windows rig with no configuration, and it crosses the subnet boundaries a UDP beacon cannot. It
costs one workspace dependency and a responder in both applications. The alternatives considered
and rejected were a UDP broadcast beacon on a fixed port (no dependency, but flat-network only and
another port to open) and remembering previously used addresses (no network work, but it never
finds a desk nobody has typed in yet).

**"Load from Visualizer" pulls the Viz editor's open document.** The editor is the only Viz surface
that holds a show of its own. A running visualizer holds nothing: it draws what it is sent, so
offering its show back to the desk would offer the desk the show it is already running.

## The service record

One service type, `_tosklight._tcp`, with the role in the TXT record, so one browser finds both and
the roles cannot drift apart:

| Key | Desk | Viz editor |
| --- | --- | --- |
| `role` | `desk` | `editor` |
| `name` | The desk's own name | `ToskLight Viz Editor` |
| `show` | Active show name, absent when none is loaded | Open document name, absent when none is open |
| `api` | The v2 API port | The document server's port |

A side with no show still advertises: an editor with nothing open is discoverable, it simply has
nothing to offer, and the entry says so rather than disappearing. Only entries carrying a `show`
are offered as something to load.

## The transfers

**Editor → desk (Load from Desk).** The desk already serves its active show as a portable file at
`GET /api/v2/shows/{id}/download`. The editor fetches it, writes it beside its own documents, and
opens it exactly as **Open** does. What arrives is a copy: patching in the editor afterwards never
writes to the desk's show.

**Desk → editor (Load from Visualizer).** The editor's planning server needs the matching route: the
open document as a portable show file, produced the same way **Save As** produces one so the
profile revisions it uses travel with it. The desk then imports it through the show library and
loads it, with the same conflict behaviour as any other imported show.

Both directions are a copy through a file the other side already knows how to read. Neither becomes
a live link, and neither application becomes a runtime dependency of the other — the rule the
[viz planning README](../Later/viz/README.md) already sets for these two products.

## Acceptance

- **DISCOVERY-001.** With a desk running on the network and an editor open, the editor's file bar
  shows **Load from Desk** naming the desk and its show. Stopping the desk removes it within the
  browser's own timeout rather than leaving a button that fails when pressed.
- **DISCOVERY-002.** With an editor holding an open document, the desk's Load Show menu offers
  **From Visualizer** naming the editor and its document. An editor with nothing open is not
  offered.
- **DISCOVERY-003.** Two desks, or two editors, are both offered and are told apart by name and
  address.
- **DISCOVERY-004.** Loading in either direction produces a copy: editing one side afterwards
  leaves the other unchanged.
- **DISCOVERY-005.** Neither application requires the other to start, and a network with no mDNS
  (or a firewall that blocks it) costs the button, not the application.

## How it was settled

**The crate is `mdns-sd`.** It runs its own responder in-process, needs no system daemon, and
coexists with both macOS's `mDNSResponder` and the desk's Matter responder — verified on a desk
and an editor advertising and finding each other on one machine while the desk's own Matter
bridge was running.

**Both sides advertise at all times.** A desk with no show is discoverable and says it has nothing
to offer, which is more useful than a desk that disappears. Only entries carrying a `show` are
offered as something to load.

**A peer publishes every address it answers on, in the order worth trying.** A machine answers on
each of its interfaces and the set arrives unordered, so a single address picked at random is
often the one the caller cannot reach. Both fetch paths try them in turn; link-local IPv6 is
dropped outright, because it needs an interface scope no URL carries.

**A peer leaves the list when the responder says it has** — a goodbye on a clean exit, record
expiry otherwise. Nothing is dropped for having been quiet: a desk that has sat there unchanged
all afternoon is exactly the desk an operator wants offered.

**`LIGHT_DISCOVERY=off`** turns the desk's half off for an installation that does not want its
desk answering on mDNS.

## Still open

1. Whether an operator can type an address for either direction when discovery finds nothing —
   the visualizer's Quick Settings already accepts one for its own connection.
2. The editor serves its document on all interfaces so a desk elsewhere can fetch it. That is
   read-only and carries nothing but the open document, but it is a listening port an operator
   cannot currently turn off.

These are optional follow-up decisions, not missing parts of the accepted discovery and transfer
contract. If either is wanted, give it a separate scoped plan rather than reopening this completed
implementation.

## Result

Completed in semantic implementation commit `f0a8f240` together with the existing Viz editor
document-transfer implementation. The current tree contains:

- the shared `light-discovery` mDNS/DNS-SD advertiser and browser;
- desk advertisement, browsing, `GET /api/v2/discovery/peers`, show download/import, and the
  conditional **Load from Visualizer** action;
- Viz editor advertisement, desk browsing, document download serving, and the conditional
  **Load from Desk** action;
- portable-copy behavior in both directions; and
- operator help for planning and transferring shows.

Focused verification on 2026-08-01:

- `cargo test -p light-discovery` — 5 passed; the one real multicast integration test remains
  intentionally ignored because it requires local-network multicast.
- `cargo test -p light-headless-runtime discovery_route_tests` — 2 passed.
- `npm test --workspace @tosklight/viz-editor -- --run src/App.test.tsx` — 8 passed.

The source audit also confirmed the desk and editor entry points, both transfer routes, the shared
service record, and the two operator-facing menu actions. This verification is focused code/test
evidence; it does not claim a fresh two-machine manual network run.
