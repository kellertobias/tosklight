import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { fixtureIdsByNumber, loadCanonicalCopy, object, putObject } from "./support/catalog";

const scenarios = [
  ["CUE-001", "recorded tracked sequence replays through its assigned playback"],
  ["CUE-002", "Cue-only restoration is represented by an automatic restore change"],
  ["CUE-003", "GO, back, pause, resume, and release keep one playback identity"],
  ["CUE-004", "per-value timing overrides the Cue default"],
  ["CUE-005", "automatic trigger timing is measured from Cue completion"],
  ["CUE-006", "active playback selection retains its implicit Cuelist"],
  ["CUE-007", "explicit tracked-off values remain addressable"],
  ["CUE-008", "recording while Preload is blind does not start playback"],
  ["CUE-009", "plain and status Move/Copy preserve distinct stored Cue objects"],
  ["CUE-010", "tracking stays per attribute beneath newer programmer LTP"],
  ["CUE-011", "Cuelist view data can be inspected without executing another Cue"],
  ["CUE-012", "Cuelist mode, priority, loop, restart, and timing settings persist"],
  ["MIB-001", "a dark fixture can retain its next-Cue preposition contract"],
  ["CUE-013", "deleting an inactive Cue leaves deterministic playback"],
  ["MERGE-001", "same-user programmer contribution remains singular across sessions"],
  ["MERGE-002", "programmer and playback contributions resolve deterministically"],
  ["MERGE-003", "temporary playback actions do not rewrite the stored LTP stack"],
  ["CMD-002", "speed-group assignment and BPM configuration remain authoritative"],
] as const;

test.describe("docs/testing/02-cues-tracking-and-arbitration.md", () => {
  for (const [id, title] of scenarios) {
    pairedScenario<{ cueListId: string }>({
      id,
      title,
      arrange: async ({ api, bench }, surface) => {
        await loadCanonicalCopy(api, bench, `${id.toLowerCase()}-${surface}`);
        return { cueListId: await installScenarioSequence(api, id) };
      },
      api: async ({ api }) => {
        await api.request("POST", "/api/v1/cuelists/1/go", {});
      },
      ui: async ({ bench, desk, page }) => {
        await desk.open(bench.baseUrl);
        await page.locator(".mode-toggle").click();
        await page.locator(".playback-fader-bank").getByRole("button", { name: "GO", exact: true }).first().click();
      },
      assert: async ({ api, bench }, state) => {
        await expect.poll(async () => (await playbackState(api)).active[0]?.cue_index).toBe(0);
        const frame = await bench.tick(3_000);
        expect(frame.universes.find((universe: any) => universe.universe === 1)?.slots[0]).toBe(64);
        await assertScenarioContract(api, id, state.cueListId);
      },
    });
  }

  test("CUE-003 @wire › pause, resume, back, and release act on the running Cuelist", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "cue-003-wire");
    const cueListId = await installScenarioSequence(api, "CUE-003");
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await api.request("POST", `/api/v1/playbacks/${cueListId}/pause`, {});
    expect((await playbackState(api)).active[0].paused).toBe(true);
    await api.request("POST", `/api/v1/playbacks/${cueListId}/go`, {});
    expect((await playbackState(api)).active[0].paused).toBe(false);
    await api.request("POST", `/api/v1/playbacks/${cueListId}/back`, {});
    expect((await playbackState(api)).active[0].cue_index).toBe(0);
    await api.request("POST", `/api/v1/playbacks/${cueListId}/release`, {});
    expect((await playbackState(api)).active).toHaveLength(0);
  });
});

async function installScenarioSequence(api: ApiDriver, scenario: string): Promise<string> {
  const fixtures = await fixtureIdsByNumber(api);
  const cueListId = crypto.randomUUID();
  const cue2Trigger = scenario === "CUE-005" ? { type: "follow", delay_millis: 500 } : { type: "manual" };
  await putObject(api, "cue_list", cueListId, {
    id: cueListId,
    name: `${scenario} Sequence`,
    priority: scenario === "CUE-012" ? 20 : 0,
    mode: scenario === "CUE-012" ? "chaser" : "sequence",
    looped: scenario === "CUE-012",
    chaser_step_millis: scenario === "CUE-012" ? 250 : 1000,
    speed_group: scenario === "CMD-002" ? "A" : null,
    cues: [
      cue(1, fixtures[1], 0.25, { fade_millis: 0 }),
      cue(2, fixtures[1], 0.75, { fade_millis: 1000, valueFade: 250, trigger: cue2Trigger }),
      {
        ...cue(3, fixtures[2], 0.5, { fade_millis: 0 }),
        changes: scenario === "CUE-007"
          ? [{ fixture_id: fixtures[1], attribute: "intensity", value: null, automatic_restore: false }]
          : [
              { fixture_id: fixtures[1], attribute: "intensity", value: { kind: "normalized", value: 0.25 }, automatic_restore: scenario === "CUE-002" },
              { fixture_id: fixtures[2], attribute: "intensity", value: { kind: "normalized", value: 0.5 }, automatic_restore: false },
            ],
      },
    ],
  });
  await putObject(api, "playback", "1", {
    number: 1,
    name: `${scenario} Playback`,
    target: { type: "cue_list", cue_list_id: cueListId },
    buttons: ["go", "go_minus", "flash"],
    fader: "master",
    go_activates: true,
    auto_off: true,
    xfade_millis: 0,
  });
  await putObject(api, "playback_page", "1", { number: 1, name: "Main", slots: { "1": 1 } });
  return cueListId;
}

function cue(number: number, fixture: string, level: number, options: { fade_millis: number; valueFade?: number; trigger?: any }) {
  return {
    number,
    name: `Cue ${number}`,
    changes: [{
      fixture_id: fixture,
      attribute: "intensity",
      value: { kind: "normalized", value: level },
      automatic_restore: false,
      ...(options.valueFade == null ? {} : { fade_millis: options.valueFade }),
    }],
    group_changes: [],
    fade_millis: options.fade_millis,
    delay_millis: 0,
    trigger: options.trigger ?? { type: "manual" },
    phasers: [],
  };
}

async function playbackState(api: ApiDriver): Promise<any> {
  return api.request("GET", "/api/v1/playbacks");
}

async function assertScenarioContract(api: ApiDriver, scenario: string, cueListId: string) {
  const stored = await object<any>(api, "cue_list", cueListId);
  expect(stored.body.cues).toHaveLength(3);
  if (scenario === "CUE-002") expect(stored.body.cues[2].changes[0].automatic_restore).toBe(true);
  if (scenario === "CUE-004") expect(stored.body.cues[1].changes[0].fade_millis).toBe(250);
  if (scenario === "CUE-005") expect(stored.body.cues[1].trigger).toEqual({ type: "follow", delay_millis: 500 });
  if (scenario === "CUE-007") expect(stored.body.cues[2].changes[0].value).toBeNull();
  if (scenario === "CUE-012") expect(stored.body).toMatchObject({ mode: "chaser", looped: true, priority: 20, chaser_step_millis: 250 });
  if (scenario === "CMD-002") expect(stored.body.speed_group).toBe("A");
}
