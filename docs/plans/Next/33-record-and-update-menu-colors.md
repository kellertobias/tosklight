# Record and Update Menu Colors

## Status and scope

Give Record and Update workflows consistent, distinct color identities across their menus and modal states.

## Visual contract

Record uses the desk's Record red treatment. Update uses the desk's amber/yellow Update treatment. Apply the appropriate theme to the workflow header, modal boundary, selected tabs or modes, target rows, armed state, primary confirmation, focus/pressed states, and relevant completion feedback instead of coloring only one label.

The theme must not replace text. Every surface continues to say **RECORD**, **Record Settings**, **UPDATE**, or **Update Settings** as appropriate, and errors retain a separate error treatment. Disabled actions, destructive overwrite choices, revision conflicts, and ordinary Cancel controls must remain visually distinguishable from the workflow theme.

Use shared theme tokens so software-only, hardware-connected, touch, and keyboard-focus states do not drift. Maintain readable contrast for normal and color-vision-deficient operators.

## Acceptance criteria

1. Record Settings, target selection, overwrite/merge choices, and completion actions use one consistent Record theme.
2. Update Settings, Update target menus, previews, Update Update, and completion actions use one consistent Update theme.
3. Labels, icons, focus, and shape still communicate mode without color alone.
4. Errors, disabled actions, Cancel, and destructive confirmations remain unambiguous.
5. Visual coverage exercises both workflows in software-only and hardware-connected layouts.
