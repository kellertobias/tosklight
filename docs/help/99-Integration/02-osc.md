# OSC

OSC connects buttons, encoders, and playback surfaces to the desk on a trusted lighting network. Configure the running server's bind address before connecting a surface.

Choose the path by the surface's role:

- `/light/desk/...` provides desk controls, including the programmer keypad, encoders, Record, and Update.
- `/light/remote/...` operates playbacks. It cannot program or become a Record target while the desk has Record armed.

Subscribe with `/light/subscribe`, supplying the client ID, path, and feedback port. Use feedback to display the desk's command line, playback page, values, and lock state. Replace locally cached state after reconnecting.

Current-page playback addresses use the visibly selected surface's command page. Explicit-page addresses keep their named page when the operator changes pages. Choose the addressing form that matches the control's purpose.

The [OSC protocol reference](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/protocols/osc.md) contains address tables, arguments, subscription rules, Dynamics, Highlight, and feedback. The [operator command reference](../20-ToskLight-Control/20-Programmer-and-Cues/01-command-line.md) defines what the desk commands do.
