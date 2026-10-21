# Snapshots and Blender

Press **Cmd+S** (Ctrl+S on Windows/Linux) or **File > Take Snapshot** in the Visualizer to capture
the current rig, fixture values and camera. The status line confirms it without interrupting output.
A snapshot stores the scene, not a screenshot.

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
