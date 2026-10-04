# Typed family values

## Purpose and status

These scenarios are the acceptance contract for typed Color, Position and Focus/Zoom values from
the command line, the desk keypad and OSC (TL-544 G3). The contract is
[Command Line](../help/10-Desk/20-Programmer-and-Cues/01-command-line.md#setting-non-intensity-values)
and the Family values section of [OSC Protocol](../help/90-Protocols/01-osc.md#family-values).

A typed value is the same Programmer edit an encoder makes: Color Intent, Position Angles and Zoom
degrees, never a normalized channel value. Values follow the family's first encoder page in
encoder order and its display units.

Executable coverage:

- Rust: `crates/light/adapters/headless/src/runtime/programmer_family_values_tests.rs` (parser) and
  `…/runtime/tests/command_http_family_values_tests.rs` (command-line route, live Group, Preload,
  Undo, OSC argument parsing).
- Root Playwright: `tests/125-typed-family-values.spec.ts`, under `npm run test:e2e` and
  `npm run test:e2e-semantic`.

Rig: three Generic RGB LED (RGB virtual dimmer) at 1.1, 1.11 and 1.21, and one Cameo AURO SPOT
Z300 (20-Channel) at 1.31.

## TYPED-FAMILY-001 — Color tuples and spreads

1. Enter `FIXTURE 1 THRU 3 AT 100`, then `1 [THRU] 3 [AT][^2] 100 [DIV] 0 [DIV] 0 [THRU] 0 [DIV] 100 [DIV] 0`.
2. Fixture 1 outputs red, fixture 3 green, fixture 2 a red and green mix with no blue.
3. The Programmer holds one `color` Color Intent per fixture and no `color.*` channel value.
4. `1 [AT][^2] [DIV][DIV] 100` (shown as `AT COLOR OFFSET 100`) adds blue to fixture 1 and leaves
   red and green.
5. A value above 100%, more than four values, or `[+]`/`[-]` inside a `[THRU]` spread is rejected
   and changes nothing.

## TYPED-FAMILY-002 — Position angles and Zoom degrees

1. `4 [AT][^3] 45 [DIV] [-][-] 30` programs Angles Pan 45°, Tilt −30°.
2. `4 [AT][^3] [+] 15` steps Pan to 60° and keeps Tilt.
3. `4 [AT][^7] [DIV] 20` programs a 20° Zoom opening.
4. On the desk keypad, `4 [AT]`, Shift+`[3]`, `90 [DIV] 10` shows `F4 AT POSITION 90 DIV 10`;
   `[ENT]` programs Pan 90°, Tilt 10°.
5. `1 [AT][^3] 45` on fixture 1, an RGB LED without Position, changes nothing and shows no error.

## TYPED-FAMILY-003 — OSC family writes and the OSC keypad

1. With fixture 4 selected, `/light/desk/programmer/family/pan -60.5` and `…/tilt 20` program
   Angles −60.5°/20°; `…/zoom 30` programs a 30° opening.
2. With fixtures 1–3 selected at full, `…/red 0 100`, `…/green 0 0` and `…/blue 100 0` spread
   blue to red: fixture 1 outputs blue, fixture 3 red.
3. `…/red 140` is rejected with `/light/desk/feedback/programmer/error` naming the address, and
   changes nothing.
4. The OSC keypad `4 [AT]`, Shift+`[7]`, `[DIV] 12` shows `F4 AT FOCUS DIV 12`; `[ENT]` programs a
   12° opening.
