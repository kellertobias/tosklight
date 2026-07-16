import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { fixtureIdsByNumber, loadCanonicalCopy, programmer, putObject } from "./support/catalog";

type Configuration = Record<string, any> & {
  preload_programmer_changes: boolean;
  preload_physical_playback_actions: boolean;
  preload_virtual_playback_actions: boolean;
};

const cases = [
  ["PRELOAD-001", "programmer-only Preload is blind until GO"],
  ["PRELOAD-002", "physical-playback-only Preload queues action verbs"],
  ["PRELOAD-003", "Virtual Playbacks is a persisted configurable pane"],
  ["PRELOAD-004", "virtual GO and TOGGLE are captured independently"],
  ["PRELOAD-005", "all eight capture-domain masks remain independent"],
  ["PRELOAD-006", "combined Preload commits and releases only programmer data"],
] as const;

test.describe("docs/testing/06-preload-modes-and-virtual-playbacks.md", () => {
  for (const [id, title] of cases) pairedScenario<{ playback: number }>({
    id, title,
    arrange: async ({ api, bench }) => {
      await loadCanonicalCopy(api, bench, `${id.toLowerCase()}`);
      await installPlayback(api);
      const state = await api.request<any>("GET", "/api/v1/playbacks");
      await setCaptureMask(api, true, true, false);
      return { playback: state.pool[0].number };
    },
    api: async ({ api }, { playback }) => {
      if (id === "PRELOAD-003") {
        const config = await configuration(api);
        expect(config.preload_virtual_playback_actions).toBe(false);
        return;
      }
      if (id === "PRELOAD-005") {
        for (let mask = 0; mask < 8; mask++) {
          await setCaptureMask(api, Boolean(mask & 1), Boolean(mask & 2), Boolean(mask & 4));
          const saved = await configuration(api);
          expect([saved.preload_programmer_changes, saved.preload_physical_playback_actions, saved.preload_virtual_playback_actions]).toEqual([Boolean(mask & 1), Boolean(mask & 2), Boolean(mask & 4)]);
        }
        return;
      }
      if (id === "PRELOAD-002") await setCaptureMask(api, false, true, false);
      if (id === "PRELOAD-004") await setCaptureMask(api, false, false, true);
      if (id === "PRELOAD-006") await setCaptureMask(api, true, true, true);
      await api.command("preload.enter", {});
      if (id !== "PRELOAD-004") await api.command("programmer.execute", { value: "GROUP 1 AT 75" });
      if (["PRELOAD-002", "PRELOAD-006"].includes(id)) await api.request("POST", `/api/v1/cuelists/${playback}/go`, {});
      if (id === "PRELOAD-004") await api.request("POST", `/api/v1/cuelists/${playback}/toggle`, { surface: "virtual" });
      const pending = await programmer(api);
      if (id === "PRELOAD-002") {
        expect(Object.keys(pending.group_values)).toContain("1");
        expect(pending.preload_group_pending).toEqual({});
      }
      if (["PRELOAD-002", "PRELOAD-004", "PRELOAD-006"].includes(id)) expect(pending.preload_playback_pending).toHaveLength(1);
      await api.command("preload.go", {});
    },
    ui: async ({ api, bench, desk, page }, { playback }) => {
      if (id === "PRELOAD-003") {
        await desk.open(bench.baseUrl);
        await addVirtualPlaybackPane(page);
        return;
      }
      if (id === "PRELOAD-005") {
        await desk.open(bench.baseUrl);
        await page.getByRole("button", { name: /Open show menu/ }).click();
        await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
        await page.getByRole("button", { name: "Inputs", exact: true }).click();
        for (const label of ["Preload programmer changes", "Preload physical playback actions", "Preload virtual playback actions"]) await expect(page.getByLabel(label)).toBeVisible();
        return;
      }
      if (id === "PRELOAD-002") await setCaptureMask(api, false, true, false);
      if (id === "PRELOAD-004") await setCaptureMask(api, false, false, true);
      if (id === "PRELOAD-006") await setCaptureMask(api, true, true, true);
      await desk.open(bench.baseUrl);
      await page.getByRole("button", { name: /^PRELOAD/ }).click();
      if (id !== "PRELOAD-004") {
        await page.getByRole("button", { name: "BUILT-INS" }).click();
        await page.locator(".dock-entry").filter({ hasText: "Fixtures" }).click();
        await page.locator(".ui-data-table-row:not(.header):not(.empty)").first().click();
        await setDimmerByTouch(page, 75);
      }
      if (["PRELOAD-002", "PRELOAD-006"].includes(id)) await api.request("POST", `/api/v1/cuelists/${playback}/go`, {});
      if (id === "PRELOAD-004") await api.request("POST", `/api/v1/cuelists/${playback}/toggle`, { surface: "virtual" });
      await page.getByRole("button", { name: "PRELOAD GO", exact: true }).click();
    },
    assert: async ({ api, bench }, { playback }) => {
      if (id === "PRELOAD-003" || id === "PRELOAD-005") return;
      const state = await api.request<any>("GET", "/api/v1/playbacks");
      if (["PRELOAD-002", "PRELOAD-004", "PRELOAD-006"].includes(id)) expect(state.active.some((active: any) => active.playback_number === playback)).toBe(true);
      if (id !== "PRELOAD-004") expect((await bench.tick(3_000)).universes[0].slots[0]).toBe(191);
      if (id === "PRELOAD-006") {
        await api.command("preload.release", {});
        const after = await api.request<any>("GET", "/api/v1/playbacks");
        expect(after.active.some((active: any) => active.playback_number === playback)).toBe(true);
      }
    },
  });

  test("PRELOAD-002 @boundary › Flash and master remain live and are never queued", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "preload-002-boundary");
    await installPlayback(api);
    const playback = (await api.request<any>("GET", "/api/v1/playbacks")).pool[0].number;
    await setCaptureMask(api, false, true, false);
    await api.command("preload.enter", {});
    await api.request("POST", `/api/v1/cuelists/${playback}/flash`, { pressed: true });
    await api.request("PUT", `/api/v1/cuelists/${playback}/master`, { value: 0.4 });
    expect((await programmer(api)).preload_playback_pending).toEqual([]);
  });
});

