# Command Line History Panel

## Status and scope

Make the visible Command Line an entry point to an expanded history panel near the top of the desk.

## Operator behavior

Clicking or tapping the Command Line opens a panel toward the top of the application and shows recent command-line history. The current command remains visible and editable in its normal desk location; opening history must not execute, clear, or replace it.

History entries distinguish accepted commands, rejected commands, and useful result/error feedback. They are ordered newest-first or oldest-first consistently, show enough text and time/context to diagnose recent work, and remain readable without covering the lower command controls. Selecting an entry may copy it into the current command only through an explicit reuse action; a simple inspection click must not execute it.

History is desk/session operator state unless a later persistence design explicitly promotes it. Define a bounded retention policy and redact secrets or sensitive authentication input. Commands arriving through software, keyboard, OSC, or attached hardware should appear once when they share the same authoritative command line.

## Acceptance criteria

1. Clicking/tapping the Command Line opens one top-positioned history panel without mutating the current input.
2. Accepted and rejected commands appear once with clear status and chronological order.
3. The panel is fully reachable at supported software-only and hardware-connected sizes and does not move command-control geometry.
4. Close, Escape, and outside-click behavior are deterministic and preserve unfinished input.
5. History is bounded, reconnect behavior is defined, and sensitive values are not retained.
6. Any reuse action populates the command line but requires the normal explicit Enter to execute.
