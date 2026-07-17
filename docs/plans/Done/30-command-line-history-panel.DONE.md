# Command Line History Panel

## Status and scope

Completed. The visible Command Line opens an expanded history panel near the top of the desk.

## Operator behavior

Clicking or tapping the Command Line opens a top-positioned panel showing recent command-line history. The current command remains visible and editable in its normal desk location; opening history does not execute, clear, or replace it.

Entries are newest-first and distinguish accepted from rejected commands with result/error feedback, wall-clock time, and software/attached-hardware source. The fixed panel remains above the lower command controls without changing their geometry. Selecting text or an entry is inspection-only. The explicit **Reuse** action copies an entry into the current command and still requires normal Enter to execute.

History is transient desk state retained in the running server's bounded event history. The same desk sees up to 50 entries after a reconnect; a server restart starts a fresh history. Password, passcode, token, secret, authorization, and API-key-like command text is replaced by a redacted entry. Software, computer-keyboard, OSC, and attached-hardware paths create one entry at the authoritative execution boundary.

## Acceptance criteria

1. Clicking/tapping the Command Line opens one top-positioned history panel without mutating the current input.
2. Accepted and rejected commands appear once with clear status and newest-first chronological order.
3. The panel is fully reachable at supported software-only and hardware-connected sizes and does not move command-control geometry.
4. Close, Escape, and outside-click behavior are deterministic and preserve unfinished input.
5. History retains 50 desk-scoped entries across reconnect, clears with server restart, and redacts sensitive values.
6. Reuse populates the command line but requires the normal explicit Enter to execute.

## Verification

- Server coverage verifies desk scoping, 50-entry retention, newest-first ordering, and command/feedback redaction.
- Component coverage verifies opening, status rendering, non-executing reuse, Escape, and outside-pointer dismissal without input mutation.
- Paired and supplemental `COMMAND-HISTORY-001` Playwright coverage verifies authenticated and production UI paths, result/error ordering, layout invariance, reload reconnect, explicit Enter, hardware-connected reachability, and attached OSC attribution.