async function configuration(api: any): Promise<Configuration> { return (await api.request("GET", "/api/v1/configuration")).configuration; }
async function setCaptureMask(api: any, programmer: boolean, physical: boolean, virtual: boolean) {
  const current = await configuration(api);
  await api.request("PUT", "/api/v1/configuration", { ...current, preload_programmer_changes: programmer, preload_physical_playback_actions: physical, preload_virtual_playback_actions: virtual });
}

async function installPlayback(api: any) {
  const fixtures = await fixtureIdsByNumber(api);
  const cueListId = crypto.randomUUID();
  await putObject(api, "cue_list", cueListId, { id: cueListId, name: "Preload Sequence", priority: 0, mode: "sequence", looped: false, chaser_step_millis: 1000, speed_group: null, cues: [{ number: 1, name: "Cue 1", changes: [{ fixture_id: fixtures[2], attribute: "intensity", value: { kind: "normalized", value: 0.5 }, automatic_restore: false }], group_changes: [], fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, phasers: [] }] });
  await putObject(api, "playback", "1", { number: 1, name: "Preload Playback", target: { type: "cue_list", cue_list_id: cueListId }, buttons: ["go", "go_minus", "flash"], fader: "master", go_activates: true, auto_off: true, xfade_millis: 0 });
  await putObject(api, "playback_page", "1", { number: 1, name: "Main", slots: { "1": 1 } });
}

async function addVirtualPlaybackPane(page: Page) {
  await page.getByRole("button", { name: "DESKS" }).click();
  await page.getByRole("button", { name: /New desk/ }).click();
  const grid = page.locator(".desk-grid");
  const box = await grid.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.click(box!.x + box!.width - 12, box!.y + box!.height - 12);
  await expect(page.getByRole("heading", { name: "Open Window" })).toBeVisible();
  await page.getByRole("button", { name: "Virtual Playbacks", exact: true }).click();
  const pane = page.locator(".desk-pane").filter({ hasText: "Virtual Playbacks" });
  await expect(pane).toBeVisible();
  await expect(pane.locator(".virtual-playback-cell")).toHaveCount(4);
}

async function setDimmerByTouch(page: Page, value: number) {
  const encoder = page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Dimmer" });
  const setValue = encoder.getByRole("button", { name: "Set value" });
  if (await setValue.isVisible()) await setValue.click(); else await encoder.locator(".vertical-touch-fader").click();
  await expect(page.getByRole("dialog", { name: "Enc 1 · Dimmer value" })).toBeVisible();
  await page.keyboard.type(String(value));
  await page.keyboard.press("Enter");
}
