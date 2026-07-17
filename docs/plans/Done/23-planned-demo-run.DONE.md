# Completed: Planned demo run

This plan is implemented by the single narrated `DEMO-001` scenario in `tests/product-demo.spec.ts`. The retained coverage contract and the literal normalizations required by the shipped fixture library and current desk model are documented in the development test catalog.

Implementation normalizations:

- House Mood uses `2.17` with multi-patches `2.18`–`2.24`, avoiding the plan's overlap with House Light `2.13`–`2.16`.
- The shipped `Curtain 2 m` profile in its `5 m` mode represents each five-metre-high curtain; the desk does not persist per-instance scale or parent/mount relationships.
- Art-Net destination universe 1 is used because destination universe 0 is not valid in the current route contract.
- Groups 11 and 12 are Profiles Odd and Profiles Even; the plan's later references to Groups 11/21 are treated as the stated typo.
- Playback slots 1 and 2 are assigned directly to the two Group masters because `SET GROUP … AT SET …` is not a valid command grammar.
- The current Cuelist label for the requested non-tracking wrap is `Reset`.
- The shipped ROBE Mode 3, JB-Lighting S16, Showtec 30 Channel, Generic DRGBW dimmer-first, Generic Two channel/four blind, and Generic Fan/Fog modes are used. ACL primary fixtures are patched at `1.1` and `1.2`; their seven multi-patches remain unpatched.
- Color-playback handoffs use explicit playback Off actions after the replacement color starts, matching the requested visible outcome without claiming cross-Cuelist auto-off semantics the playback engine does not provide.

Okay, now let's use this demo setup to build the following SINGLE test case And it needs to be one single test.
Ideally we also have titles on the video that explains what we are currently doing:

# Show Setup
- app opens, go into show menu, click new show
- go into show patch
- add the following layers: "Front Truss", "Back Truss", "Floor", "House Lights", "Stage"
- Start with adding the following stage elements: ("Count"x "Manufacturer" -> "Fixture" -> "Mode")
  - 16x Venue -> Stage 2x1 -> 50cm: place them so that we have a 8m wide by 4m deep stage
  - 3x Venue -> 4-Point Truss -> 2 Meters: All 3 of them get 3 multipatches, so that each "fixture" has 4 patches. Set Z = 4.15m for all.
    - The first Truss is named "Back Truss";  Place them at Y =  4m and X = -3m THRU 3m 
    - The second Truss is named "Mid Truss";  Place them at Y =  0m and X = -3m THRU 3m 
    - The third Truss is named "Front Truss"; Place them at Y = -3m and X = -3m THRU 3m 
    - INFO: the center point of the truss is in the middle, so a 2m truss placed at -3m goes from -4m to -2m
  - 4x Venue -> 1-Point Truss/ Pipe -> 2.5 Meters: Place at X = -1.5m THRU 1.5m, Y = 4.2m, Z = 3.05m, name them Pipe 1-4
  - 2x Venue -> Curtain -> 4 Meters: Place at X = -2 THRU 2, Y = 4.3, Z = 2.5m and scale so that the curtain is 5m high
- Now, Patch the lamps:
  - 4x Generic -> Dimmer: Fresnel -> 1ch: Name: "Front Left 1-4",  X = -3.8m THRU -3m,  Y = -3, Z = 4m, mount = Front Truss, layer = Front Truss, Patch 2.1-2.4, Fixture ID: 1-4
  - 4x Generic -> Dimmer: Fresnel -> 1ch: Name: "Front Right 1-4", X =    3m THRU 3.8m, Y = -3, Z = 4m, mount = Front Truss, layer = Front Truss, Patch 2.7-2.10, Fixture ID: 5-8
  - 1x Generic -> Dimmer -> 1ch: Name: "House Light", layer: "House Light", patch 2.13, multipatch 2.14-2.16, Fixture ID: 99
  - 1x Generic -> Dimmer -> 1ch: Name: "House Mood",  layer: "House Light", patch 2.17, multipatch 2.14-2.24, Fixture ID: 98
  - 2x Generic -> Dimmer: Single ACL -> 1ch: Names: "ACL In" & "ACL Out", Both get 7 multipatch, unpatched. , layer = Back Truss, patch, 1.1 & 1.2, Fixture IDs: 81 & 82
    - ACL In gets mounted to the Back Truss on the top in the middle, and angled, so that it does a proper fan out
    - ACL Out gets mounted to the Back Truss on the top 4 left, 4 rigfht, and angled, so that it does a proper fan in
  - 8x Robe -> Robin DLS Profile -> Choose any mode: Name Profile 1-8. X = -3.8 THRU 3.8, Y = 3.85, Z=4. Rotate, so it points down. Layer: Back Truss, Mounted to Back Truss. Fixture IDs: 101+. Patch: 1.13+
  - 7x JBLed -> A7 -> 16 bit mode: Name Was 1-7. X = So that they sit between the Profiles, Y = 3.85, Z=4. Rotate, so it points down. Layer: Back Truss, Mounted to Back Truss. Fixture IDs: 201+ Patch: Next suggested address on Universe 1
  - 8x Showtec -> Sunstrip LED RGB -> 10 Cell mode: Mount so, that they point forward on the 4 pipes, mount to pipe, layer: Back truss. Patch to Universe 3 Fixture IDs: 401+
  - 16x Generic -> RGBW LED Par -> 8 Bit RGBWI: Name Floor Spot 1-16. Place in 4 Groups of 4, fanned. Position: Y = 3.5m, Z = 5m. Pointed upwards, in a fan fowards the audience. Beam angle narrow. Layer "Floor". Patch: Next free on Universe 3. Fixture ID: 301+
  - 2x Generic -> Blind -> 2ch 4 Blind: Name Blind left and blind right. Mount top stacked on front truss, pointing towards audience. Fixture ID: 801+
  - 2x Generic -> Haze -> Fan, Fog: Name Haze Left & Haze Right. Fixture ID: 998 & 999
