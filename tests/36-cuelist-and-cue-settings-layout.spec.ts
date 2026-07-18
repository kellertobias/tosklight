import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy, object, putObject } from "./support/catalog";

test("CUELIST-LAYOUT-001 @supplemental-ui › compact Cue settings stay inline while Cuelist Settings opens as a structured modal", async ({ api, bench, desk, page }) => {
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

  await expect(page.getByRole("heading", { name: "Cue Settings" })).toBeHidden();
  await expect(page.locator(".cue-table")).toBeVisible();
  const sidebar = page.locator(".cue-properties");
  const preview = sidebar.locator(".cue-selected-preview");
  const badge = preview.locator(".cue-selected-label");
  await expect(badge).toHaveText("Selected Cue · 1");
  const [sidebarBounds, previewBounds, badgeBounds] = await Promise.all([sidebar.boundingBox(), preview.boundingBox(), badge.boundingBox()]);
  expect(sidebarBounds).not.toBeNull();
  expect(previewBounds).not.toBeNull();
  expect(badgeBounds).not.toBeNull();
  expect(Math.abs(previewBounds!.width - (sidebarBounds!.width - 16))).toBeLessThanOrEqual(2);
  expect(badgeBounds!.x).toBeGreaterThan(previewBounds!.x);
  expect(badgeBounds!.y).toBeGreaterThan(previewBounds!.y);
  const badgeStyle = await badge.evaluate((element) => {
    const style = getComputedStyle(element);
    return { color: style.color, background: style.backgroundColor, paddingLeft: style.paddingLeft, paddingTop: style.paddingTop };
  });
  expect(badgeStyle.color).toBe("rgb(255, 255, 255)");
  expect(badgeStyle.background).not.toBe("rgba(0, 0, 0, 0)");
  expect(Number.parseFloat(badgeStyle.paddingLeft)).toBeGreaterThan(0);
  expect(Number.parseFloat(badgeStyle.paddingTop)).toBeGreaterThan(0);
  await expect(sidebar.locator(".cue-settings-grid-measure > .ui-form-field > label")).toHaveText(["Title", "Fade", "Delay", "Trigger"]);
  const fullEditorStyle = await sidebar.locator(".cue-settings-grid").evaluate((element) => {
    const style = getComputedStyle(element);
    const rows = element.querySelector(".cue-settings-grid-measure");
    const rowStyle = rows ? getComputedStyle(rows) : null;
    return { border: style.borderTopWidth, background: style.backgroundColor, gap: rowStyle?.rowGap };
  });
  expect(fullEditorStyle.border).toBe("0px");
  expect(fullEditorStyle.background).toBe("rgba(0, 0, 0, 0)");
  expect(Number.parseFloat(fullEditorStyle.gap ?? "99")).toBeLessThanOrEqual(2);

  await page.setViewportSize({ width: 1280, height: 300 });
  await expect(sidebar.locator(".cue-selected-preview")).toBeVisible();
  await expect(sidebar.locator(".cue-settings-compact-fallback")).toBeVisible();
  await expect(sidebar).toHaveCSS("overflow", "hidden");
  await expect(sidebar.getByText("Press SET, then press an attribute value to edit it.")).toBeVisible();
  await page.getByRole("button", { name: "SET", exact: true }).click();
  await expect(sidebar.getByText("SET is active. Press an attribute value to edit it.")).toBeVisible();
  await sidebar.getByRole("button", { name: "Set Cue Fade" }).click();
  const fadeInput = page.getByRole("dialog", { name: "Fade" });
  await expect(fadeInput).toBeVisible();
  await fadeInput.getByRole("button", { name: "5", exact: true }).click();
  await fadeInput.getByRole("button", { name: "ENTER", exact: true }).click();
  await expect.poll(async () => (await object<any>(api, "cue_list", cueListId)).body.cues[0].fade_millis).toBe(5_000);
  await page.setViewportSize({ width: 1280, height: 1100 });
  await expect(sidebar.locator(".cue-settings-compact-fallback")).toBeHidden();
  await page.setViewportSize({ width: 1280, height: 720 });

  await page.getByRole("button", { name: "Cuelist Settings", exact: true }).click();
  const settings = page.getByRole("dialog", { name: "Cuelist Settings" });
  await expect(settings).toBeVisible();
  await expect(page.locator(".cue-table")).toBeVisible();
  expect(await settings.evaluate((dialog) => dialog.closest(".cue-properties") !== null)).toBe(false);
  await expect(settings.getByRole("button", { name: "Save", exact: true })).toBeVisible();
  await expect(settings.getByRole("button", { name: "Cancel", exact: true })).toHaveCount(0);
  await expect(settings.getByRole("button", { name: /Mode\s*\(Sequence\)/ })).toBeVisible();
  await expect(settings.getByRole("heading", { level: 3 })).toHaveText(["Priority", "Restart behavior", "Timing"]);
  const columns = settings.locator(".cuelist-settings-columns > section");
  const columnBounds = await columns.evaluateAll((items) => items.map((item) => item.getBoundingClientRect()));
  expect(columnBounds).toHaveLength(3);
  expect(Math.max(...columnBounds.map((bounds) => bounds.y)) - Math.min(...columnBounds.map((bounds) => bounds.y))).toBeLessThanOrEqual(2);
  const clipped = await settings.evaluate((element) => {
    const bounds = element.getBoundingClientRect();
    return [...element.querySelectorAll<HTMLElement>("input:not([type=checkbox]),button,.ui-form-field,.ui-form-control,.ui-switch-control")]
      .filter((candidate) => candidate.getBoundingClientRect().right > bounds.right + 1)
      .map((candidate) => candidate.getAttribute("aria-label") ?? candidate.textContent?.trim().slice(0, 40) ?? candidate.className);
  });
  expect(clipped).toEqual([]);

  await settings.getByRole("button", { name: /Mode\s*\(Sequence\)/ }).click();
  await settings.getByRole("menuitemradio", { name: "Chaser", exact: true }).click();
  await expect(settings.getByRole("button", { name: /Mode\s*\(Chaser\)/ })).toBeVisible();
  await settings.getByLabel("Speed multiplier").fill("1.5");
  const xfade = settings.getByRole("slider", { name: "Chaser X-fade" });
  await expect(xfade).toHaveAttribute("min", "0");
  await expect(xfade).toHaveAttribute("max", "100");
  await settings.getByLabel("Numeric priority").fill("12");
  await expect(page.locator(".cue-table tbody tr").nth(1)).toHaveAttribute("aria-disabled", "true");
  await page.locator(".cue-table tbody tr").nth(1).dispatchEvent("click");
  await expect(page.locator(".cue-table tbody tr").first()).toHaveClass(/selected/);
  await settings.getByRole("button", { name: "Close Cuelist Settings", exact: true }).click();
  const decision = page.getByRole("dialog", { name: "Unsaved Cuelist Settings" });
  await expect(decision).toBeVisible();
  await decision.getByRole("button", { name: "Stay", exact: true }).click();
  await expect(settings).toBeVisible();
  await settings.getByRole("button", { name: "Close Cuelist Settings", exact: true }).click();
  await page.getByRole("dialog", { name: "Unsaved Cuelist Settings" }).getByRole("button", { name: "Discard changes", exact: true }).click();

  await expect(settings).toBeHidden();
  await expect(page.getByRole("heading", { name: "Cue Settings" })).toBeHidden();
  await page.locator(".cue-table tbody tr").nth(1).click();
  await expect(page.getByText("Selected Cue · 2", { exact: true })).toBeVisible();
  await expect(page.locator(".cue-table")).toBeVisible();
});
