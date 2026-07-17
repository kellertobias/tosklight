# Finalize Preload

Extend Preload so it has three independently configurable capture domains. Settings exposes one persisted switch for programmer changes, one for physical/page-playback actions, and one for virtual-playback actions. All eight switch combinations are valid. The default **Both** configuration enables programmer and physical playback capture and leaves virtual playback capture disabled until the operator enables it.

Enabled domains are blind while Preload is armed: they create pending entries without changing their live target. Disabled domains continue to operate live. Preload GO commits every pending domain at one application timestamp. Programmer Fade is the fallback transition time for programmer values and for results produced by physical or virtual playback actions; Cue Fade is not substituted.

For physical playbacks, the Preload payload identifies the real target playback and retains an ordered list containing only these explicit actions:

- Toggle
- Go
- Go minus
- Off
- On
- Temp on
- Temp off

The configured **TEMP** button remains a press-to-toggle control, matching its normal UI behavior; it is not converted into a held button. While Preload capture is armed, the first applicable TEMP press is retained as **Temp on** and the next as **Temp off**, in order, without changing the live temporary state.

Flash actions, ordinary fader changes, and an On state caused only by moving a fader are never captured. Pending entries store action verbs rather than predicted end states. Multiple actions for one playback preserve operator order and execute against the playback's actual state at Preload GO.

Virtual Playbacks are a pane kind in the normal configurable window system, never a built-in fixed surface. A Virtual Playbacks pane contains a configurable grid of single-button cells. Each cell can be assigned a Cuelist, defaults to GO, and can instead be configured as TOGGLE. Pane placement, grid dimensions, assignments, and actions persist. When virtual capture is enabled, virtual GO and TOGGLE actions remain pending until Preload GO and then execute against their real underlying playbacks.

Preload Release removes only the active temporary programmer contribution created by Preload GO. Physical and virtual actions have already changed their actual playbacks and are never undone by Preload Release.

The executable contract and complete configuration matrix are implemented by the paired Playwright scenarios in [`tests/06-preload-modes-and-virtual-playbacks.spec.ts`](../../../tests/06-preload-modes-and-virtual-playbacks.spec.ts).
