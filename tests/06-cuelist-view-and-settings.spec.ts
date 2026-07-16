import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { activeShowId, fixtureIdsByNumber, loadCanonicalCopy, object, putObject } from "./support/catalog";

test.describe("planned feature 06 — Cuelist View and Cuelist Settings", () => {
  test.beforeEach(({}, testInfo) => testInfo.setTimeout(90_000));

  test("CUE-011 @ui › Cue rows edit exact fields without executing playback and survive reopen", async ({ api, bench, desk, page }) => {
    const show = await loadCanonicalCopy(api, bench, "cue-011-cuelist-view", "compact-rig");
    const installed = await installCuelist(api, { name: "CUE-011 Sequence", numbers: [1, 2, 3] });
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await openCuelistView(page, desk, bench.baseUrl, installed.name);

    await expect(page.locator(".ui-window-header")).toContainText("Cuelist View · Cuelist 1");
    await expect(page.locator(".ui-window-header")).toContainText(installed.name);
    await expect(page.locator(".cue-table thead th")).toHaveText(["Preview", "No.", "Name", "Trigger", "Fade"]);
    await expect(page.locator(".cue-table thead")).not.toContainText("Status");
    for (const action of ["GO", "GO −", "Toggle", "Off"]) {
      await expect(page.locator(".cue-properties > button").filter({ hasText: new RegExp(`^${action}$`, "i") })).toHaveCount(0);
    }

    const beforeSelection = await playbackState(api);
    const beforeObject = await object<any>(api, "cue_list", installed.id);
    const beforeOutput = slot(await bench.tick(0), 1);
    const rows = page.locator(".cue-table tbody tr");
    await rows.nth(1).click();
    await expect(rows.nth(1)).toHaveClass(/selected/);
    await expect(page.getByText("Selected Cue · 2", { exact: true })).toBeVisible();
    await rows.nth(2).focus();
    await page.keyboard.press("Enter");
    await rows.nth(1).focus();
    await page.keyboard.press("Space");
    expect((await object<any>(api, "cue_list", installed.id)).revision).toBe(beforeObject.revision);
    expect((await playbackState(api)).active).toEqual(beforeSelection.active);
    expect(slot(await bench.tick(0), 1)).toBe(beforeOutput);

    await commitField(page, api, installed.id, "Title", "Center transition", (body) => body.cues[1].name);
    await expect(rows.nth(1)).toContainText("Center transition");
    await commitField(page, api, installed.id, "Fade", "2.5", (body) => body.cues[1].fade_millis);
    await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body.cues[1].fade_millis).toBe(2_500);
    await commitField(page, api, installed.id, "Delay", "1.25", (body) => body.cues[1].delay_millis);
    await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body.cues[1].delay_millis).toBe(1_250);

    await chooseCueTrigger(page, api, installed.id, "GO", "FOLLOW");
    await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body.cues[1].trigger).toEqual({ type: "follow", delay_millis: 0 });
    await chooseCueTrigger(page, api, installed.id, "FOLLOW", "GO");
    await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body.cues[1].trigger).toEqual({ type: "manual" });
    await chooseCueTrigger(page, api, installed.id, "GO", "TIME");
    await commitField(page, api, installed.id, "Trigger time", "4", (body) => body.cues[1].trigger.delay_millis);
    await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body.cues[1].trigger).toEqual({ type: "wait", delay_millis: 4_000 });
    await expect(rows.nth(1).locator("td").nth(3)).toHaveText("TIME");

    await rows.nth(2).click();
    await expect(page.getByLabel("Title")).toHaveValue("Cue 3");
    await rows.nth(1).click();
    await expect(page.getByLabel("Title")).toHaveValue("Center transition");
    await expect(page.getByLabel("Fade")).toHaveValue("2.5");
    await expect(page.getByLabel("Delay")).toHaveValue("1.25");
    await expect(page.getByLabel("Trigger time")).toHaveValue("4");

    await page.getByRole("button", { name: "Cuelist Settings", exact: true }).click();
    const settings = page.getByRole("dialog", { name: "Cuelist Settings" });
    await expect(settings).toContainText(installed.name);
    await settings.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(rows.nth(1)).toHaveClass(/selected/);

    const lastValid = await object<any>(api, "cue_list", installed.id);
    await page.getByLabel("Fade").fill("-1");
    await page.getByLabel("Fade").press("Enter");
    await expect(page.getByRole("alert").filter({ hasText: "Cue edit was not saved" })).toBeVisible();
    const afterInvalid = await object<any>(api, "cue_list", installed.id);
    expect(afterInvalid.revision).toBe(lastValid.revision);
    expect(afterInvalid.body.cues[1]).toEqual(lastValid.body.cues[1]);
    expect((await playbackState(api)).active).toEqual(beforeSelection.active);

    await api.request("POST", `/api/v1/shows/${show.id}/open`, { transition: "hold_current" });
    await page.reload();
    await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
    await openCuelistFromCurrentDesk(page, installed.name);
    await page.locator(".cue-table tbody tr").nth(1).click();
    await expect(page.getByLabel("Title")).toHaveValue("Center transition");
    await expect(page.getByLabel("Fade")).toHaveValue("2.5");
    await expect(page.getByLabel("Delay")).toHaveValue("1.25");
    await expect(page.getByLabel("Trigger time")).toHaveValue("4");
  });

  test("CUE-011 @ui › Renumber is one revision, preserves stable Cue/runtime identity, and rejects every invalid path", async ({ api, bench, desk, page }) => {
    await loadCanonicalCopy(api, bench, "cue-011-renumber", "compact-rig");
    const installed = await installCuelist(api, { name: "Renumber Sequence", numbers: [1, 1.5, 2, 7] });
    const oneCue = await installCuelist(api, { name: "One Cue Sequence", numbers: [1], playback: 2 });
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await openCuelistView(page, desk, bench.baseUrl, installed.name);
    const rows = page.locator(".cue-table tbody tr");
    await rows.nth(1).click();
    const original = await object<any>(api, "cue_list", installed.id);
    const selectedId = original.body.cues[1].id;
    const beforeOutput = slot(await bench.tick(0), 1);
    const auditMark = (await audit(api)).length;

    await openRenumber(page);
    const renumber = page.getByRole("dialog", { name: "Renumber Cues" });
    await renumber.getByLabel("Start Cue").focus();
    await page.keyboard.press("Enter");
    await expect(renumber).toBeHidden();
    const fromOne = await object<any>(api, "cue_list", installed.id);
    expect(fromOne.body.cues.map((cue: any) => cue.number)).toEqual([1, 2, 3, 4]);
    expect(fromOne.body.cues.map((cue: any) => cue.id)).toEqual(original.body.cues.map((cue: any) => cue.id));
    expect(fromOne.body.cues.map(stripNumber)).toEqual(original.body.cues.map(stripNumber));
    expect(fromOne.revision).toBeGreaterThan(original.revision);
    await expect(page.locator(".cue-table tbody tr.selected")).toContainText("Cue 1.5");
    await expect(page.locator(".cue-table tbody tr.selected td").nth(1)).toHaveText("2");
    await expect.poll(async () => runtime(api, 1)).toMatchObject({ current_cue_id: selectedId, current_cue_number: 2 });
    expect(slot(await bench.tick(0), 1)).toBe(beforeOutput);
    const renumberEvents = (await audit(api)).slice(auditMark).filter((event: any) => event.kind === "show_object_changed" && event.payload?.id === installed.id);
    expect(renumberEvents).toHaveLength(1);

    const revisionAfterSuccess = fromOne.revision;
    await openRenumber(page);
    await page.getByRole("dialog", { name: "Renumber Cues" }).getByRole("button", { name: "Cancel", exact: true }).click();
    expect((await object<any>(api, "cue_list", installed.id)).revision).toBe(revisionAfterSuccess);
    await openRenumber(page);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Renumber Cues" })).toBeHidden();
    expect((await object<any>(api, "cue_list", installed.id)).revision).toBe(revisionAfterSuccess);
    await openRenumber(page);
    await page.locator(".cuelist-settings-modal > .modal-backdrop").click({ position: { x: 4, y: 4 } });
    await expect(page.getByRole("dialog", { name: "Renumber Cues" })).toBeHidden();
    expect((await object<any>(api, "cue_list", installed.id)).revision).toBe(revisionAfterSuccess);

    for (const invalid of ["0", "-2", "1.5", "9007199254740991"]) {
      await openRenumber(page);
      const dialog = page.getByRole("dialog", { name: "Renumber Cues" });
      await dialog.getByLabel("Start Cue").fill(invalid);
      await dialog.getByRole("button", { name: "Renumber", exact: true }).click();
      await expect(dialog.getByRole("alert")).toContainText("positive whole number");
      expect((await object<any>(api, "cue_list", installed.id)).revision).toBe(revisionAfterSuccess);
      expect((await object<any>(api, "cue_list", installed.id)).body.cues.map((cue: any) => cue.number)).toEqual([1, 2, 3, 4]);
      await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    }

    await openRenumber(page);
    const staleDialog = page.getByRole("dialog", { name: "Renumber Cues" });
    await staleDialog.getByLabel("Start Cue").fill("10");
    let intercepted = false;
    await page.route(`**/objects/cue_list/${installed.id}`, async (route) => {
      if (route.request().method() !== "PUT" || intercepted) return route.continue();
      intercepted = true;
      const current = await object<any>(api, "cue_list", installed.id);
      await putObject(api, "cue_list", installed.id, { ...current.body, name: "Concurrent Renumber Sequence" }, current.revision);
      await route.continue();
    });
    await staleDialog.getByRole("button", { name: "Renumber", exact: true }).click();
    await expect(staleDialog.getByRole("alert")).toContainText("revision conflict");
    expect((await object<any>(api, "cue_list", installed.id)).body.cues.map((cue: any) => cue.number)).toEqual([1, 2, 3, 4]);
    await page.unroute(`**/objects/cue_list/${installed.id}`);
    await staleDialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await page.getByRole("dialog", { name: "Cuelist Settings" }).getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.locator(".ui-window-header")).toContainText("Concurrent Renumber Sequence");

    await openRenumber(page);
    await page.getByRole("dialog", { name: "Renumber Cues" }).getByLabel("Start Cue").fill("10");
    await page.getByRole("dialog", { name: "Renumber Cues" }).getByRole("button", { name: "Renumber", exact: true }).click();
    await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body.cues.map((cue: any) => cue.number)).toEqual([10, 11, 12, 13]);
    await expect.poll(async () => runtime(api, 1)).toMatchObject({ current_cue_id: selectedId, current_cue_number: 11 });

    await page.getByRole("button", { name: "← Cuelist Pool", exact: true }).click();
    await page.locator(".cuelist-card").filter({ hasText: oneCue.name }).click();
    await openRenumber(page);
    await page.getByRole("dialog", { name: "Renumber Cues" }).getByLabel("Start Cue").fill("10");
    await page.getByRole("dialog", { name: "Renumber Cues" }).getByRole("button", { name: "Renumber", exact: true }).click();
    await expect.poll(async () => (await object<any>(api, "cue_list", oneCue.id)).body.cues[0].number).toBe(10);
    await expect(page.getByRole("button", { name: "Delete Cue", exact: true })).toBeDisabled();
  });

  test("CUE-011 @wire › deleting the active Cue holds output and anchors GO/GO minus without a hidden persisted Cue", async ({ api, bench, desk, page }) => {
    await loadCanonicalCopy(api, bench, "cue-011-delete-active", "compact-rig");
    const fixtures = await fixtureIdsByNumber(api);
    const installed = await installCuelist(api, {
      name: "Delete Active Sequence",
      numbers: [1, 2, 3],
      cueFactory: (number, index) => cue(number, crypto.randomUUID(), fixtures[1], (index + 1) * 0.25, { fade: index === 2 ? 1_000 : 0 }),
    });
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(3_000);
    const heldLevel = slot(await bench.tick(0), 1);
    await openCuelistView(page, desk, bench.baseUrl, installed.name);
    await page.locator(".cue-table tbody tr").nth(1).click();
    const before = await object<any>(api, "cue_list", installed.id);
    const mark = (await audit(api)).length;
    await page.getByRole("button", { name: "Delete Cue", exact: true }).click();
    await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body.cues.map((cue: any) => cue.number)).toEqual([1, 3]);
    const deleted = await object<any>(api, "cue_list", installed.id);
    expect(deleted.revision).toBeGreaterThan(before.revision);
    expect(deleted.body.cues.some((cue: any) => cue.number === 2)).toBe(false);
    await expect(page.locator(".cue-table tbody tr")).toHaveCount(2);
    await expect.poll(async () => runtime(api, 1)).toMatchObject({
      current_cue_number: 2,
      deleted_cue_hold: { deleted_number: 2, previous_number: 1, next_number: 3 },
      normal_next_cue_number: 3,
    });
    expect(slot(await bench.tick(0), 1)).toBe(heldLevel);
    expect((await audit(api)).slice(mark).filter((event: any) => event.kind === "show_object_changed" && event.payload?.id === installed.id)).toHaveLength(1);

    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await expect.poll(async () => (await runtime(api, 1)).current_cue_number).toBe(3);
    expect((await runtime(api, 1)).deleted_cue_hold).toBeUndefined();
    expect(slot(await bench.tick(0), 1)).toBe(heldLevel);
    expect(slot(await bench.tick(500), 1)).toBeGreaterThanOrEqual(159);
    expect(slot(await bench.tick(0), 1)).toBeLessThanOrEqual(160);
    expect(slot(await bench.tick(500), 1)).toBe(191);
    await api.request("POST", "/api/v1/cuelists/1/go-minus", {});
    await expect.poll(async () => runtime(api, 1)).toMatchObject({ current_cue_number: 1 });
    expect(slot(await bench.tick(0), 1)).toBe(191);
    expect(slot(await bench.tick(3_000), 1)).toBe(64);
  });

  test("CUE-012 @ui › every settings control persists, validates Chaser overlap, and upgrades legacy defaults", async ({ api, bench, desk, page }) => {
    const show = await loadCanonicalCopy(api, bench, "cue-012-settings", "compact-rig");
    const installed = await installCuelist(api, { name: "CUE-012 Settings", numbers: [1, 2, 3, 4], legacy: true, looped: true });
    const legacyOff = await installCuelist(api, { name: "Legacy Off", numbers: [1, 2], playback: 2, legacy: true, looped: false });
    const legacyTracking = await installCuelist(api, {
      name: "Legacy Tracking Chaser",
      numbers: [1, 2],
      playback: 3,
      legacy: true,
      looped: true,
      mode: "chaser",
      speedGroup: null,
    });
    await openCuelistView(page, desk, bench.baseUrl, installed.name);
    await page.getByRole("button", { name: "Cuelist Settings", exact: true }).click();
    const settings = page.getByRole("dialog", { name: "Cuelist Settings" });
    await expect(settings).toContainText(installed.name);
    await expect(selectField(settings, "Mode")).toContainText("Sequence");
    await expect(selectField(settings, "Intensity priority mode")).toContainText("HTP");
    await expect(selectField(settings, "Wrap Around")).toContainText("Tracking");
    await expect(selectField(settings, "Restart mode")).toContainText("First Cue");
    await expect(settings.getByLabel("Force Cue Timing")).not.toBeChecked();
    await expect(settings.getByLabel("Disable Cue Timing")).not.toBeChecked();

    await choose(page, settings, "Sequence", "Chaser");
    await settings.getByLabel("Numeric priority").fill("42");
    await expect(settings.getByLabel("Numeric priority")).toHaveValue("42");
    await choose(page, settings, "HTP", "LTP");
    await expect(settings.getByLabel("Numeric priority")).toHaveValue("42");
    await choose(page, settings, "Tracking", "Reset");
    await choose(page, settings, "First Cue", "Continue Current Cue");
    await clickSwitch(settings, "Force Cue Timing");
    await clickSwitch(settings, "Disable Cue Timing");
    await expect(settings.getByLabel("Numeric priority")).toHaveValue("42");
    await expect(selectField(settings, "Speed Group")).toContainText("A");
    await choose(page, settings, "1×", "2×");
    await settings.getByLabel("Chaser X-fade").fill("0.3");
    const beforeInvalid = await object<any>(api, "cue_list", installed.id);
    await settings.getByRole("button", { name: "Save", exact: true }).click();
    await expect(settings.getByRole("alert")).toContainText("Chaser X-fade must not exceed");
    expect((await object<any>(api, "cue_list", installed.id)).revision).toBe(beforeInvalid.revision);
    await settings.getByLabel("Chaser X-fade").fill("0.1");
    await settings.getByRole("button", { name: "Save", exact: true }).click();
    await expect(settings).toBeHidden();
    await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body).toMatchObject({
      mode: "chaser",
      priority: 42,
      intensity_priority_mode: "ltp",
      wrap_mode: "reset",
      restart_mode: "continue_current_cue",
      force_cue_timing: true,
      disable_cue_timing: true,
      speed_group: "A",
      speed_multiplier: 2,
      chaser_xfade_millis: 100,
    });

    await page.getByRole("button", { name: "← Cuelist Pool", exact: true }).click();
    const legacyCard = page.locator(".cuelist-card").filter({ hasText: legacyOff.name });
    const box = await legacyCard.boundingBox();
    expect(box).not.toBeNull();
    await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
    await page.mouse.down();
    await page.waitForTimeout(700);
    await page.mouse.up();
    const shortcutSettings = page.getByRole("dialog", { name: "Cuelist Settings" });
    await expect(shortcutSettings).toContainText(legacyOff.name);
    await expect(selectField(shortcutSettings, "Wrap Around")).toContainText("Off");
    await shortcutSettings.getByRole("button", { name: "Save", exact: true }).click();
    await expect.poll(async () => (await object<any>(api, "cue_list", legacyOff.id)).body.wrap_mode).toBe("off");

    await page.locator(".cuelist-card").filter({ hasText: legacyTracking.name }).click();
    await page.getByRole("button", { name: "Cuelist Settings", exact: true }).click();
    const trackingSettings = page.getByRole("dialog", { name: "Cuelist Settings" });
    await expect(selectField(trackingSettings, "Wrap Around")).toContainText("Tracking");
    await expect(selectField(trackingSettings, "Speed Group")).toContainText("Legacy fixed step");
    await trackingSettings.getByRole("button", { name: "Save", exact: true }).click();
    await expect.poll(async () => (await object<any>(api, "cue_list", legacyTracking.id)).body.wrap_mode).toBe("tracking");

    await api.request("POST", `/api/v1/shows/${show.id}/open`, { transition: "hold_current" });
    await bench.restart();
    await api.login();
    expect((await object<any>(api, "cue_list", installed.id)).body).toMatchObject({
      mode: "chaser",
      priority: 42,
      intensity_priority_mode: "ltp",
      wrap_mode: "reset",
      restart_mode: "continue_current_cue",
      force_cue_timing: true,
      disable_cue_timing: true,
      speed_group: "A",
      speed_multiplier: 2,
      chaser_xfade_millis: 100,
    });
    const migratedOff = await object<any>(api, "cue_list", legacyOff.id);
    expect(migratedOff.body).toMatchObject({ wrap_mode: "off", restart_mode: "first_cue", disable_cue_timing: false, intensity_priority_mode: "htp", speed_multiplier: 1 });
    const migratedTracking = await object<any>(api, "cue_list", legacyTracking.id);
    expect(migratedTracking.body).toMatchObject({
      wrap_mode: "tracking",
      restart_mode: "first_cue",
      disable_cue_timing: false,
      intensity_priority_mode: "htp",
      speed_multiplier: 1,
      chaser_step_millis: 1_000,
      speed_group: null,
    });
    expect((await object<any>(api, "playback", "1")).body.target.cue_list_id).toBe(installed.id);
    expect((await object<any>(api, "playback", "2")).body.target.cue_list_id).toBe(legacyOff.id);
    expect((await object<any>(api, "playback", "3")).body.target.cue_list_id).toBe(legacyTracking.id);
  });

  test("CUE-012 @wire › arbitration, wrap, restart, timing precedence, and Chaser phase are engine behavior", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "cue-012-engine", "compact-rig");
    const fixtures = await fixtureIdsByNumber(api);
    await setControlTiming(api, [120, 90, 60, 30, 15], 0);

    const high = await installCuelist(api, { name: "Priority High", numbers: [1], playback: 1, fixture: fixtures[1], levels: [0.8] });
    const low = await installCuelist(api, { name: "Priority Low", numbers: [1], playback: 2, fixture: fixtures[1], levels: [0.3] });
    expect(fixtures[21]).toBeTruthy();
    for (const [installed, level] of [
      [high, 0.8],
      [low, 0.3],
    ] as const) {
      const stored = await object<any>(api, "cue_list", installed.id);
      stored.body.cues[0].changes.push({
        fixture_id: fixtures[21],
        attribute: "red",
        value: { kind: "normalized", value: level },
        automatic_restore: false,
      });
      await putObject(api, "cue_list", installed.id, stored.body, stored.revision);
    }
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(1);
    await api.request("POST", "/api/v1/cuelists/2/go", {});
    expect(slot(await bench.tick(0), 1)).toBe(204);
    expect(await visualizationLevel(api, fixtures[21], "red")).toBeCloseTo(0.3, 4);
    for (const installed of [high, low]) {
      const stored = await object<any>(api, "cue_list", installed.id);
      await putObject(api, "cue_list", installed.id, { ...stored.body, intensity_priority_mode: "ltp" }, stored.revision);
    }
    expect(slot(await bench.tick(0), 1)).toBe(77);
    expect(await visualizationLevel(api, fixtures[21], "red")).toBeCloseTo(0.3, 4);
    const highPriority = await object<any>(api, "cue_list", high.id);
    await putObject(api, "cue_list", high.id, { ...highPriority.body, priority: 10 }, highPriority.revision);
    expect(slot(await bench.tick(0), 1)).toBe(204);
    expect(await visualizationLevel(api, fixtures[21], "red")).toBeCloseTo(0.8, 4);

    await loadCanonicalCopy(api, bench, "cue-012-wrap", "compact-rig");
    await setControlTiming(api, [120, 90, 60, 30, 15], 0);
    const base = await installCuelist(api, { name: "Underlying", numbers: [1], playback: 2, fixture: fixtures[2], levels: [0.2], priority: -1 });
    const wrapped = await installCuelist(api, {
      name: "Wrapped",
      numbers: [1, 2],
      playback: 1,
      cueFactory: (number, index) =>
        index === 0
          ? cue(number, crypto.randomUUID(), fixtures[1], 1, { fade: 2_000, delay: 1_000 })
          : cue(number, crypto.randomUUID(), fixtures[2], 1),
    });
    await api.request("POST", "/api/v1/cuelists/2/go", {});
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(3_000);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(0);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    expect((await runtime(api, 1)).current_cue_number).toBe(2);
    expect(slot(await bench.tick(0), 2)).toBe(255);
    const offBody = await object<any>(api, "cue_list", wrapped.id);
    await putObject(api, "cue_list", wrapped.id, { ...offBody.body, wrap_mode: "tracking" }, offBody.revision);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    expect((await runtime(api, 1)).current_cue_number).toBe(1);
    expect(slot(await bench.tick(0), 2)).toBe(255);
    await api.request("POST", "/api/v1/cuelists/1/go-minus", {});
    expect((await runtime(api, 1)).current_cue_number).toBe(1);
    const trackingBody = await object<any>(api, "cue_list", wrapped.id);
    await putObject(api, "cue_list", wrapped.id, { ...trackingBody.body, wrap_mode: "reset" }, trackingBody.revision);
    await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 2 });
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    expect(slot(await bench.tick(999), 2)).toBe(255);
    expect(slot(await bench.tick(1_001), 2)).toBeGreaterThan(51);
    expect(slot(await bench.tick(1_000), 2)).toBe(51);
    expect((await object<any>(api, "cue_list", base.id)).body.cues[0].changes[0].value.value).toBeCloseTo(0.2, 6);

    await loadCanonicalCopy(api, bench, "cue-012-restart", "compact-rig");
    const restarted = await installCuelist(api, { name: "Restart", numbers: [1, 2, 3], levels: [0.2, 0.5, 0.8] });
    for (const action of ["on", "toggle", "go"]) {
      await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 2 });
      await api.request("POST", "/api/v1/cuelists/1/off", {});
      await api.request("POST", `/api/v1/cuelists/1/${action}`, {});
      expect((await runtime(api, 1)).current_cue_number).toBe(1);
    }
    let restartBody = await object<any>(api, "cue_list", restarted.id);
    await putObject(api, "cue_list", restarted.id, { ...restartBody.body, restart_mode: "continue_current_cue" }, restartBody.revision);
    for (const action of ["on", "toggle", "go"]) {
      await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 2 });
      await api.request("POST", "/api/v1/cuelists/1/off", {});
      await api.request("POST", `/api/v1/cuelists/1/${action}`, {});
      expect((await runtime(api, 1)).current_cue_number).toBe(2);
      expect(slot(await bench.tick(0), 1)).toBe(128);
    }
    const neverRun = await installCuelist(api, { name: "Never Run Continue", numbers: [1, 2], playback: 2 });
    let neverRunBody = await object<any>(api, "cue_list", neverRun.id);
    await putObject(api, "cue_list", neverRun.id, { ...neverRunBody.body, restart_mode: "continue_current_cue" }, neverRunBody.revision);
    await api.request("POST", "/api/v1/cuelists/2/on", {});
    expect((await runtime(api, 2)).current_cue_number).toBe(1);
    await api.request("POST", "/api/v1/cuelists/2/off", {});
    await api.request("POST", "/api/v1/cuelists/1/off", {});
    restartBody = await object<any>(api, "cue_list", restarted.id);
    await putObject(api, "cue_list", restarted.id, { ...restartBody.body, cues: restartBody.body.cues.filter((item: any) => item.number !== 2) }, restartBody.revision);
    await api.request("POST", "/api/v1/cuelists/1/on", {});
    expect((await runtime(api, 1)).current_cue_number).toBe(1);

    await loadCanonicalCopy(api, bench, "cue-012-timing", "compact-rig");
    await setControlTiming(api, [120, 90, 60, 30, 15], 0);
    const timed = await installCuelist(api, {
      name: "Timing",
      numbers: [1, 2],
      cueFactory: (number, index) =>
        index === 0
          ? cue(number, crypto.randomUUID(), fixtures[1], 1, { fade: 1_000, delay: 500, valueFade: 2_000, valueDelay: 100 })
          : cue(number, crypto.randomUUID(), fixtures[1], 0.2, { trigger: { type: "wait", delay_millis: 4_000 } }),
    });
    const serializedTiming = timingBytes((await object<any>(api, "cue_list", timed.id)).body);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    expect(slot(await bench.tick(1_100), 1)).toBe(128);
    await api.request("POST", "/api/v1/cuelists/1/off", {});
    let timedBody = await object<any>(api, "cue_list", timed.id);
    await putObject(api, "cue_list", timed.id, { ...timedBody.body, force_cue_timing: true }, timedBody.revision);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    expect(slot(await bench.tick(1_000), 1)).toBe(128);
    await api.request("POST", "/api/v1/cuelists/1/off", {});
    timedBody = await object<any>(api, "cue_list", timed.id);
    await putObject(api, "cue_list", timed.id, { ...timedBody.body, disable_cue_timing: true }, timedBody.revision);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    expect(slot(await bench.tick(0), 1)).toBe(51);
    expect((await runtime(api, 1)).current_cue_number).toBe(2);
    timedBody = await object<any>(api, "cue_list", timed.id);
    await putObject(api, "cue_list", timed.id, { ...timedBody.body, disable_cue_timing: false, force_cue_timing: false }, timedBody.revision);
    expect(timingBytes((await object<any>(api, "cue_list", timed.id)).body)).toBe(serializedTiming);

    await loadCanonicalCopy(api, bench, "cue-012-chaser", "compact-rig");
    await setControlTiming(api, [120, 90, 60, 30, 15], 0);
    const chaser = await installCuelist(api, { name: "Chaser", numbers: [1, 2, 3, 4], mode: "chaser", speedGroup: "A", speedMultiplier: 1, chaserXfade: 100 });
    const chaserShowId = await activeShowId(api);
    const valid = await object<any>(api, "cue_list", chaser.id);
    await expect(putObject(api, "cue_list", chaser.id, { ...valid.body, chaser_xfade_millis: 501 }, valid.revision)).rejects.toThrow(/effective step duration/i);
    const afterRejectedXfade = await object<any>(api, "cue_list", chaser.id);
    if (afterRejectedXfade.revision !== valid.revision) {
      await putObject(api, "cue_list", chaser.id, valid.body, afterRejectedXfade.revision);
    }
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    expect((await runtime(api, 1)).current_cue_number).toBe(1);
    await bench.tick(499);
    expect((await runtime(api, 1)).current_cue_number).toBe(1);
    await bench.tick(1);
    expect((await runtime(api, 1)).current_cue_number).toBe(2);
    expect(slot(await bench.tick(50), 1)).toBe(96);
    expect(slot(await bench.tick(50), 1)).toBe(128);

    await reopenAndReset(api, chaserShowId);
    let chaserBody = await object<any>(api, "cue_list", chaser.id);
    await putObject(api, "cue_list", chaser.id, { ...chaserBody.body, speed_multiplier: 0.5 }, chaserBody.revision);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(999);
    expect((await runtime(api, 1)).current_cue_number).toBe(1);
    await bench.tick(1);
    expect((await runtime(api, 1)).current_cue_number).toBe(2);

    await reopenAndReset(api, chaserShowId);
    chaserBody = await object<any>(api, "cue_list", chaser.id);
    await putObject(api, "cue_list", chaser.id, { ...chaserBody.body, speed_multiplier: 2 }, chaserBody.revision);
    chaserBody = await object<any>(api, "cue_list", chaser.id);
    await expect(putObject(api, "cue_list", chaser.id, { ...chaserBody.body, chaser_xfade_millis: 251 }, chaserBody.revision)).rejects.toThrow(/effective step duration/i);
    const afterRejectedFastXfade = await object<any>(api, "cue_list", chaser.id);
    if (afterRejectedFastXfade.revision !== chaserBody.revision) {
      await putObject(api, "cue_list", chaser.id, chaserBody.body, afterRejectedFastXfade.revision);
    }
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(249);
    expect((await runtime(api, 1)).current_cue_number).toBe(1);
    await bench.tick(1);
    expect((await runtime(api, 1)).current_cue_number).toBe(2);

    await reopenAndReset(api, chaserShowId);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(1_000);
    const direct = await runtime(api, 1);
    await reopenAndReset(api, chaserShowId);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    for (let index = 0; index < 4; index += 1) await bench.tick(250);
    const incremental = await runtime(api, 1);
    expect(direct.current_cue_number).toBe(incremental.current_cue_number);
    expect(direct.activated_at).toBe(incremental.activated_at);

    await reopenAndReset(api, chaserShowId);
    chaserBody = await object<any>(api, "cue_list", chaser.id);
    await putObject(api, "cue_list", chaser.id, { ...chaserBody.body, speed_multiplier: 1 }, chaserBody.revision);
    await setControlTiming(api, [120, 90, 60, 30, 15], 0);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(250);
    await setControlTiming(api, [60, 90, 60, 30, 15], 0);
    await bench.tick(499);
    expect((await runtime(api, 1)).current_cue_number).toBe(1);
    await bench.tick(1);
    expect((await runtime(api, 1)).current_cue_number).toBe(2);

    await reopenAndReset(api, chaserShowId);
    await setControlTiming(api, [120, 90, 60, 30, 15], 0);
    chaserBody = await object<any>(api, "cue_list", chaser.id);
    await putObject(api, "cue_list", chaser.id, { ...chaserBody.body, speed_multiplier: 2, disable_cue_timing: true }, chaserBody.revision);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(250);
    expect((await runtime(api, 1)).current_cue_number).toBe(2);
    expect(slot(await bench.tick(0), 1)).toBe(128);
    await bench.tick(249);
    expect((await runtime(api, 1)).current_cue_number).toBe(2);
    await bench.tick(1);
    expect((await runtime(api, 1)).current_cue_number).toBe(3);
  });
});

