# Media Cue Previews

## Purpose

Prove that a Cue containing only ToskLight Media Server content is previewed by the Media Server
that plays it. The preview is the Cue's programmed Program or layer picture, never a Stage
picture, a library thumbnail, or another server's image. Prove also that a single-Cue Cuelist on
a Virtual Playback shows that preview as its default image, while an operator's chosen icon or
image always wins.

## MEDIACUE-001 — Program and layer pictures per Cue, per server, with named fallbacks

Given two discovered 2-layer Media Servers, Rack A and Rack B, patched through **Show Patch ›
Media Servers › Patch suggested**, a Cuelist on playback 201 holds these Cues:

1. **Program A**: Rack A's layer 1 and master
2. **Layer A2**: only Rack A's layer 2
3. **Layer B1**: only Rack B's layer 1
4. **Lighting**: a dimmer
5. **Blank A1**: Rack A's layer 1 with no file
6. **Master B**: Rack B's master
7. **Layer B2**: Rack B's layer 2

- The desk's media-preview index names Cue 1 as Rack A's **Program** on Rack A's patched output.
  Cue 2 is Rack A **layer 2**, Cue 3 is Rack B **layer 1** on Rack B's output, and Cue 6 is
  Rack B's Program. Cue 4 is not a media Cue.
- In a fixed **Cues · Cuelist** pane, Cue 1 shows Rack A's opaque Program picture. Cue 2 shows
  only its layer picture: the transparent corners stay transparent over a checkerboard, and its
  button reads **Open Cue 2 Layer 2 preview**. Cue 3 shows Rack B's picture, never Rack A's.
- Cue 4 asks for no media picture and keeps its Stage preview path.
- Cue 5 reads **Empty media**.
- When Rack B does not answer, Cue 6 reads **Media Server offline** with no image. Touching it
  asks again, and once the server answers it shows the picture.
- When the server has no such output, Cue 7 reads **Media output missing** with no image.
- Touching Cue 2's picture opens it larger, still over the checkerboard.
- Editing Cue 2's file gives it a new preview key. The desk asks for the new picture, and the row
  shows it instead of the previous one.

## MEDIACUE-002 — Single-Cue Virtual Playbacks default to the Cue preview

Given the same two servers, a pinned 1 × 4 **Virtual Playbacks** pane on page 1 holds:

- **1001**: a one-Cue Cuelist addressing Rack A's master
- **1002**: a one-Cue Cuelist addressing only Rack B's layer 1
- **1003**: a one-Cue Cuelist with an image chosen in Playback Configuration
- **1004**: a one-Cue Cuelist addressing Rack B's master while Rack B is offline

Then:

- 1001 and 1002 show their own Cue pictures, marked as the automatic Cue preview. 1002 keeps its
  transparency over a checkerboard.
- 1003 shows the operator's image.
- 1004 names **Media Server offline** and shows no image.
- Adding a second Cue to 1001's Cuelist removes its automatic image; removing that Cue brings it
  back.
- Choosing an icon for 1002 replaces the automatic image with the icon.
- Editing 1001's only Cue changes its preview key and its displayed picture.
