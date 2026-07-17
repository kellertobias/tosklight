import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy, putObject } from "./support/catalog";

test("PLAYBACK-COLOR-001 @supplemental-ui › runtime strengthens configured color while selection stays separate", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "playback-color-001", "default-stage");
  const cueListId = crypto.randomUUID();
  await putObject(api, "cue_list", cueListId, { id: cueListId, name: "Color Test", priority: 0, mode: "sequence", looped: false, chaser_step_millis: 1000, speed_group: null, cues: [{ id: crypto.randomUUID(), number: 1, name: "On", changes: [], group_changes: [], fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, phasers: [] }] });
  await putObject(api, "playback", "41", { number: 41, name: "Color Test", target: { type: "cue_list", cue_list_id: cueListId }, buttons: ["go_minus", "go", "flash"], button_count: 3, fader: "master", has_fader: true, go_activates: true, auto_off: true, xfade_millis: 0, color: "#f6e58d", flash_release: "release_all", protect_from_swap: false });
  await putObject(api, "playback_page", "1", { number: 1, name: "Main", slots: { "1": 41 } });
  await desk.open(api.baseUrl);
  await page.locator(".mode-toggle").click();
  const card = page.locator(".playback-fader-bank article.playback-colored").first();
  await expect(card).toBeVisible();
  const slot = Number(await card.getAttribute("data-playback-slot"));
  const before = await background(card);
  await api.request("POST", `/api/v1/control-desks/${api.session!.desk.id}/page-playbacks/${slot}/button`, { button: 2, pressed: true, surface: "virtual" });
  await expect(card).toHaveClass(/running/);
  await expect.poll(() => background(card)).not.toBe(before);

  const state = await api.request<any>("GET", "/api/v1/playbacks");
  const number = state.pages.find((candidate: any) => candidate.number === state.active_page).slots[String(slot)];
  await api.request("POST", `/api/v1/playback-pool/${number}/select`, {});
  await expect(card).toHaveClass(/selected/);
  await expect(card).toHaveClass(/running/);
  expect(await card.getAttribute("data-selected-playback")).toBe("true");

  const hardware = await bench.osc();
  await hardware.subscribe(`playback-colors-${crypto.randomUUID()}`, api.session!.desk.osc_alias);
  try {
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);
    const hardwareCard = page.locator(`.hardware-playback-card[data-playback-slot="${slot}"]`);
    await expect(hardwareCard).toHaveClass(/playback-colored/);
    await expect(hardwareCard).toHaveClass(/running/);
    await expect(hardwareCard).toHaveClass(/selected/);
    expect(await background(hardwareCard)).not.toBe(before);
  } finally {
    await hardware.close();
  }
});

async function background(locator: any) {
  return locator.evaluate((element: HTMLElement) => {
    const style = getComputedStyle(element);
    return `${style.backgroundColor}|${style.backgroundImage}|${style.borderTopColor}`;
  });
}
