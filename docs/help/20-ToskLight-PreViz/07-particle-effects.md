# Particle Effects

Patch flame units and cold-spark fountains as **Effect** fixtures and program their actual DMX
mode. A compatible fixture package supplies the visual behavior. Check the selected mode and its
trigger, release and Off values against the device's manual.

The shipped **Generic Cold Spark Fountain**, **Generic Flame Jet** and **Generic Five-nozzle Flame
Unit** are reference fixtures. They demonstrate intensity, fountain height and one or several
nozzles; their charts are not manufacturer hardware personalities.

Scenery hides flame and spark particles behind it. The particles glow but do not cast dynamic
light or shadows. Higher rendering qualities allow denser effects; if necessary, the renderer
reduces particle density to maintain performance.

After a renderer restart, particles restart from the current DMX state rather than reconstructing
every particle that was previously in the air. If a package's effect fails, that fixture turns off
in the preview and reports its reason; other fixtures continue.
