# Desk Lock

Add a per-desk lock that prevents anyone from operating a desk while leaving other desks available. Locking a desk must affect every screen connected to that desk and every hardware input device assigned to that desk.

## Locked behavior

When Desk Lock is activated:

- every connected screen replaces its normal desk interface with the configured lock screen;
- navigation, touch, mouse, keyboard, software-keypad, and other application controls on those screens cannot operate the desk;
- all hardware inputs connected or assigned to that desk are disabled, including buttons, faders, encoders, and similar control surfaces; and
- reconnecting or newly connected screens and hardware must receive the current locked state immediately rather than briefly exposing working controls.

Desk Lock is scoped to one desk. It must not lock another desk or disable hardware assigned to another desk. Locking prevents new operator input; it must not implicitly stop playbacks, release the programmer, change output, or alter the current show state.

## Configurable lock screen

The lock screen can be configured per desk with:

- a custom wallpaper uploaded or selected by the user;
- a custom message, for example: `Desk locked. If you want to do something, contact this number.`; and
- the configured unlock control.

The wallpaper and message must be shown consistently on every screen connected to the locked desk. The layout must remain readable across the supported screen sizes and provide a safe fallback when no custom wallpaper or message is configured or when the configured image is unavailable.

## Unlock modes

The desk supports two configurable unlock modes:

1. **Unlock button**: the lock screen displays an **Unlock Desk** button and unlocks the desk when it is pressed.
2. **PIN required**: the lock screen requires the user to enter the configured PIN before the desk unlocks. An incorrect PIN keeps the desk locked and shows a clear error without revealing the PIN.

After a successful unlock, all connected screens return to the normal desk interface and hardware inputs resume from authoritative desk state. Resuming hardware must not interpret stale held buttons, buffered input, or fader movement made while locked as a new action; controls should use the normal safe takeover behavior where applicable.

## Persistence and coordination

Lock-screen configuration is desk-persistent rather than show-persistent. The current locked state must be authoritative for the desk and synchronized across all connected screens and hardware gateways. Planning before implementation must define whether a desk remains locked across an application or server restart, how authorized users recover from a forgotten PIN, and which administrative path can configure or force-unlock a desk without weakening the normal lock screen.

PINs must not be stored or exposed as plaintext. Configuration changes and unlock attempts should be handled by the authoritative desk service so that hiding or bypassing the browser lock screen cannot restore control while the desk remains locked.

## Implemented persistence and recovery policy

The locked state and lock-screen configuration persist with the control desk across server restarts. The server enforces the lock for REST, WebSocket, and desk-addressed OSC input; browsers merely render the authoritative state. PINs are stored as salted SHA-256 digests. A forgotten PIN can be force-unlocked only through the server endpoint with the separately configured `LIGHT_ADMIN_RECOVERY_TOKEN`; the normal lock screen never exposes that path or token. Input received while locked is discarded rather than buffered, and normal live-input ownership/takeover begins again only after unlock.
