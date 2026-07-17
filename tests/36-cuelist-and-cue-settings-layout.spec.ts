import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy, putObject } from "./support/catalog";

test("CUELIST-LAYOUT-001 @supplemental-ui › Cuelist Settings replaces the full sidebar while Cue Settings stays inline", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "cuelist-layout-001", "compact-rig");
  const cueListId = crypto.randomUUID();
  await putObject(api, "cue_list", cueListId, {
    id: cueListId,
    name: "Layout Sequence",
    priority: 0,
    mode: "sequence",
    looped: false,
    chaser_step_millis: 1000,
    speed_group: null,
    cues: [1, 2].map((number) => ({ id: crypto.randomUUID(), number, name: `Cue ${number}`, changes: [], group_changes: [], fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, phasers: [] })),
  });
  await putObject(api, "playback", "1", { number: 1, name: "Layout Sequence", target: { type: "cue_list", cue_list_id: cueListId }, buttons: ["go_minus", "go", "flash"], button_count: 3, fader: "master", has_fader: true, go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false });
  await putObject(api, "playback_page", "1", { number: 1, name: "Main", slots: { "1": 1 } });

  await desk.open(api.baseUrl);
  await page.setViewportSize({ width: 1280, height: 720 });
  const shift = page.getByRole("button", { name: "SHIFT", exact: true });
  if (!(await shift.isVisible().catch(() => false))) await page.locator(".mode-toggle").click();
  await shift.click();
  await page.getByRole("button", { name: "4", exact: true }).click();
  await page.locator(".cuelist-card").filter({ hasText: "Layout Sequence" }).click();

  await expect(page.getByRole("heading", { name: "Cue Settings" })).toBeVisible();
  await expect(page.locator(".cue-table")).toBeVisible();
  await page.getByRole("button", { name: "Cuelist Settings", exact: true }).click();
  const sidebar = page.locator(".cue-properties");
  const settings = page.getByRole("dialog", { name: "Cuelist Settings" });
  await expect(settings).toBeVisible();
  await expect(page.locator(".cue-table")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Cue Settings" })).toBeHidden();

  const [sidebarBounds, settingsBounds] = await Promise.all([sidebar.boundingBox(), settings.boundingBox()]);
  expect(sidebarBounds).not.toBeNull();
  expect(settingsBounds).not.toBeNull();
  expect(Math.abs(settingsBounds!.x - sidebarBounds!.x)).toBeLessThanOrEqual(1);
  expect(Math.abs(settingsBounds!.width - sidebarBounds!.width)).toBeLessThanOrEqual(1);
  const clipped = await settings.evaluate((element) => {
    const bounds = element.getBoundingClientRect();
    return [...element.querySelectorAll<HTMLElement>("input:not([type=checkbox]),button,.ui-form-field,.ui-form-control,.ui-switch-control")]
      .filter((candidate) => candidate.getBoundingClientRect().right > bounds.right + 1)
      .map((candidate) => candidate.getAttribute("aria-label") ?? candidate.textContent?.trim().slice(0, 40) ?? candidate.className);
  });
  expect(clipped).toEqual([]);

  await settings.getByLabel("Numeric priority").fill("12");
  await expect(page.locator(".cue-table tbody tr").nth(1)).toHaveAttribute("aria-disabled", "true");
  await page.locator(".cue-table tbody tr").nth(1).dispatchEvent("click");
  await expect(page.locator(".cue-table tbody tr").first()).toHaveClass(/selected/);
  await settings.getByRole("button", { name: "Cancel", exact: true }).click();
  const decision = page.getByRole("dialog", { name: "Unsaved Cuelist Settings" });
  await expect(decision).toBeVisible();
  await decision.getByRole("button", { name: "Stay", exact: true }).click();
  await expect(settings).toBeVisible();
  await settings.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("dialog", { name: "Unsaved Cuelist Settings" }).getByRole("button", { name: "Discard changes", exact: true }).click();

  await expect(settings).toBeHidden();
  await expect(page.getByRole("heading", { name: "Cue Settings" })).toBeVisible();
  await page.locator(".cue-table tbody tr").nth(1).click();
  await expect(page.getByText("Selected Cue · 2", { exact: true })).toBeVisible();
  await expect(page.locator(".cue-table")).toBeVisible();
});
