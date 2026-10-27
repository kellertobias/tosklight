# Snapshots and Blender

A **snapshot** keeps the rig and lighting state currently on screen: press `Cmd`+`S` in the Visualizer, `Ctrl`+`S` away from macOS, or
**File → Take Snapshot**, to capture the current rig.

The Visualizer captures the state immediately and reports the result on the window's bottom status line.

The window's bottom bar lists the shortcuts:

```
Cmd+S snapshot  •  Enter settings  •  Space overlays  •  1-8 views
```

On a narrow window the list shortens from the right. The snapshot is the last one to go.

The snapshot stores the rig: every fixture where it hangs, every head where
it was pointing, the colour and the level the desk had it at, the trusses and the deck around
it, and the camera you were looking through. Capturing does not change the show's output.

## Snapshot location

Each one is a folder of its own, named for the time it was taken and the show it came from,
inside the place your computer keeps application data:

| | |
| --- | --- |
| macOS | `~/Library/Application Support/ToskLight/Visualizer/Snapshots` |
| Windows | `%APPDATA%\ToskLight\Visualizer\Snapshots` |
| Linux | `~/.local/share/tosklight/visualizer/snapshots` |

**Quick Settings** says which folder it is using, under the list of snapshots.

The last twelve are kept. Taking a thirteenth drops the oldest, so preserve a copy elsewhere when it must outlast that limit. Anything else you keep in that folder is left alone.

## Export a snapshot to Blender

The snapshots you have taken are listed in **Quick Settings**, newest first, by the time each
was taken. Press `Enter` on one to export it. Blender is started in the background and writes
a `rig.blend` beside the capture; the row says what it is doing, and says what happened when
it is finished.

Exporting needs Blender installed. You can capture a snapshot without Blender and export it later on the rendering computer.

ToskLight looks for Blender where each platform installs it and on the command search path.
The **Blender** row in Quick Settings says which one it found, or says that it found none. If
you have more than one version, or keep it somewhere unusual, type the path there and that is
the one that will be used.

## Blender export contents

- The rig, in metres, at the placement, pan and tilt of that moment, with each fixture drawn
  from its library model where its profile carries one.
- One spot light per lit head, aimed where the desk aimed it, with its cone, the softness of
  its rim, its colour, and a power that follows the fixture's own output and its zoom, so a
  beam light is brighter than a flood built on the same engine, exactly as it is on stage.
- A volume of hazed air over the rig. A beam is only visible in air that has something in it,
  and this is what makes the export look like the picture it came from.
- Your camera, and Cycles set up to render through it.

The export keeps the desk's placement, aim, colour, and level. Haze density, exposure, and sample count are initial rendering settings at the top of the import script beside the snapshot. Edit them in Blender as needed.

Gobo patterns, prisms and framing shutters do not come across. Those are the Visualizer's own
optics rather than anything in the rig, so a beam arrives as its cone, its colour and its
level. Each snapshot says so in its own `snapshot.json`, along with anything else it could not
carry.

## Use a snapshot without Blender

The geometry is written as `rig.glb`, an ordinary glTF 2.0 file that any 3D application opens,
and `snapshot.json` beside it is plain text: one entry per lit head, with its position, aim,
cone angle, colour, level and patch address. A rig can be taken into another renderer, or read
as a record of what the desk was doing at that moment, without ToskLight or Blender being
involved at all.
