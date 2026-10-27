# Visualize the rig

Choose **Open Viz** in Architect to launch the renderer against the open planning document. Keep CAD or Patch alongside it to inspect placement changes as you work. Architect's fixture preview lights the rig before a desk is connected.

## Scene sources

The dedicated Visualizer can follow an Architect document, a running Control show, an opened portable show, or the Demo Show. Its scene source supplies the rig; live Art-Net or sACN supplies fixture values. Preview values from Architect apply before a universe has received live DMX.

A Visualizer following a Control desk needs a real output route. When both applications run on one computer, use an explicit loopback unicast route. See [DMX output](../20-ToskLight-Control/10-Show-Setup/04-dmx-output.md) for Control's output configuration.

## Lighting the rig without a desk

Select fixtures in the patch sheet and use the preview controls to light them. The preview works without a desk or network route.

**Simple** exposes Intensity, Pan, Tilt, Colour and Gobo. It sets them on every selected fixture at
once, through the fixture's own profile, so one colour works on an RGB fixture and a CMY one
without you having to know which you are looking at, and a 16-bit channel gets its fine byte.

**Full DMX** is available when exactly one fixture is selected, and shows every slot of that
fixture's complete mode: every logical head, every channel, coarse and fine bytes, named as the
fixture library names them. Selecting none or several disables Full DMX and leaves its existing preview values alone.

**Clear** returns the selected fixtures to their defaults.

Preview values last for the window's current session and clear when the document closes. Use Control's Programmer to store presets, cues, and playbacks. Architect has no Programmer command line.

### When real DMX arrives

Preview values apply to a fixture while no source has delivered that fixture's universe.

* Start a real Art-Net or sACN source for one of the show's universes and the fixtures on it follow
  the network immediately. Fixtures on other universes keep the editor's values.
* Stop that source and the visualizer **holds the last values it received** rather than reverting to
  the preview look. A universe that has received real DMX holds it until you clear it or a source resumes.
* Editor values and received DMX are never blended for the same parameter. A universe has one
  owner at a time.

The visualizer's status line says which is happening: **No DMX in — the Viz editor is driving these fixtures** when the preview plane is lighting the rig, and the ordinary **Waiting for DMX** when
nothing is driving it at all.

**Settings**, below the screens, holds the pages that are not a view of the rig: **Visualizer**,
**Library** and **DMX**. It opens on **Visualizer**. **Visualizer** is one page of boxes that fill the window's width, two to a row, three or four once
the window is wide enough, and one to a row in a narrow window: **Lamp** and **Laser** atmosphere,
then **Rendering**, **Features** and **Picture**. **Features** holds **Crowd amount** beside the
labels, selection and floor grid. When the boxes are taller than the window, the page scrolls, so
every control can be reached however short the window is.

## The DMX settings page

**DMX** is a page of **Settings**. Its own three tabs sit left of the Settings pages in the title:

* **Network** configures where the show's DMX arrives from: the network interface this computer
  receives it on, and the live DMX inputs described below.
* **Values** is the desk's **DMX Output** window applied to the DMX this machine receives. Each
  universe the show patches or listens on is a row of dots, one for every channel, that brighten
  with the received level. A patched channel's dot has a grey outline and an unpatched one a dark
  red outline. A universe's header names the protocol, frame rate and sender, **Holding the last
  frame** once a source stops, or **Waiting for DMX** before anything arrived. Select a dot to
  read its value, fixture and DIP switches; with nothing selected, the side column lists every
  input being listened on with its health, its sender, and how many packets it accepted. Values
  has no window settings. The editor outputs no DMX, so nothing on this tab overrides a value.
* **Sources** lists every Art-Net node and sACN source this computer finds on the network, one row per IP address.
  Each row shows its name, IP address, protocols, and the universes it **Sends** and **Receives**.
  **Inputs** lists each DMX input and the Art-Net universe it sends; **Outputs** lists each DMX output and the Art-Net universe it plays. A universe is highlighted while its data is
  arriving. Select a node's name to read its long name, MAC address and status report.

Values listens exactly where the Visualizer does: the show's output routes, the live DMX inputs
over them, and the Art-Net and sACN defaults for every patched universe when neither names any,
each on the interface chosen for its protocol. It only listens while the tab is open, and shares
its ports so a Visualizer on the same computer keeps receiving beside it.

Sources sends an Art-Net poll to each network every three seconds and lists the nodes that answer;
a node that stops answering disappears after about ten seconds. sACN has no poll: a source is
listed from the universe-discovery announcements it sends every ten seconds. The ToskLight desk
does both, so it appears with every universe its routes send. A sender that neither answers polls
nor announces still appears, with the universes it is seen sending: any Art-Net broadcast, and
sACN on the show's own universes. sACN receivers announce
nothing, so they cannot be listed. Like Values, Sources uses the interface chosen for each protocol
and only looks while the tab is open. A node that answers polls only by unicast can be missed
while a Visualizer on the same computer is receiving Art-Net.

## Choose the input network

A computer with more than one network, a lighting network beside an office or venue network, or
two lighting networks, can receive DMX on only the one that carries it. The **Input Interfaces**
section at the top of the **DMX** settings page's **Network** tab has one choice per protocol:

* **Art-Net interface** and **sACN interface** each offer **All interfaces**, the default, and
  every network interface this computer has, by its system name and IPv4 address, for example
  `en0 · 10.0.0.5`. The loopback interface is marked **(this computer only)**.
