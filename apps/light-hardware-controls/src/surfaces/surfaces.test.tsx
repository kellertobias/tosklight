import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { initialFeedbackState } from "../controller/types";
import { GridSurface } from "./GridSurface";
import { PlaybackSurface } from "./PlaybackSurface";
import { ProgrammerSurface } from "./ProgrammerSurface";
import { SettingsSurface } from "./SettingsSurface";

const send = () => undefined;

describe("hardware controller surfaces", () => {
  it("keeps the playback and programmer console labels", () => {
    const playback = renderToStaticMarkup(
      <PlaybackSurface
        topRowVisible
        levels={{}}
        lamps={{}}
        send={send}
      />,
    );
    const programmer = renderToStaticMarkup(
      <ProgrammerSurface
        updateArmed
        lamps={{}}
        highlight={initialFeedbackState.highlight}
        send={send}
      />,
    );

    expect(playback).toContain("Encoder 1 up");
    expect(playback).toContain(">21<");
    expect(playback).toContain("FADER");
    expect(programmer).toContain("UPDATE");
    expect(programmer).toContain("PRELOAD GO");
    expect(programmer).toContain("HIGH");
    expect(programmer).toContain("Prog Fade");
    expect(programmer).toContain("Cue Fade");
  });

  it("keeps the expanded grid and speed-group surfaces", () => {
    const markup = renderToStaticMarkup(
      <GridSurface levels={{}} lamps={{}} speedBpms={{}} send={send} />,
    );

    expect(markup).toContain(">41<");
    expect(markup).toContain(">90<");
    expect(markup).toContain("Playbacks 91–96");
    expect(markup).toContain("Speed groups");
    expect(markup).toContain("120 BPM");
  });

  it("keeps the OSC settings wording and reconnect action", () => {
    const markup = renderToStaticMarkup(
      <SettingsSurface
        connected
        settings={{ host: "light.local", port: 9000, desk: "wing", top: true }}
        updateSettings={() => undefined}
        connect={async () => undefined}
      />,
    );

    expect(markup).toContain("OSC connection");
    expect(markup).toContain("Save and reconnect");
    expect(markup).toContain("Connected to wing on light.local:9000");
  });
});
