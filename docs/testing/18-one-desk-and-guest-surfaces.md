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

## Opening a screen joins the open desk

1. Start the desktop application on a server other than the default `127.0.0.1:5000`, with a
   different server stored as the operator's server setting.
2. Add a screen and press **Open Screen**. Confirm the screen window appears, reaches only the
   main window's server, and does not create another client or operator session.
3. Type on the main window's keypad. Confirm the screen shows the same command line live while the
   main window stays open and usable.
4. Make the server unreachable for the screen. Confirm the screen names the server and says it is
   not reachable, offers no server setting of its own, and offers **Retry now**.
5. Press **Retry now** several times. Confirm it only tries to join again: no new client, session
   or window appears.

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

## Hardware Controls input modes

1. Start Hardware Controls against a running desk. Confirm the title bar shows **OSC** selected and
   the status reads *OSC* with the desk connected and its page. Press an encoder; the status names
   the control that was sent and the desk reacts once.
2. Select **Native Hardware** with no native extension configured. Confirm the status reads
   *No device available* with the desk's reason, the console is dimmed, and on-screen presses do
   nothing on the desk.
3. Stop the desk server. Confirm the status turns into a *Device error* naming the unreachable
   desk rather than keeping the last good state.
4. Select **OSC** again. Confirm only one mode is shown as selected, the status no longer mentions a
   device, the desk sees one Hardware Controls subscription, and an on-screen press arrives once.
5. Quit and relaunch. Confirm the last chosen mode is restored.

### Native simulator protocol

1. Install and enable `tl-hardware-simulator-extension`, then rescan extensions. Confirm the main
   desk switches to hardware-connected layout when the supervised child completes its handshake,
   without any OSC subscriber.
2. Select **Native Simulator** in Hardware Controls. Confirm its relay connects and the desk still
   has no OSC subscriber.
3. Exercise a Programmer key, encoder, navigation control, Playback button, and Playback fader.
   Confirm each arrives once with action source `extension` and the desk mirrors the result.
4. Stop or fault the simulator extension. Confirm the main desk returns to software-only layout
   within a few seconds unless another OSC or native control surface remains connected.
5. Run a telemetry-only or timecode-only extension. Confirm neither one switches the desk into
   hardware-connected layout.

### Real-device check (manual)

With an approved control-surface extension package and its device attached to the desk:

1. Select **Native Hardware**. Confirm the status reads *Device connected* with the package name.
2. Move a fader and press a Programmer key on the device. Confirm the desk reacts once, the
   on-screen console mirrors the change, and nothing is sent from Hardware Controls itself.
3. Unplug the device. Confirm the status changes to starting or error with the extension's last
   error within a few seconds, and recovers to *Device connected* after plugging it back in.
4. Switch to **OSC** and back. Confirm no input is doubled and the device still drives the desk.
5. Record the device, package version, operating system and result in the release test log.



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
