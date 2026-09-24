# Exclusive Color Playbacks

## Purpose

Prove that a small set of colour Cues on Virtual Playbacks can be run as a ritual: at most one
colour is on at a time, each one colours the whole mixed rig from shared Color presets, and
turning the winner off brings every fixture back to its resting colour without starting another.

Fixtures used throughout, as hung at the Südbahnhof:

- **Sunstrip 1**: `Showtec › Sunstrip LED RGB 42206`, mode `30 Channel` (ten RGB pixels, virtual
  intensity)
- **PAR L 2** and **PAR R 3**: `Generic › Dimmer RGB Control PAR` (RGB, virtual intensity)
- **Wash 4**: `ROBE › Robin 300 LEDWash`, mode `Mode 3` (RGBW mover with a dimmer)

Five Color presets hold Red, Green, Blue, Amber (red with three-quarter green), and White. White
also sets the mover's white emitter. Each of the other presets sets it to zero, so the
mover shows the pure colour.

## BENCH-RITUAL-COLOR-001 — One colour at a time, and back to rest

Automated in `tests/113-semantic-exclusive-color-playbacks.spec.ts`.

1. Store Group 1 with all four fixtures.
2. Store the five Color presets for the group.
3. For each preset, record one Cue on its own Playback with the group at full on that preset.
   Name each Playback after its colour and give it a single Toggle button.
4. Assign the five Playbacks to Virtual Playback cells 1–5 and put all five in one exclusion zone.
5. Press each cell in turn. After each press:
   - only that Virtual Playback is On;
   - the Sunstrip's first and last pixel, both PARs and the mover all show that colour on their
     DMX channels;
   - the mover is at full intensity.
6. Press Green again. Green takes over and White goes off.
7. Press Green once more. No colour is On, and nothing else has started. The Sunstrip and PARs
   rest at full white (255/255/255, their virtual intensity default). The mover rests at
   Intensity 0 with its RGBW channels at 255.

### Show check at the Südbahnhof

Run the same five colours from the show's own Virtual Playback page on the real rig. Confirm:

- each press changes the whole rig to that colour in one step;
- the previous colour's cell goes dark;
- pressing the lit cell again leaves no colour playing, and the bars and PARs return to their
  resting look.
