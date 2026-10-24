# Snapshots and Blender

Press **Cmd+S** (Ctrl+S on Windows/Linux) or **File > Take Snapshot** in the Visualizer to capture
the current rig, fixture values and camera. The status line confirms it without interrupting output.
A snapshot stores the scene, not a screenshot.

## Render a 4K image

Use **Render 4K** in a Control Stage pane, or **File > Render 4K Image** / the rendering row in
Visualizer **Quick Settings**. With that view focused, press **Cmd+Shift+R** on macOS or
**Ctrl+Shift+R** on Windows/Linux. The shortcut is inactive while entering text, using the Desk
command line, or choosing in a modal dialog. Cmd/Ctrl+S keeps its existing scene-snapshot behavior.

The action freezes that pane's camera, Live or Follow Preload values, crowd amount, and available
media pixels before checking Blender or asking for quality. Moving the rig while the choice is open does not
change the captured moment. Both engines produce a new **3840 × 2160 PNG**; they do not enlarge
the live pane's raster. The camera keeps its vertical field of view with 16:9 framing, so the image
can show more or less at the sides than a differently shaped pane. An orthographic view stays
orthographic. A 3D Lines invocation renders the same camera with full lighting.

When a usable Blender is installed, choose:

- **Ultra quality — regular ToskLight renderer:** a separate full-quality native render.
- **Ultimate quality — Blender:** ToskLight launches a managed Cycles render and saves the final
  image. It can take substantially longer. Its physical lighting and AgX color transform can
  differ from the native preview. Any unsupported frozen feature is listed beside the choice;
  choose Ultra when Ultimate cannot represent that look.

Ultimate currently uses plain materials for aluminium, multiplex and fabric finishes, omits the
floor grid and selection guides, and does not reproduce native bloom. Beam softness, fog and
indirect lighting can also look different. For 2D plans or active laser scans, choose Ultra; the
Ultimate choice explains why it is unavailable.

Without Blender, the action uses Ultra. A broken configured installation reports its problem and
keeps Ultra available. Rendering does not change the live quality preference: Draft remains Draft.
Missing media frames are identified explicitly; the image uses only the frozen known fallback or
black, never a later server frame substituted after the choice.

Progress, cancellation, failures and the saved path appear in the view. After success, use **Open**
or **Reveal**. Another image can start when the current job and its cleanup finish. Cancellation
cannot interrupt GPU work already submitted, so cleanup can take time. Images are saved in the
Visualizer **Images** folder beside **Snapshots** and are not deleted by the twelve-snapshot
retention rule below.

Offline Blender images use at most eight CPU threads and no more than half the available CPU
threads, with a minimum of one. ToskLight lowers the new Blender process's CPU priority where
available. This preserves image quality and leaves more capacity for the desk, but does not
guarantee output deadlines under resource contention.

## Find a snapshot

**Quick Settings** lists the newest twelve snapshots and their folder. Taking another removes the
oldest snapshot; copy any you want to keep elsewhere first.

| Platform | Default snapshot folder |
| --- | --- |
| macOS | `~/Library/Application Support/ToskLight/Visualizer/Snapshots` |
| Windows | `%APPDATA%\ToskLight\Visualizer\Snapshots` |
| Linux | `~/.local/share/tosklight/visualizer/snapshots` |

## Export to Blender

1. Install Blender. Taking the snapshot itself does not require it.
2. In **Quick Settings**, check the **Blender** path or enter your installation's path.
3. Select a snapshot and press **Enter**. Watch the row for progress or an error.
4. Open the resulting `rig.blend` beside the snapshot.

The export includes the posed rig in metres, lit heads with colour/level/cone, haze, your camera
and a Cycles starting setup. Adjust exposure, haze and sampling in Blender for the final image.
Gobos, prisms and framing shutters do not transfer. Read the snapshot's listed limitations.

For another 3D application, use `rig.glb`. `snapshot.json` records each lit head's placement, aim,
colour, level, cone and patch address.