* Choosing an interface takes effect at once: the **Values** tab and a running Visualizer listen
  on that interface only. Art-Net broadcast, sACN multicast, and unicast addressed to that
  interface all still arrive; DMX arriving on any other network is ignored.
* The two protocols are independent, so Art-Net can come from one network and sACN from another.

The choice belongs to this computer, not the show. It is saved with the Visualizer's settings,
so the Visualizer and the editor always listen on the same network, and a show opened on another
computer never carries this one's interface names.

An interface is remembered by its name, so an address that changes when the network is renewed is
still the same choice. When the chosen interface is not connected, it is listed as **not
connected** and a warning says that the protocol is not received until it returns: the editor
does not quietly fall back to every network. New interfaces appear in the list within a few
seconds of being connected.

## Configure live DMX inputs

The **DMX** settings page's **Network** tab maps a logical show universe to the Art-Net or
sACN universe the separate Visualizer output receives. Each mapping can be enabled or disabled
and carries its protocol, wire universe, delivery mode, and UDP port. Art-Net offers Broadcast or
Unicast; sACN offers Multicast or Unicast. Choose **Apply** to store the mappings in the portable
show, or **Cancel** to discard the draft. The network interface the mappings are received on is
chosen separately for each computer, as described above, and is never written into the show.

When a desk is detected, **Take from Desk** reads that desk's compatible output routes through a
read-only Visualizer session. With more than one desk, first select the source. The imported routes
are only a preview until **Apply** is chosen: taking routes does not replace the show and does not
change the desk. An explicit show input wins over a derived output route for the same logical
universe.

A Visualizer showing the editor's document always receives where these settings say. The
per-universe input pins in the Visualizer's own Quick Settings apply only while it shows a desk or
an opened show file; they are hidden while the editor is the source and never replace the show's
inputs.

Use **Running & Output > Visualizer** to control a connected external Visualizer from the Desk.
The tab is available while a Visualizer is connected. Settings for the Desk's internal Stage view
remain in **Settings**.

## Choose a view

The eight named views are the same ones the Visualizer's own number keys reach, in the
same order:

| | |
| --- | --- |
| **3D Full** | Everything the renderer can do: beams, haze, shadows and bloom |
| **Top Down** | The stage plan |
| **Front → Back** | The elevation from the audience |
| **Left → Right**, **Right → Left** | The elevations from the wings |
| **3D Simple** | A fast 3D picture for previsualization |
| **Back → Front** | The elevation from upstage |
| **3D Lines** | Bodies and aim lines, no beams |

Pressing one sends it. The Visualizer takes the view and frames the rig in it, so **Top Down** shows the framed rig.

**Rendering quality**, Draft, Standard, High or Ultra, is sent the same way. It decides
how much the renderer spends on volumetrics, shadows and bloom, and applies immediately
without the Visualizer reconnecting or reloading the show.

Draft preserves the rig, optics, and laser content at a reduced rendering resolution. Fine model and media detail is softer. Its opaque-scenery shadow budget covers two lights, so other lamps may light through opaque objects. Drapes retain separate blocking checks. Use Ultra or a native 4K image to inspect fine lighting detail.

**3D Lines** shows model-derived edges with depth occlusion, without lighting, shadows, or bloom. Static views stop submitting new GPU work. Standalone Draft and Lines cap steady animation at 60 Hz; Standard, High, and Ultra follow the display rate.

Fresnels have soft field edges; PAR and ACL fields retain a brighter centre and soft falloff. Explicit tungsten and halogen sources also have an estimated heating and cooling response. Choose the installed **Light source** in Show Patch when the profile does not identify it. LEDs and unknown sources respond immediately. This preview behavior does not change DMX values or Cue fade timing, and remains separate from observer persistence.

**Reset physics scenery** is a separate authoritative show-control action. It restores every
released physics body on the selected renderer target to its authored pose. Re-selecting a view,
losing DMX, or reconnecting is not a reset.

At Ultra, the dedicated Visualizer provides independent cloudiness and turbulence controls for lamps and lasers in **Quick Settings**. Cloudiness ranges from even to patchy haze; turbulence controls how quickly it changes. Draft, Standard, and High use uniform fog.

Control's internal Stage has lightweight 2D plan and 3D wireframe views. Open the separate Architect Visualizer for rendered beams, haze, and shadows. Use [native 4K images](90-Advanced/03-snapshots-and-4k-images.md#render-a-4k-image) when you need a full-quality export.

## Desk and local control

A Desk view command selects and frames that view on the connected Visualizer. Between Desk
commands, the local operator can navigate the camera or select a view with the number keys.
Selecting a view again restores its Desk framing.

The local operator can set **Rendering quality** in **Quick Settings**. A local setting is marked
`(local)` in the status bar and is kept until **Follow source** is selected. If the renderer must
reduce the requested quality, Quick Settings shows the active limit and how to restore it.

## Visualizer Camera

Patch **ToskLight Visualizer Camera** when camera position and zoom must be recorded and played
back with DMX. It controls the dedicated external 3D Visualizer only; Stage panes and 2D views
keep their own cameras.

While camera DMX is live, it controls X/Y/Z, Yaw/Pitch/Roll, and Zoom. Local camera navigation
sets **Local camera control**. Press `C` to resume the current DMX pose. If camera input is lost
or the camera is unpatched, the Visualizer holds its last pose and local navigation remains
available.
