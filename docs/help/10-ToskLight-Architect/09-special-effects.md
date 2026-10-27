# Special effects

Lasers, flame units, cold-spark fountains, and releasable scenery can be part of the visualized rig. Patch the matching fixture profile, place it, and drive it through Architect's preview or live DMX. The fixture package determines how its channels appear in the Visualizer.

## Flame and spark fixtures

The Generic Cold Spark Fountain exposes intensity, fountain height, and visible spark lifetime. Generic Flame Jet shows a single nozzle; Generic Five-nozzle Flame Unit shows five positioned nozzles. These are reference profiles. Use a manufacturer profile with the real device's chart for a production patch.

Flames and sparks are visible geometry: scenery in front hides them, and bright particles contribute to bloom. They do not cast light or shadows. A renderer restart starts particles from current DMX rather than restoring individual particles.

## Lasers

A laser profile needs its own scan engine to reproduce its patterns. A laser without that script stays dark, and the Visualizer reports why. Set the fixture's installed position, rotation, and bracket angle to aim the projector. Its position channels usually move the pattern inside the scan field.

The visible figure is the scanned path on the stage floor. Haze makes the beam between the aperture and figure visible. Walls and scenery stop the beam, but the Visualizer does not draw its pattern on those surfaces. Plan views show fixtures and their aim without laser figures or beams.

**Laser brightness** in Quick Settings scales all laser figures and beams from zero to four times their built-in strength. It belongs to this renderer on this computer. It does not change the show's DMX.

## Choose a laser pattern

The supplied **Visualizer Laser** profile adds four **Control** choices: **Standard patterns**
uses the normal pattern bank, **Single point** holds one beam, and **Segmented line** draws a
horizontal line with blank gaps. **Dotted line** spreads separate stationary beams across a line.
**Pattern size** sets the line width within the configured
scan angle. Use Color for white, cyan, or another supported colour.

Aim a single beam with the installed rotation and Bracket angle. Changing its scan position
moves the point within the projector's scan field.

![Laser figure, blanked travel, and beam through haze](../assets/diagrams/laser-scan.svg)

## Persistence of vision

Quick Settings controls how long flashes remain visible in the rendered picture. These settings affect lamps and strobes as well as lasers, and stay on this computer.

**Persistence of vision** sets the time for full brightness to decay to black. A tenth of a second approximates an eye's persistence. At zero, each frame shows only the current fixture state. A light rising in level appears immediately.

**Persistence falloff** shapes that decay. At `1.00`, it is linear. Above `1.00`, the bright beginning stays visible while the dim tail fades faster. The default is `2.00`.

Decay uses full brightness as its reference, so a half-brightness flash fades out in half the time of a full-brightness flash. Strobe rendering also accounts for the fraction of a frame during which the shutter was open.

## Physics scenery

The Generic Kabuki Curtain demonstrates a released scenic body. Its single channel uses `0-31` for Reset, `32-191` for Hold, and `192-255` for Release. Use the manufacturer profile's own values for another device.

Release latches. The body continues falling if DMX disappears, the renderer reconnects, or Release remains selected. Use explicit Reset or **Running & Output > Visualizer > Reset physics scenery** to restore its authored pose.

Released bodies fall against the authored floor and settle with their lower face on it. High and Ultra may also include collisions with static scenery. Body-to-body collision depends on the profile. Restart restores the recorded release time or settled result; a backwards timecode seek reconstructs the release/reset state.

Fixture authors can use the [Architect scripting reference](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/operator-manual-architect-scripting.md) to package scan engines, particle emitters, or physics actions.
