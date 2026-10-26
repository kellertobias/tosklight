# Snapshots and 4K Images

Press **Cmd+S** (Ctrl+S on Windows/Linux) or **File > Take Snapshot** in the Visualizer to capture
the current rig, fixture values and camera. The status line confirms it without interrupting output.
A snapshot stores the scene, not a screenshot.

## Render a 4K image

In **ToskLight Architect**, open the native 3D window with **Open Viz**. In that window, choose
**File > Render 4K Image**, or the rendering row in **Quick Settings**. With the 3D window
focused, press **Cmd+Shift+R** on macOS or **Ctrl+Shift+R** on Windows/Linux. The shortcut is
inactive while entering text or choosing in a modal dialog. Cmd/Ctrl+S keeps its existing
scene-snapshot behavior. The Light Desk's embedded Stage has no image-export action.

The action freezes Architect's current camera, resolved fixture values, crowd amount, Detailed people
preference and available media pixels. It renders a new **3840 × 2160 PNG** with the regular
ToskLight renderer at full native Ultra quality. It does not enlarge the live pane's raster.
The camera keeps its vertical field of view with 16:9 framing, so the image can show more or less
at the sides than a differently shaped pane. An orthographic view stays orthographic.
A 3D Lines invocation renders the same camera with full lighting.

In Architect's **Quick Settings**, choose **Fog amount**, **Ambient**, **Exposure** and
**Detailed people** before rendering. The image freezes those preferences, including Detailed
people when the live quality omits them. A Draft or Lines preview keeps its existing quality.
Moving the rig or changing preferences after invocation does not change the captured moment.

Architect freezes the latest available decoded media pixels. An expired feed or a capture made
before the first decoded frame is identified explicitly. The image uses only the frozen known
fallback or black, never a later server frame.

Progress, cancellation, failures and the saved path appear in the view. After success, use **Open**
or **Reveal**. Another image can start when the current job and its cleanup finish. Cancellation
cannot interrupt GPU work already submitted, so cleanup can take time. Images are saved in the
Visualizer **Images** folder beside **Snapshots** and are not deleted by the twelve-snapshot
retention rule below. Image rendering needs graphics resources while the job is active.

## Find a snapshot

**Quick Settings** lists the newest twelve snapshots and their folder. Selecting a snapshot and
pressing **Enter** reports its saved folder. Taking another removes the oldest snapshot; copy any
you want to keep elsewhere first.

| Platform | Default snapshot folder |
| --- | --- |
| macOS | `~/Library/Application Support/ToskLight/Visualizer/Snapshots` |
| Windows | `%APPDATA%\ToskLight\Visualizer\Snapshots` |
| Linux | `~/.local/share/tosklight/visualizer/snapshots` |

Each snapshot contains `rig.glb` for 3D geometry exchange and `snapshot.json` for the resolved
scene values, including each lit head's placement, aim, colour, level, cone and patch address.