interface InstallOptions {
  name: string;
  numbers: number[];
  playback?: number;
  fixture?: string;
  levels?: number[];
  priority?: number;
  mode?: "sequence" | "chaser";
  speedGroup?: "A" | "B" | "C" | "D" | "E" | null;
  speedMultiplier?: number;
  chaserXfade?: number;
  legacy?: boolean;
  looped?: boolean;
  cueFactory?: (number: number, index: number) => any;
}

async function installCuelist(api: ApiDriver, options: InstallOptions): Promise<{ id: string; name: string }> {
  const playback = options.playback ?? 1;
  const fixture = options.fixture ?? (await fixtureIdsByNumber(api))[1];
  const id = crypto.randomUUID();
  const cues = options.numbers.map((number, index) =>
    options.cueFactory?.(number, index) ?? cue(number, crypto.randomUUID(), fixture, options.levels?.[index] ?? (index + 1) * 0.25),
  );
  const body: Record<string, unknown> = {
    id,
    name: options.name,
    priority: options.priority ?? 0,
    mode: options.mode ?? "sequence",
    looped: options.looped ?? false,
    chaser_step_millis: 1_000,
    speed_group: options.speedGroup ?? null,
    cues,
  };
  if (!options.legacy) {
    Object.assign(body, {
      intensity_priority_mode: "htp",
      wrap_mode: options.looped ? "tracking" : "off",
      restart_mode: "first_cue",
      force_cue_timing: false,
      disable_cue_timing: false,
      chaser_xfade_millis: options.chaserXfade ?? 0,
      speed_multiplier: options.speedMultiplier ?? 1,
    });
  }
  await putObject(api, "cue_list", id, body);
  await putObject(api, "playback", String(playback), {
    number: playback,
    name: `${options.name} Playback`,
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
  let page: any;
  try {
    page = await object<any>(api, "playback_page", "1");
  } catch {
    page = null;
  }
  await putObject(
    api,
    "playback_page",
    "1",
    { number: 1, name: "Main", slots: { ...(page?.body.slots ?? {}), [String(playback)]: playback } },
    page?.revision ?? 0,
  );
  return { id, name: options.name };
}

function cue(
  number: number,
  id: string,
  fixture: string,
  level: number,
  options: { fade?: number; delay?: number; valueFade?: number; valueDelay?: number; trigger?: any } = {},
) {
  return {
    id,
    number,
    name: `Cue ${number}`,
    fade_millis: options.fade ?? 0,
    delay_millis: options.delay ?? 0,
    trigger: options.trigger ?? { type: "manual" },
    changes: [
      {
        fixture_id: fixture,
        attribute: "intensity",
        value: { kind: "normalized", value: level },
        automatic_restore: false,
        ...(options.valueFade == null ? {} : { fade_millis: options.valueFade }),
        ...(options.valueDelay == null ? {} : { delay_millis: options.valueDelay }),
      },
    ],
    group_changes: [],
    phasers: [],
  };
}

async function openCuelistView(page: Page, desk: any, baseUrl: string, name: string): Promise<void> {
  await desk.open(baseUrl);
  await openCuelistFromCurrentDesk(page, name);
}

async function openCuelistFromCurrentDesk(page: Page, name: string): Promise<void> {
  const shift = page.getByRole("button", { name: "SHIFT", exact: true });
  if (!(await shift.isVisible().catch(() => false))) await page.locator(".mode-toggle").click();
  await shift.click();
  await page.getByRole("button", { name: "4", exact: true }).click();
  await expect(page.locator(".cuelist-pool-window")).toBeVisible();
  await page.locator(".cuelist-card").filter({ hasText: name }).click();
  await expect(page.locator(".cue-table")).toBeVisible();
}

async function commitField(page: Page, api: ApiDriver, cueListId: string, label: string, value: string, read: (body: any) => unknown): Promise<void> {
  const before = await object<any>(api, "cue_list", cueListId);
  const field = page.getByLabel(label, { exact: true });
  await field.fill(value);
  await field.press("Enter");
  await expect.poll(async () => (await object<any>(api, "cue_list", cueListId)).revision).toBeGreaterThan(before.revision);
  const after = await object<any>(api, "cue_list", cueListId);
  await expect(page.locator(".ui-window-info")).toContainText(`Revision ${after.revision}`);
  expect(read(after.body)).not.toBeUndefined();
}

async function choose(page: Page, scope: ReturnType<Page["locator"]>, current: string, next: string): Promise<void> {
  await scope.getByRole("button", { name: current, exact: true }).click();
  await page.getByRole("option", { name: next, exact: true }).click();
}

async function chooseCueTrigger(page: Page, api: ApiDriver, cueListId: string, current: string, next: string): Promise<void> {
  const before = await object<any>(api, "cue_list", cueListId);
  await choose(page, page.locator(".cue-properties"), current, next);
  await expect.poll(async () => (await object<any>(api, "cue_list", cueListId)).revision).toBeGreaterThan(before.revision);
  const after = await object<any>(api, "cue_list", cueListId);
  await expect(page.locator(".ui-window-info")).toContainText(`Revision ${after.revision}`);
}

async function clickSwitch(scope: ReturnType<Page["locator"]>, label: string): Promise<void> {
  await scope.locator(".ui-form-field").filter({ hasText: label }).locator(".ui-switch-control").click();
}

function selectField(scope: ReturnType<Page["locator"]>, label: string) {
  return scope.locator(".ui-form-field").filter({ hasText: label }).getByRole("button").first();
}

async function openRenumber(page: Page): Promise<void> {
  const settings = page.getByRole("dialog", { name: "Cuelist Settings" });
  if (!(await settings.isVisible().catch(() => false))) await page.getByRole("button", { name: "Cuelist Settings", exact: true }).click();
  await settings.getByRole("button", { name: "Renumber Cues", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Renumber Cues" })).toBeVisible();
}

async function playbackState(api: ApiDriver): Promise<any> {
  return api.request("GET", "/api/v1/playbacks");
}

async function runtime(api: ApiDriver, playback: number): Promise<any> {
  return (await playbackState(api)).active.find((item: any) => item.playback_number === playback);
}

async function audit(api: ApiDriver): Promise<any[]> {
  return api.request("GET", "/api/v1/audit?after=0");
}

async function visualizationLevel(api: ApiDriver, fixtureId: string, attribute: string): Promise<number> {
  const visualization = await api.request<any>("GET", "/api/v1/visualization");
  const value = visualization.values.find((item: any) => item.fixture_id === fixtureId && item.attribute === attribute)?.value;
  return typeof value === "number" ? value : value?.value;
}

function slot(frame: { universes: Array<{ universe: number; slots: number[] }> }, address: number): number | undefined {
  return frame.universes.find((universe) => universe.universe === 1)?.slots[address - 1];
}

function stripNumber(cueBody: any): any {
  const { number: _number, ...rest } = cueBody;
  return rest;
}

async function setControlTiming(api: ApiDriver, speedGroups: number[], sequenceMasterFade: number): Promise<void> {
  const configuration = await api.request<any>("GET", "/api/v1/configuration");
  await api.request("PUT", "/api/v1/configuration", {
    ...configuration,
    speed_groups_bpm: speedGroups,
    sequence_master_fade_millis: sequenceMasterFade,
  });
}

async function reopenAndReset(api: ApiDriver, showId: string): Promise<void> {
  await api.request("POST", `/api/v1/shows/${showId}/open`, { transition: "hold_current" });
  await api.request("POST", "/api/v1/test/clock/reset", undefined, false);
}

function timingBytes(body: any): string {
  return JSON.stringify(
    body.cues.map((item: any) => ({
      fade_millis: item.fade_millis,
      delay_millis: item.delay_millis,
      trigger: item.trigger,
      changes: item.changes.map((change: any) => ({ fade_millis: change.fade_millis, delay_millis: change.delay_millis })),
    })),
  );
}
