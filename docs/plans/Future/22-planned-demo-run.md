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
  -
