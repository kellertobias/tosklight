# Crowd Areas

The shipped **Venue — Crowd Area** fixture puts an audience into the show as one visual-only
scenery fixture. It has no DMX address. Choose one of its nine modes: Sitting, Standing still, or
Dancing, each at Sparse, Medium, or Dense authored density.

In **Show Patch**, set **Footprint width** and **Footprint depth** independently. These values are
stored with the Stage layout in metres and draw the same rectangular footprint in plan views that
the 3D Visualizer populates with flat, double-sided black people and contrasting outlines from the
shipped audience artwork. Height is never scaled with the footprint: Sitting uses seated human
height, while Standing still and Dancing use standing human height.

Population is deterministic. The fixture's stable show identity, selected mode, authored
footprint, and density produce the same people after saving, reopening, or restarting the
Visualizer. Moving another fixture does not reshuffle them. Changing mode or footprint produces a
new deterministic layout contained within the rectangle and on its floor plane.

Draft and Standard omit crowds. High draws up to 384 people per frame, and Ultra draws up to 768
with a fixed budget. Extreme starts at 1,024 and follows the renderer's measured 16 ms GPU
adaptation ladder down with render scale when necessary.
When an authored audience exceeds the current budget, the Visualizer retains a stable subset and
reports authored and drawn counts in its benchmark output instead of destabilizing frame time.

**Crowd amount** in the Visualizer's local Quick Settings applies from 0% to 100% to every Crowd
Area in that window. It changes only how many authored people that renderer draws; it never rewrites
the show, mode, footprint, or stable identity.
