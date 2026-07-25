import type { ApiDriver } from "../apps/control-ui/e2e/bench/core/api";
import { expect, test } from "../apps/control-ui/e2e/bench/core/fixtures";
import { fixtureIdsByNumber, loadCanonicalCopy, putObject } from "./support/catalog";

// Maintainer requirement (docs/engineering/api-rules.md §1: volatile state is pushed,
// not polled — telemetry design decided 2026-07-23): fast-changing playback
// runtime values are sampled server-side on a render-frame divider nearest ~10 Hz (44 Hz
// output → every 4th frame ≈ 11 Hz) and pushed as delta ticks on the v2 events lane; the
// client renders them from a retained store without polling.
test("TELEMETRY-001 @supplemental-ui › playback fades stream ~10 Hz delta samples without polling", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "playback-telemetry", "compact-rig");
  const fixtures = await fixtureIdsByNumber(api);
  await installFadingCuelist(api, 1, "Telemetry A", fixtures[1], 4_000);
  await installFadingCuelist(api, 2, "Telemetry B", fixtures[2], 4_000);

  const telemetryTicks: Array<{ frame: number; sample_rate_hz: number; samples: any[] }> = [];
  const snapshotRequests: string[] = [];
  page.on("websocket", (socket) => {
    if (!socket.url().includes("/api/v2/events")) return;
    socket.on("framereceived", (frame) => {
      const payload = typeof frame.payload === "string" ? frame.payload : frame.payload.toString();
      try {
        const message = JSON.parse(payload);
        if (message?.event?.payload?.type === "playback_telemetry_sampled")
          telemetryTicks.push(message.event.payload.tick);
      } catch {
        // Non-JSON frames are not telemetry.
      }
    });
  });
  page.on("request", (request) => {
    if (request.url().includes("/playback-runtime/snapshot")) snapshotRequests.push(request.url());
  });

  await desk.open(bench.baseUrl);
  await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
  const hydrationSnapshots = snapshotRequests.length;

  await api.playbackNumberAction(1, "go", {});
  await api.playbackNumberAction(2, "go", {});

  // One simulated second of render frames at the configured 44 Hz output rate. The frame
  // divider (4) must produce ~11 sampled ticks while the fades progress.
  for (let frame = 0; frame < 44; frame += 1) await bench.tick(23);
  await expect.poll(() => telemetryTicks.length, { timeout: 10_000 }).toBeGreaterThanOrEqual(8);

  expect(telemetryTicks.length).toBeLessThanOrEqual(13);
  for (const tick of telemetryTicks) expect(tick.sample_rate_hz).toBe(11);

  const progressFor = (playback: number) =>
    telemetryTicks
      .flatMap((tick) => tick.samples)
      .filter((sample) => sample.playback_number === playback)
      .map((sample) => sample.fade_progress as number);
  for (const playback of [1, 2]) {
    const progress = progressFor(playback);
    expect(progress.length).toBeGreaterThanOrEqual(8);
    for (const value of progress) {
      expect(value).toBeGreaterThan(0);
      expect(value).toBeLessThanOrEqual(1);
    }
    for (let index = 1; index < progress.length; index += 1)
      expect(progress[index]).toBeGreaterThanOrEqual(progress[index - 1]);
    // A 4-second fade sampled across one simulated second stays mid-fade: these are genuine
    // intermediate progress samples, not just the settled end state.
    expect(progress[0]).toBeLessThan(0.5);
  }

  // The samples arrived as pushed deltas: no runtime snapshot polling happened after the
  // initial hydration.
  expect(snapshotRequests.length).toBe(hydrationSnapshots);

  // Once the fades settle, unchanged sweeps stop producing ticks entirely.
  for (let frame = 0; frame < 176; frame += 1) await bench.tick(23);
  const settled = telemetryTicks.length;
  for (let frame = 0; frame < 44; frame += 1) await bench.tick(23);
  expect(telemetryTicks.length).toBe(settled);
  expect(snapshotRequests.length).toBe(hydrationSnapshots);
});

async function installFadingCuelist(
  api: ApiDriver,
  playback: number,
  name: string,
  fixture: string,
  fadeMillis: number,
): Promise<void> {
  const id = crypto.randomUUID();
  await putObject(api, "cue_list", id, {
    id,
    name,
    priority: 0,
    mode: "sequence",
    looped: false,
    chaser_step_millis: 1_000,
    speed_group: null,
    intensity_priority_mode: "htp",
    wrap_mode: "off",
    restart_mode: "first_cue",
    force_cue_timing: false,
    disable_cue_timing: false,
    chaser_xfade_percent: 0,
    speed_multiplier: 1,
    cues: [
      {
        id: crypto.randomUUID(),
        number: 1,
        name: "Cue 1",
        fade_millis: fadeMillis,
        delay_millis: 0,
        trigger: { type: "manual" },
        changes: [
          {
            fixture_id: fixture,
            attribute: "intensity",
            value: { kind: "normalized", value: 1 },
            automatic_restore: false,
          },
        ],
        group_changes: [],
        phasers: [],
      },
    ],
  });
  await putObject(api, "playback", String(playback), {
    number: playback,
    name: `${name} Playback`,
    target: { type: "cue_list", cue_list_id: id },
    buttons: ["go_minus", "go", "flash"],
    fader: "master",
    go_activates: true,
    auto_off: false,
    xfade_millis: 0,
    color: "#20c997",
    flash_release: "release_all",
    protect_from_swap: false,
  });
}
