# The ToskLight suite

ToskLight is a suite for planning lighting rigs, programming shows, and running lighting and media. Start with the application that suits the job:

- **ToskLight Architect** plans the rig. Add fixtures, arrange and patch them, then build the venue around them. Prepare drawings and inspect the result in the Visualizer.
- **ToskLight Control** programs and runs the show. Select fixtures, build Groups and Presets, record Cues, and operate Playbacks. Control also builds and patches rigs directly.
- **ToskLight Pixel** is the optional media server. Use it for video, pictures, text, generated visuals, and effects on screens, projectors, or mapped pixels.

![ToskLight Control, Architect, and Pixel application icons](../assets/branding/tosklight-suite.png)

Architect and Control use the same fixture library, patch sheet, and portable `.show` format. Architect is optional: you can add, position, and patch the whole rig in Control. The [shared fixture and patch instructions](../10-ToskLight-Architect/02-Fixtures-and-Patch/index.md) apply to both.

Architect's Visualizer draws the rig and its lighting. Use it to check the plan without a desk or follow Control's output while rehearsing the programmed show. The [Architect visualization guide](../10-ToskLight-Architect/11-visualizer.md) explains scene sources and live DMX. Control's Stage provides a 2D plan and lightweight 3D wireframe for selecting and positioning fixtures.

Pixel's generated visuals are media content. They are separate from the lighting Visualizer, even though Pixel calls their settings page **Visualizers**.

## From rig to show

This walkthrough starts in Architect. If you are working at the desk already, begin with the same fixture and patch tasks in Control.

1. **Add the fixtures.** Choose their profiles and modes, give them fixture IDs, and patch addresses when known. You can program unpatched fixtures and assign addresses later.
2. **Arrange the rig.** Position, rotate, and align fixtures. Add truss, staging, scenery, and any media surfaces around them; group Venue elements when they should be handled together. Save the rig as a native `.show` file.
3. **Open the rig in Control.** Publish it from Architect, load the offered show in Control, or open the saved file. Use a bound Architect document when rig changes should synchronize with the active Control show. Independent file copies stay independent.
4. **Program the show.** Build ordered Groups, reusable Presets, and Dynamics. Record looks into Cues, arrange Cuelists, and assign Playbacks. Use touch controls, encoders, and the programmer command line together.
5. **Rehearse in the Visualizer.** Check looks, Cue transitions, timing, and releases. Configure matching Art-Net or sACN output and input when following live DMX.
6. **Add Pixel if needed.** Test its physical output, prepare the media library, then discover and patch its output in Control. Program media layers alongside lighting.
7. **Check the production rig.** Verify actual fixtures, destinations, and media displays. Save a named revision before the show.

The programmer command line is the desk's lighting command entry, such as `1 [THRU] 12 [AT] 100 [ENT]`. It is an operator control available alongside the keypad and encoders. Terminal commands used for installation are separate.

## Start with the Demo Show

The bundled Demo Show provides a rig and programming to inspect before building your own. Its physical Groups describe the rig, such as stage Profiles, Wash fixtures, audience lights, and Sunstrips. Referenced Groups combine those selections for busking; Presets, Cues, and Playbacks build on them.

Open the Demo Show and inspect a Group, a Preset, and its playback. **Load Clean Built-in Default** creates a fresh working copy when you want to start again. It leaves the bundled source available for another reset.

![Control with fixture selection, Groups, Stage, and the Programmer](../assets/screenshots/default-desk-overview.png)

## Continue with an application

- [Install and start the suite](01-installation-and-first-start.md).
- [Plan a rig in Architect](../10-ToskLight-Architect/index.md).
- [Program and run a show in Control](../20-ToskLight-Control/index.md).
- [Add media with Pixel](../30-ToskLight-Pixel/index.md).
- [Look up keypad and keyboard controls](../99-Appendix/00-keypad-and-keyboard.md).

> [!danger] Testing builds
> Rehearse the production workflow, keep recoverable show copies, and verify physical output before using a testing build for a live event.
