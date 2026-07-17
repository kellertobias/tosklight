# TODO

Completed items are recorded in [todos-done.md](todos-done.md), with executable
evidence in [docs/todo-completion-audit.md](docs/todo-completion-audit.md).

## Small changes
- Add a "Return Home" to the position special dialog. This sets the given lamps to their default position, which if not defined is 50% / 50&
- Add color alignment to the Color special dialog. While Shift is held on either the normal keyboard or connected hardware, pressing or touching a start color and dragging to an end color defines a color range. On pointer/touch release, apply the range immediately to the current ordered selection, keeping the first and last selected fixtures at the chosen endpoints and spacing every value in between equally. A normal click/tap without Shift continues to apply one color uniformly.
- add a light grey outline around the color dot in the fixture sheet, so that dark colors actually properly distinct from the dark background
- Make sure that the button labels in the Hardware-Connected view of Playbacks are not clickable. The whole playback is clickable and acts either as select (for groups/ group masters) or as select (for cuelists). Selecting a cuelist is used either for a record target (to store on that given cuelist) or it opens the cuelist in the built-ins cue window if not in record mode
- remove the "Development" built ins window. Instead add it via the settings of the help window
- Clicking the Command Line opens the command line towards the top and shows the command line history
- Fix Hardware-Connected Display of Encoders
- DEREF groups command line display
- Record and update menu colors
- Color of active playbacks
- Set Address/ Fixture Address screen needs the available dmx addresses + numblock directly
- Cuelist settings use full width of sidebar. Cue Settings must be inline
- Chaser X-Fade must be in percent, not ms
- Virtual Playbacks Checken (Exclusion Zone Visuals)
- Preload Go Extra Text?
- Playback Configuration Layout
- Preload settings is not input!
- Check file manager + editor
- ArtNET Multicast/ Broadcast
- 

## Larger Blocks
- Dynamics + Chasers
- Check Cuelists + Fade times
- [Cue Go To and Load command grammar](docs/planned%20features/09-cue-go-to-and-load.md)
