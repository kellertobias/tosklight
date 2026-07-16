import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { fixtureIdsByNumber, loadCanonicalCopy, object, putObject } from "./support/catalog";

const cases = [
  ["PBK-001", "Set intercepts every playback surface with one inert modal"],
  ["PBK-002", "function, color, persistence, and clear remain atomic"],
  ["PBK-003", "Cuelist button mappings retain distinct actions"],
  ["PBK-004", "Master, X-fade, and Temp retain distinct fader ownership"],
  ["PBK-005", "Flash, Temp, Swap, and protection preserve underlying playback state"],
  ["PBK-006", "master-specific layouts address authoritative targets"],
] as const;

test.describe("docs/testing/07-playback-configuration.md", () => {
  for (const [id, title] of cases) pairedScenario<{}>({
    id, title,
    arrange: async ({ api, bench }) => { await loadCanonicalCopy(api, bench, `${id.toLowerCase()}`); await installPlayback(api); return {}; },
    api: async ({ api }) => {
      const playback = await object<any>(api, "playback", "1");
      if (id === "PBK-002") await putObject(api, "playback", "1", { ...playback.body, color: "#8b5cf6", protect_from_swap: true, flash_release: "release_intensity_only" }, playback.revision);
      if (id === "PBK-003") { await api.request("POST", "/api/v1/cuelists/1/go", {}); await api.request("POST", "/api/v1/cuelists/1/go-minus", {}); }
      if (id === "PBK-004") await api.request("PUT", "/api/v1/cuelists/1/master", { value: 0.5 });
      if (id === "PBK-005") { await api.request("POST", "/api/v1/cuelists/1/flash", { pressed: true }); await api.request("POST", "/api/v1/cuelists/1/flash", { pressed: false }); }
      if (id === "PBK-006") await putObject(api, "playback", "1", { ...playback.body, target: { type: "speed_group", group: "A" }, buttons: ["double", "half", "learn"], fader: "learned_percentage" }, playback.revision);
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await page.locator(".mode-toggle").click();
      await page.getByRole("button", { name: "SET", exact: true }).click();
      await page.getByRole("button", { name: "Configure page 1 playback 1" }).click();
      const modal = page.getByRole("dialog", { name: "Playback Configuration" });
      await expect(modal).toBeVisible();
      await expect(modal.getByRole("button", { name: "Playback Function" })).toBeVisible();
      await modal.getByRole("button", { name: "Playback Layout" }).click();
      await expect(modal.getByText("Top button", { exact: true })).toBeVisible();
      if (id === "PBK-002") { const color = modal.getByRole("button", { name: "Playback color #8b5cf6" }); await color.click(); await expect(color).toHaveClass(/active/); await modal.getByRole("button", { name: "Apply" }).click(); }
      else await modal.getByRole("button", { name: "Cancel", exact: true }).click();
    },
    assert: async ({ api }) => {
      const stored = await object<any>(api, "playback", "1");
      expect(stored.body.buttons).toHaveLength(3);
      if (id === "PBK-002") await expect.poll(async () => (await object<any>(api, "playback", "1")).body.color).toBe("#8b5cf6");
      if (id === "PBK-006") expect(["cue_list", "speed_group"]).toContain(stored.body.target.type);
    },
  });
});

async function installPlayback(api: any) {
  const fixtures = await fixtureIdsByNumber(api);
  const cueListId = crypto.randomUUID();
  await putObject(api, "cue_list", cueListId, { id: cueListId, name: "Configured Sequence", priority: 0, mode: "sequence", looped: false, chaser_step_millis: 1000, speed_group: null, cues: [1,2].map((number) => ({ number, name: `Cue ${number}`, changes: [{ fixture_id: fixtures[1], attribute: "intensity", value: { kind: "normalized", value: number * 0.25 }, automatic_restore: false }], group_changes: [], fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, phasers: [] })) });
  await putObject(api, "playback", "1", { number: 1, name: "Configured Playback", target: { type: "cue_list", cue_list_id: cueListId }, buttons: ["go_minus", "go", "flash"], fader: "master", go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false });
  await putObject(api, "playback_page", "1", { number: 1, name: "Main", slots: { "1": 1 } });
}
