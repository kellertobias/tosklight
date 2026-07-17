# Hardware-Connected Playback Selection Surface

## Status and scope

Correct pointer ownership in the Hardware-Connected Playbacks view. A playback card is one selection surface; its labels are display-only and must not become independent clickable controls.

## Interaction model

The whole playback card is clickable or touchable. Header text, Cue rows, status labels, button-function labels, and other descriptive text must not install their own click action or create nested interactive targets. Real playback buttons and faders remain separately operable and must not trigger card selection when used.

Selecting a Group or Group Master playback makes that playback the authoritative selected playback context for the corresponding group/master workflow. Selecting a Cuelist playback makes that concrete playback the active playback. When Record is armed, the selected Cuelist playback becomes the Record target. When Record is not armed, selecting it opens that Cuelist in the built-in Cue window while retaining the concrete playback context. Explicit page/playback identity must not collapse to the same slot on the current page.

Keyboard, touch, pointer, OSC, and attached-hardware feedback must converge on the same selected-playback state. Labels should use normal text cursors/selection suppression appropriate to a touch desk without acquiring button semantics.

## Acceptance criteria

1. Clicking/tapping any non-control area of the card selects exactly one playback.
2. Labels are not buttons or links and cannot dispatch a second or different action.
3. Operating a real playback button or fader does not also select/open the card unless that control's documented action is Select.
4. Group/Group Master and Cuelist targets follow their distinct selection behavior.
5. Record-armed Cuelist selection targets that concrete playback; non-Record selection opens it in the built-in Cue window.
6. Current-page and explicit-page selection remain distinct across page changes.
