# Hardware-Connected Playback Selection Surface

## Status and scope

Completed. The Hardware-Connected Playbacks view now treats an assigned playback card as one selection surface. Its labels are display-only and do not become independent clickable controls.

## Interaction model

The whole non-control area of an assigned playback card is clickable or touchable. Header text, Cue rows, status labels, button-function labels, and other descriptive text do not install their own click action or create nested interactive targets. Outside Record, real playback buttons and faders remain separately operable and do not trigger card selection when used. With Record armed, the whole card is one Record target: touching its descriptive area, any playback button, or its fader records to the concrete page/playback and suppresses the control's normal action.

Selecting a Group or Group Master playback makes that playback the authoritative selected playback context and applies the corresponding Group selection workflow. Selecting a Cuelist playback makes that concrete playback active. When Record is armed, the selected Cuelist playback becomes the Record target. When Record is not armed, selecting it opens that Cuelist in the built-in Cue window while retaining the concrete playback context. Explicit page/playback identity does not collapse to the same slot on the current page.

Keyboard, touch, pointer, OSC, and attached-hardware actions converge on the same desk-local selected-playback state. Labels use ordinary display semantics with text-selection suppression appropriate to the touch desk.

## Acceptance criteria

1. Clicking/tapping any non-control area of the card selects exactly one playback.
2. Labels are not buttons or links and cannot dispatch a second or different action.
3. Outside Record, operating a real playback button or fader does not also select/open the card unless that control's documented action is Select.
4. Group/Group Master and Cuelist targets follow their distinct selection behavior.
5. With Record armed, every area of the card, including its real buttons and fader, targets that concrete page/playback without executing the touched control; non-Record selection opens a Cuelist playback in the built-in Cue window.
6. Current-page and explicit-page selection remain distinct across page changes.

## Verification

- Component coverage verifies display-only label semantics, single card dispatch, real-control isolation outside Record, and full-card explicit-page Record calls in touch and hardware-connected layouts.
- Paired `PLAYBACK-SELECT-001` coverage compares authenticated API selection with the production hardware-connected card.
- Supplemental browser coverage verifies Cuelist opening, concrete Record mutation, Group selection, explicit page retention, and attached OSC selection.
