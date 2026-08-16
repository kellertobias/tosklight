# Snapshots and Blender

A look that works is usually a look that is about to change. A **snapshot** keeps the one
that is on screen: press `Cmd`+`S` in the Visualizer — `Ctrl`+`S` away from macOS, or
**File → Take Snapshot** — and the rig is frozen exactly as it stands.

Nothing is asked and nothing opens. A dialog would take the moment the snapshot is for, so
the Visualizer takes it first and tells you afterwards, on the status line at the bottom of
the window.

The bar along the bottom of the window carries the shortcut, so it is there when you need it
without going looking:

```
Cmd+S snapshot  •  Enter settings  •  Space overlays  •  1-8 views
```

On a narrow window the list shortens from the right. The snapshot is the last one to go.

What is kept is the rig, not a picture of it: every fixture where it hangs, every head where
it was pointing, the colour and the level the desk had it at, the trusses and the deck around
it, and the camera you were looking through. Nothing on stage is disturbed and no frame is
dropped, on a rig of any size.

## Where snapshots go

Each one is a folder of its own, named for the time it was taken and the show it came from,
inside the place your computer keeps application data:

| | |
| --- | --- |
| macOS | `~/Library/Application Support/ToskLight/Visualizer/Snapshots` |
| Windows | `%APPDATA%\ToskLight\Visualizer\Snapshots` |
| Linux | `~/.local/share/tosklight/visualizer/snapshots` |

**Quick Settings** says which folder it is using, under the list of snapshots.

The last twelve are kept. Taking a thirteenth drops the oldest, so the shortcut can be used as
freely as it deserves to be. Anything else you keep in that folder is left alone.

## Turning one into a Blender file

The snapshots you have taken are listed in **Quick Settings**, newest first, by the time each
was taken. Press `Enter` on one to export it. Blender is started in the background and writes
a `rig.blend` beside the capture; the row says what it is doing, and says what happened when
it is finished.

Exporting needs Blender installed. Taking a snapshot does not — which is the point of the two
being separate. Keep the moment on the machine beside the stage, make the file later on the
machine you render with.

ToskLight looks for Blender where each platform installs it and on the command search path.
The **Blender** row in Quick Settings says which one it found, or says that it found none. If
you have more than one version, or keep it somewhere unusual, type the path there and that is
the one that will be used.

## What arrives in Blender

- The rig, in metres, at the placement, pan and tilt of that moment, with each fixture drawn
  from its library model where its profile carries one.
- One spot light per lit head, aimed where the desk aimed it, with its cone, the softness of
  its rim, its colour, and a power that follows the fixture's own output and its zoom — so a
  beam light is brighter than a flood built on the same engine, exactly as it is on stage.
- A volume of hazed air over the rig. A beam is only visible in air that has something in it,
  and this is what makes the export look like the picture it came from.
- Your camera, and Cycles set up to render through it.

The placement, the aim, the colour and the level are the desk's, and they are exact. The haze
density, the exposure and the number of samples are a starting look: they are named at the top
of the import script that is written beside the snapshot, and everything they produce is
ordinary Blender data you can edit by hand.

Gobo patterns, prisms and framing shutters do not come across. Those are the Visualizer's own
optics rather than anything in the rig, so a beam arrives as its cone, its colour and its
level. Each snapshot says so in its own `snapshot.json`, along with anything else it could not
carry.

## Using the snapshot without Blender

The geometry is written as `rig.glb`, an ordinary glTF 2.0 file that any 3D application opens,
and `snapshot.json` beside it is plain text: one entry per lit head, with its position, aim,
cone angle, colour, level and patch address. A rig can be taken into another renderer, or read
as a record of what the desk was doing at that moment, without ToskLight or Blender being
involved at all.
