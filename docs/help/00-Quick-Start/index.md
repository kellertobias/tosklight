# Quick Start

ToskLight is open-source show-control software for people who want the concepts of a professional lighting desk without requiring a full-size console. The source is published in the [ToskLight GitHub repository](https://github.com/kellertobias/tosklight), and the conditions for using, modifying, and distributing it are in the [ToskLight license](https://github.com/kellertobias/tosklight/blob/main/LICENSE).

ToskLight is still under active development. Rehearse the complete production workflow, keep recoverable show copies, and verify physical output before relying on a testing build for a live event.

## The ToskLight applications

The ToskLight bundle contains three operator products:

- **ToskLight Desk** is the lighting-control application. It owns the Programmer, Groups, Presets, Cuelists, Cues, Playbacks, Dynamics, patch, output routes, users, and screen layouts.
- **ToskLight PreViz** combines a standalone renderer with the **PreViz Rig Editor**. The Rig Editor plans and patches a rig; the renderer draws it from live Art-Net or sACN, from the editor's preview, or from its built-in demonstration source.
- **ToskLight Media Server** plays video, pictures, text, generated visuals, and effects on configured displays. The Desk controls its master and layers through patched Media fixtures and can use CITP/MSEX for names, thumbnails, libraries, and previews.

The Desk and PreViz Rig Editor use the same portable `.show` format. A show can be opened by either application without conversion, and each file contains the fixture-profile revisions required by its patch. When both applications are on the same network, their ToskLight discovery service can offer **Load from Desk** or **Load from Visualizer**. This transfers a copy; it does not create two simultaneous writers to one live file.

The PreViz Renderer can connect directly to a running Desk for scene and configuration data. Its live fixture values still arrive through real Art-Net or sACN, so a Desk output route must reach the PreViz machine. A PreViz window launched by the Desk receives the Desk connection automatically.

Media cooperation uses a different protocol. The Media Server advertises CITP/MSEX, and the PreViz Rig Editor can discover it while configuring media surfaces. In ToskLight Desk, patch the Media fixture first and configure its CITP address under **Show Patch → Media Servers**. Layer selection and numeric folder/file programming remain available when CITP is absent.

## Intended show size and use

ToskLight is aimed at small and medium productions. Its central workflows suit live busking, discos, DJs, bands, clubs, and community events, while tracked Cues, Timecodes, Macros, schedules, and Preload also support theatre, stage plays, musicals, architectural lighting, churches, schools, and community centres.

The target sweet spot is a working rig of roughly 400 controllable fixtures. ToskLight does not impose an artificial fixture-count limit: larger patches remain valid, but output load, active Dynamics, client count, fixture complexity, and 3D rendering eventually expose the limits of the computer and network being used. Test the real show on the real machines rather than treating a fixture count as a universal performance guarantee.

## Start from the Demo Show

The bundled **Demo Show** is a useful starting point, not merely a screenshot fixture. It follows the lead programmer's normal setup philosophy:

1. Stable Groups describe the physical rig, such as stage Profile fixtures, stage Wash fixtures, audience lights, or Sunstrips.
2. Those physical Groups are combined or referenced to create the Groups used while busking.
3. Presets store reusable Intensity, Color, Position, Beam, Shapers, Focus, Control, Media, or mixed looks for those selections.
4. Cues, Cuelists, Dynamics, and Playbacks build the running show on top of that structure.

Open the Demo Show, inspect how its Groups lead to Presets and Playbacks, and adapt that structure to the production. Use **Load Clean Built-in Default** when a fresh working copy is needed; it leaves the untouched bundled source available for the next reset.

![The default programming desk with fixture selection, Group shortcuts, Stage, and the live Programmer](../assets/screenshots/default-desk-overview.png)

## First Desk look

This example turns on a range of fixtures, records the Programmer to a playback, and then proves that the playback can reproduce the look.

1. Start **ToskLight Desk** and load the Demo Show.
2. Select the intended fixtures in the Fixture Sheet, Stage, or Group Pool. A simple command-line example is `[1][THRU] 12 [AT] 100 [ENT]`, which selects Fixtures 1 through 12 and gives them 100% Intensity in the Programmer.
3. Press `[REC]`, then choose an empty playback. On the touch playback surface the complete playback is the Record target. On an attached desk, use its topmost playback button; the hardware fader is never intercepted by Record.
4. The Desk creates a Cuelist, records its first Cue, and assigns that Cuelist to the playback.
5. Press `[CLR]` once to clear the selection and again to clear the Programmer values.
6. Press the playback's GO or Toggle button. The fixtures now come from the recorded Cue rather than from the Programmer.

What is in the Programmer is what gets stored in the Cue. A resolved output value, Highlight, a running playback, or a fixture default is not recorded merely because it can be seen. Continue with [Show Setup and Patching](../10-Desk/10-Show-Setup/index.md), [Programmer and Cues](../10-Desk/20-Programmer-and-Cues/index.md), and the precise [Command Line Reference](../10-Desk/20-Programmer-and-Cues/01-command-line.md).

## First Media output

1. Start **ToskLight Media Server** and open its administration address in a browser.
2. Choose the Main output and select the intended monitor or off-screen target. Use the test pattern before loading show content.
3. Choose a library folder/file slot and upload the source. The Media Server queues conversion to its playable format; wait for the job to finish or resolve its visible failure before programming that slot.
4. Open the output on the display, take control of a layer, select the converted content, and verify the composite preview and physical display.
5. When the Desk should control it, patch the matching combined Media Server personality, match its Art-Net or sACN address, and configure the Media Server's CITP endpoint in Show Patch.

Network, monitor, resolution, presentation-rate, and audio-device changes are saved configuration and take effect after the Media Server restarts. Layer, playback, and takeover changes are live.

## First PreViz rig

1. Start **ToskLight PreViz**. A standalone launch opens the **PreViz Rig Editor** first.
2. Use its writable Demo Show copy, open another `.show` file, import MVR, or choose a discovered Desk and **Load from Desk**.
3. Patch and position fixtures in the Rig Editor, then choose **Open Viz** to start the PreViz Renderer against that planning document.
4. Use the editor's fixture preview for a quick check without a lighting desk. For live operation, configure the Art-Net or sACN inputs and send the matching universes from the Desk or another console.
5. If discovery is unavailable, open the same portable show file directly or enter the Desk host and port manually. Discovery is a convenience, not a requirement.

The Rig Editor consumes the fixture package's models, gobos, wheels, lasers, and effect scripts; fixture asset authoring belongs to the Desk's Fixture Library workflow. Continue with [ToskLight PreViz](../20-ToskLight-PreViz/index.md).

## Before doors

- Save a named revision and prove that the intended show reopens.
- Verify the active user, show, playback page, output routes, universes, and physical destinations.
- Clear temporary Programmer, Highlight, Freeze, FixAT, and Preload states that are not part of the show.
- Run every required Cuelist, Macro, Timecode, Dynamic, Preload transition, and release path.
- Confirm Media displays, audio, CITP previews, and content slots from the production machine.
- Confirm PreViz scene data and every required Art-Net or sACN universe from the production network.
- Keep the generated PDF manual with the release, or open **Help** from ToskLight Desk.

Continue with [Installation and First Start](01-installation-and-first-start.md), [ToskLight Desk](../10-Desk/index.md), and [Windows and Panes](../10-Desk/30-Windows/index.md).