- Save the show as "Demo Show"
- Go into Desk Setup and add 3 Routes:
  - Universe 1: ArtNET Universe 0 (IP Address and so doesn't really matter, output active)
  - Universe 2: sACN Universe 1 (IP Address and so doesn't really matter, output active)
  - Universe 3: ArtNET Universe 1 (IP Address and so doesn't really matter, output active)

# Group Preparation
- Open Fixture view. Go into settings, enable the group overlay.
- `1 THRU 8 REC GROUP 9 ENTER` to store the front lights in Group 9
- `SET GROUP 9 ENTER` to open the group settings modal. Give the Group the name "Front"
- `SET GROUP 1 ENTER` to open the group settings modal and make it an empty group (from a null group). Give the Group the name "Profiles"
- `SET GROUP 2 ENTER` to open the group settings modal and make it an empty group (from a null group). Give the Group the name "Wash"
- `SET GROUP 3 ENTER` to open the group settings modal and make it an empty group (from a null group). Give the Group the name "LED"
- `SET GROUP 4 ENTER` to open the group settings modal and make it an empty group (from a null group). Give the Group the name "Strips"
- `GROUP 1 DIV 2 REC GROUP 11 ENTER`, then `SET GROUP 11 ENTER` and name it "Profiles Odd`
- `GROUP 1 DIV 2 + 1 REC GROUP 11 ENTER`, then `SET GROUP 21 ENTER` and name it "Profiles Even`
- `101 THRU 199 REC GROUP 1` to actually assign lamps to the group 1
- `201 THRU 299 REC GROUP 2` to actually assign lamps to the group 2
- `301 THRU 399 REC GROUP 3` to actually assign lamps to the group 3
- `401 THRU 499 REC GROUP 4` to actually assign lamps to the group 4
- `SET GROUP 11 AT SET 1.1 ENTER` to assign group 11 at playback 1 on page 1
- `SET GROUP 12 AT SET 1.2 ENTER` to assign group 11 at playback 2 on page 1

# Turn lights on
- Clear the selection
- Go into Programmer -> Control
- Open the Special Dialog
- Click "Lamps On" to turn ALL lamps on (since we have an empty selection)

# Preset Programming
- Program Red, Yellow, Green, Cyan, Blue, Magentha, White Color presets for all lamps (one preset per color, merged for all lamps)
- Program Fan Out, Mirrored Fan Out, Audience, Center, Crossed position presets for all moving lights.
- Program Presets for some GOBOs

# Cue Programming
- Set all front lights to 100%, record to playback 1.3
- Set all Profiles (via the group) to 100%, record to playback 1.3
- Set all wash via group to 100%, record to playback 1.3, choose "add second step"
- now set both groups via preset to a color, one red, the other blue. record as next step in 1.3
- now store all wash in red, store on playback 1.21
- now store all wash in blue, store on playback 1.22
- now store all profile in red, store on playback 1.23
- now store all profile in blue, store on playback 1.24
- Store ACL In @ 100% to playback 1.4, second cue: ACL I @ 0%, ACL Out @ 100%.
- Go in cuelist settings for 1.4 and configure the wrap around to non-tracking and to chaser with speed group A

# Busking
- Trigger 1.21 & 1.23. Turn on Playback 1.1, 1.2 and 1.3
- Wait a few seconds
- click go on 1.3
- Trigger 1.22 (see 1.21 turn off and wash immediately turn blue)
- Trigger 1.3 GO. This is the cue, where profile and wash both get a color. See 1.22 and 1.23 turn off.
- Turn off 1.3 and start it again with GOTO CUE 2

# Preloading
- Press Preload
- turn on 1.21 and 1.24
- select new positions (presets) for both, profile and wash
- set a programmer fade time of 4s
- press preload go
- wait 6s
- turn on 1.4
- tap speed in Speed group A


---

Here is how to use the demo recording

Use [product-demo.spec.ts](/Users/keller/repos/light/tests/product-demo.spec.ts) as the template:
Load a known show state with loadCanonicalCopy(...).
Open ?demo=product.
Perform the demo using normal Playwright interactions or API commands.
Add short waits where viewers need time to understand the result.
Assert the expected state—so it remains a real regression test.
Run:
./test demo
Recording mode automatically uses Chrome at 1920×1080 with 250 ms slow motion. Outputs are written to:
artifacts/product-demo/tosklight-product-demo.webm
artifacts/product-demo/tosklight-product-demo-1920x1080.png
For additional demo scenarios, create another Playwright spec following the same pattern and give its video.saveAs(...) call a unique filename. Simulated hardware interactions should use actual button locators, for example:
await keypad.getByRole("button", { name: "GRP", exact: true }).click();
That exercises the real interaction path and records the visible pressed-button state. For recording arbitrary UI tests, use:
./test record tests/my-demo.spec.ts
