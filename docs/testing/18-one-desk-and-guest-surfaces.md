# One Desk and Guest Surfaces

## Purpose

Prove that every surface of the desk operates one Programmer, and that a surface can be limited to
working playback without gaining a Programmer of its own — so somebody else can turn a light on or
run a playback while the operator is recording, without either disturbing the other.

## One Programmer, many surfaces

1. Open the main window, add an optional screen, and connect an OSC wing on `/light/desk`.
2. Type `GROUP` on the main window. Confirm the optional screen and the wing's feedback both show
   the partial command line; it is one command line, not three.
3. Finish `GROUP 7 ENTER` on the wing. Confirm the selection appears on all three, and that the
   Fixture Sheet shows one ordered selection rather than a selection per surface.
4. Set an intensity on the optional screen. Confirm the value is in the Programmer on all three,
   and that a Record from the main window captures it.
5. Press **UPDATE** on the wing. Confirm the main window shows the armed Update. The desk has one
   command line and it is armed everywhere.

## A Not Editable screen

1. In a screen's settings, set **Programming** to *Not editable*. Confirm the switch reads
   *Not editable* and explains what the screen can still do.
2. On that screen, confirm the fixture sheet, the Stage view and the desk's Programmer values are
   all visible and current.
3. From that screen, run a playback and move the Grand Master. Confirm both work.
4. From that screen, attempt to set a Programmer value, and attempt `RECORD GROUP 1`. Confirm both
   are refused and say the screen is Not Editable.
5. Fire a macro from that screen whose only line sets a speed-group speed. Confirm it runs.
6. Fire a macro from that screen containing a `RECORD` line. Confirm it is refused and nothing is
   recorded.
7. Confirm the main window is unaffected throughout: its command line, selection and Programmer
   values are exactly as they were.

## An OSC remote-control surface

1. Subscribe an OSC client on `remote` and another on `desk`.
2. On the main window, select a fixture, set a value, and press **RECORD** so a Record is armed.
3. From the `remote` client, raise a playback fader. Confirm the fader moves, subject to ordinary
   pickup, and that the armed Record on the main window is still armed and has **not** taken the
   playback as its target.
4. From the `remote` client, send a keypad key and a Record key. Confirm neither reaches the
   command line.
5. From the `desk` client, send the same keys. Confirm they behave exactly as the main window's.
6. Confirm the `remote` client cannot operate by addressing `/light/desk/...`, and the `desk`
   client cannot operate by addressing `/light/remote/...`.

## Desk Lock

1. Lock the desk from the main window.
2. Confirm the optional screen, the browser session and both OSC surfaces are all locked — the lock
   is one lock over the installation, not one per screen.
3. Confirm running output is unchanged while locked.
4. Unlock and confirm input resumes on every surface without a stuck held key.

## Migration from an installation with several Programmers

1. Start against a desk database written before the collapse that holds more than one persisted
   Programmer.
2. Confirm the desk comes up operating the Programmer that was touched most recently.
3. Confirm `backups/desk-collapse-*.json` exists under the data directory and contains every
   superseded Programmer whole, with the policy stated.
4. Confirm the log names what was kept and where the rest went.
5. Confirm screen configurations survive: names, layouts, playback layouts, Follow Main/Dedicated
   Page choices, encoder placement, display assignment and fixed panes are all as they were.
6. Confirm a client, a saved hardware configuration or a stored URL naming an identity from before
   the collapse still reaches the desk rather than being refused.
