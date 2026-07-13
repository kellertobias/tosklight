import { expect, test } from "./bench/fixtures";
import type { APIRequestContext, Page } from "@playwright/test";

interface Session { session_id: string; token: string }
interface AuditEvent { revision: number; kind: string; payload: Record<string, unknown> }
test.describe.configure({ mode: "serial" });

async function waitForConnected(page: Page) {
  await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
  await expect(page.locator(".connection-banner")).toBeHidden({ timeout: 10_000 });
}

async function setDimmerByTouch(page: Page, value: number) {
  const encoder = page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Dimmer" });
  const setValue = encoder.getByRole("button", { name: "Set value" });
  if (await setValue.isVisible()) await setValue.click();
  else await encoder.locator(".vertical-touch-fader").click();
  await expect(page.getByRole("dialog", { name: "Enc 1 · Dimmer value" })).toBeVisible();
  await page.keyboard.type(String(value));
  await page.keyboard.press("Enter");
}

test("physical Enter saves Save As and activates the new show", async ({ page, request }) => {
  const session = await jsonRequest<Session>(request, "post", "/api/v1/sessions", undefined, { username: "Operator" });
  const empty = await jsonRequest<{ id: string }>(request, "post", "/api/v1/shows", session, { name: `Empty-${crypto.randomUUID()}`, data_base64: null, overwrite: false });
  await jsonRequest(request, "post", `/api/v1/shows/${empty.id}/open`, session, { transition: "hold_current" });
  await page.goto("/");
  await waitForConnected(page);
  await page.getByRole("button", { name: "Open show menu" }).click();
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await page.getByLabel("Show name").click();
  await page.keyboard.type("My Show");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("dialog", { name: "Save show" })).toBeHidden();
  await expect(page.locator(".show-details>b")).toHaveText("My Show");
  const bootstrap = await jsonRequest<{ active_show: { name: string } | null }>(request, "get", "/api/v1/bootstrap", session);
  expect(bootstrap.active_show?.name).toBe("My Show");
});

async function jsonRequest<T>(request: APIRequestContext, method: "get" | "post" | "put", url: string, session?: Session, data?: unknown): Promise<T> {
  const headers = session ? { authorization: `Bearer ${session.token}`, ...(method === "put" ? { "if-match": "0" } : {}) } : undefined;
  const response = await request[method](url, { data, headers });
  expect(response.ok(), `${method.toUpperCase()} ${url}: ${await response.text()}`).toBeTruthy();
  return response.status() === 204 ? undefined as T : response.json() as Promise<T>;
}

async function waitForDmx(request: APIRequestContext, expected: number) {
  const settle = await request.post("/api/v1/test/clock/advance", { data: { millis: 3_000 } });
  expect(settle.ok(), `manual fade settle: ${await settle.text()}`).toBeTruthy();
  await expect.poll(async () => {
    const tick = await request.post("/api/v1/test/clock/advance", { data: { milliseconds: 0 } });
    expect(tick.ok(), `manual output tick: ${await tick.text()}`).toBeTruthy();
    const snapshot = await jsonRequest<{ universes: Array<{ universe: number; slots: number[] }> }>(request, "get", "/api/v1/dmx");
    return snapshot.universes.find((universe) => universe.universe === 1)?.slots[0];
  }, { timeout: 8_000 }).toBe(expected);
}

async function registerAuditReceiver(page: Page, session: Session) {
  await page.evaluate(({ token }) => new Promise<void>((resolve, reject) => {
    window.__lightAuditEvents = [];
    const socket = new WebSocket(`ws://${location.host}/api/v1/events`, ["light.v1", `light.token.${token}`]);
    socket.addEventListener("message", (message) => {
      const event = JSON.parse(String(message.data)) as AuditEvent;
      if ("kind" in event) window.__lightAuditEvents.push(event);
    });
    socket.addEventListener("open", () => resolve(), { once: true });
    socket.addEventListener("error", () => reject(new Error("audit WebSocket failed")), { once: true });
    window.__lightAuditSocket = socket;
  }), session);
}

test("touch programmer path is audited and reaches the rendered DMX output", async ({ page, request, desk }) => {
  const setupSession = await jsonRequest<Session>(request, "post", "/api/v1/sessions", undefined, { username: "Operator" });
  const show = await jsonRequest<{ id: string }>(request, "post", "/api/v1/shows", setupSession, { name: `E2E-${crypto.randomUUID()}`, data_base64: null, overwrite: false });
  const fixtureIds = [crypto.randomUUID(), crypto.randomUUID(), crypto.randomUUID()];
  for (const [index, fixtureId] of fixtureIds.entries()) await jsonRequest(request, "put", `/api/v1/shows/${show.id}/objects/patched_fixture/dimmer-${index + 1}`, setupSession, {
    fixture_id: fixtureId, fixture_number: index + 1,
    definition: {
      schema_version: 1, id: crypto.randomUUID(), revision: 1, manufacturer: "E2E", model: `Dimmer ${index + 1}`, mode: "1ch", footprint: 1,
      heads: [{ index: 0, name: "Main", shared: true, parameters: [{ attribute: "intensity", components: [{ offset: 0, byte_order: "msb_first" }], default: 0, virtual_dimmer: false, capabilities: [] }] }],
      color_calibration: null, hazardous: false, signal_loss_policy: { type: "hold_last" }, safe_values: {},
    }, universe: 1, address: index + 1, logical_heads: [],
  });
  await jsonRequest(request, "put", `/api/v1/shows/${show.id}/objects/group/1`, setupSession, { name: "All Dimmers", fixtures: fixtureIds, master: 1, playback_fader: 1 });
  await jsonRequest(request, "post", `/api/v1/shows/${show.id}/open`, setupSession, { transition: "hold_current" });

  await page.goto("/");
  await waitForConnected(page);
  await registerAuditReceiver(page, setupSession);

  await page.locator(".ui-data-table-row:not(.header):not(.empty)").first().click();
  await setDimmerByTouch(page, 75);

  await expect.poll(() => page.evaluate(() => window.__lightAuditEvents.some((event) => event.kind === "command_applied" && event.payload.command === "programmer.set"))).toBeTruthy();
  await waitForDmx(request, 191);

  const audit = await jsonRequest<AuditEvent[]>(request, "get", "/api/v1/audit?after=0", setupSession);
  expect(audit.some((event) => event.kind === "command_applied" && event.payload.command === "programmer.set")).toBeTruthy();

  await page.getByTitle("Open output and timecode controls").click();
  await page.getByRole("button", { name: "BLACKOUT", exact: true }).click();
  await waitForDmx(request, 0);
  await expect.poll(() => page.evaluate(() => window.__lightAuditEvents.some((event) => event.kind === "command_applied" && event.payload.command === "master.set"))).toBeTruthy();
  await page.getByRole("button", { name: "RELEASE BLACKOUT", exact: true }).click();
  await waitForDmx(request, 191);

  await page.locator(".modal-close").click();
  await page.getByRole("button", { name: "CLR", exact: true }).click();
  await page.getByRole("button", { name: "CLR", exact: true }).click();
  await desk.command("GROUP 1 AT 50");
  await expect.poll(() => page.evaluate(() => window.__lightAuditEvents.some((event) => event.kind === "command_applied" && event.payload.command === "programmer.execute"))).toBeTruthy();
  await waitForDmx(request, 128);
});

test("all built-in windows and contextual dialogs are reachable by touch", async ({ page }) => {
  await page.goto("/");
  await waitForConnected(page);
  await page.getByRole("button", { name: "BUILT-INS" }).click();
  for (const [button, windowClass] of [["Stage", ".stage-window"], ["Fixtures", ".fixture-window"], ["Presets", ".pool-window"], ["Playback", ".playback-window"], ["Dynamics", ".dynamics-window"], ["Channels", ".channels-window"], ["DMX", ".dmx-window"]] as const) {
    await page.locator(".dock-entry").filter({ hasText: button }).click();
    await expect(page.locator(windowClass)).toBeVisible();
  }

  await page.getByRole("button", { name: "DESKS" }).click();
  await page.getByRole("button", { name: /Programming/ }).click();
  await page.getByRole("button", { name: "Color", exact: true }).click();
  await expect(page.getByRole("button", { name: /Special Dialog/ })).toBeVisible();
  await page.getByRole("button", { name: /Special Dialog/ }).click();
  await expect(page.getByRole("heading", { name: "Color · Special Dialog" })).toBeVisible();
});

test("patch, store, speed-group, and debug TODO workflows are reachable", async ({ page }) => {
  await page.goto("/");
  await waitForConnected(page);

  await page.getByRole("button", { name: "Open show menu" }).click();
  await page.getByRole("button", { name: "Show Patch", exact: true }).click();
  await expect(page.locator(".patch-window")).toBeVisible();
  await expect(page.locator(".patch-table thead")).toContainText("Location X/Y/Z");
  await expect(page.locator(".patch-table thead")).toContainText("Rotation X/Y/Z");
  await page.getByRole("button", { name: "SET", exact: true }).click();
  await page.locator(".patch-table tbody .patch-value").first().click();
  await expect(page.getByRole("heading", { name: "Set fixture name" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();

  await page.getByRole("button", { name: "BUILT-INS" }).click();
  await page.locator(".dock-entry").filter({ hasText: "Presets" }).click();
  await expect(page.locator(".preset-card")).toHaveCount(200);
  await expect(page.getByRole("button", { name: "Groups", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "REC", exact: true }).click();
  await expect(page.getByRole("button", { name: "REC ARMED", exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "REC", exact: true })).toBeVisible();
  await expect(page.locator(".show-dirty-dot")).toHaveCount(0);
  await page.getByRole("button", { name: "REC", exact: true }).click();
  await page.locator(".preset-card.empty").first().click();
  await expect(page.locator(".show-dirty-dot")).toBeVisible();

  await page.locator(".mode-toggle").click();
  await expect(page.locator(".speed-group-stack button")).toHaveCount(5);
  await page.locator(".mode-toggle").click();

  await page.getByRole("button", { name: "Open show menu" }).click();
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.locator(".show-dirty-dot")).toHaveCount(0);
  await page.getByRole("button", { name: "Debug", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Debug" })).toBeVisible();
  await expect(page.getByText("Server event log")).toBeVisible();
  await page.getByRole("button", { name: "Simulate hardware" }).click();
  await expect(page.getByRole("button", { name: "Hardware connected" })).toBeVisible();
});

test("stage gestures and responsive control acceptance paths are operational", async ({ page }) => {
  await page.goto("/");
  await waitForConnected(page);
  await page.getByRole("button", { name: "BUILT-INS" }).click();
  await page.locator(".dock-entry").filter({ hasText: "Stage" }).click();
  const fixtures = page.locator(".stage-fixture[data-fixture-id]");
  await expect(fixtures).toHaveCount(12);
  await fixtures.nth(0).click();
  await fixtures.nth(1).click({ modifiers: ["Meta"] });
  await expect(page.locator(".stage-fixture.selected")).toHaveCount(2);
  await fixtures.nth(2).click({ modifiers: ["Shift"] });
  await expect(page.locator(".stage-fixture.selected")).toHaveCount(2);
  await fixtures.nth(0).click();
  const second = await fixtures.nth(1).boundingBox(), third = await fixtures.nth(2).boundingBox();
  expect(second).not.toBeNull(); expect(third).not.toBeNull();
  await page.keyboard.down("Meta");
  await page.mouse.move(Math.min(second!.x, third!.x) - 5, Math.min(second!.y, third!.y) - 5);
  await page.mouse.down();
  await page.mouse.move(Math.max(second!.x + second!.width, third!.x + third!.width) + 5, Math.max(second!.y + second!.height, third!.y + third!.height) + 5);
  await page.mouse.up();
  await page.keyboard.up("Meta");
  await expect(page.locator(".stage-fixture.selected")).toHaveCount(3);

  await page.getByRole("button", { name: "DESKS" }).click();
  await expect(page.locator(".dock-list-enter")).toHaveCSS("animation-duration", "0.16s");
  for (const key of ["DIV", "SET", "GRP"]) await expect(page.getByRole("button", { name: key, exact: true })).toBeVisible();
  await expect(page.locator(".numeric-pad")).toHaveCSS("height", /\d+px/);
  const iconBefore = await page.locator(".mode-icon").textContent();
  await page.locator(".mode-toggle").click();
  await expect(page.locator(".mode-icon")).not.toHaveText(iconBefore ?? "");
  await expect(page.locator(".mode-toggle .mode-icon")).toHaveCount(1);
  await expect(page.locator(".save-desk:visible, .dock-operator:visible")).toHaveCount(0);

  await page.getByRole("button", { name: "DESKS" }).click();
  await page.getByRole("button", { name: /New desk/ }).click();
  await page.locator(".empty-desk").click();
  await page.getByRole("button", { name: "Group pool", exact: true }).click();
  const group = page.locator(".group-card").filter({ hasText: "All Dimmers" });
  await group.dispatchEvent("pointerdown");
  await page.waitForTimeout(650);
  await expect(page.getByRole("button", { name: "Select frozen group" })).toBeVisible();
  await expect(page.locator(".group-order")).toContainText("Ordered members: 1.");
  await page.getByRole("button", { name: "Select frozen group" }).click();
});

test("programmer and playback controls keep a stable control-section frame", async ({ page }) => {
  await page.goto("/");
  await waitForConnected(page);
  const section = page.locator(".control-section");
  const programmerBox = await section.boundingBox();
  await page.locator(".mode-toggle").click();
  await expect(page.locator(".playback-fader-bank")).toBeVisible();
  const playbackBox = await section.boundingBox();
  expect(playbackBox).toEqual(programmerBox);
  await expect(page.locator(".playback-tools")).toBeVisible();
});

test("full-HD and landscape-tablet layouts stay fitted and touchable", async ({ page }) => {
  for (const viewport of [{ width: 1920, height: 1080 }, { width: 1024, height: 768 }]) {
    await page.setViewportSize(viewport);
    await page.goto("/");
    await waitForConnected(page);
    const metrics = await page.evaluate(() => ({ bodyWidth: document.body.scrollWidth, bodyHeight: document.body.scrollHeight, viewportWidth: innerWidth, viewportHeight: innerHeight }));
    expect(metrics.bodyWidth).toBeLessThanOrEqual(metrics.viewportWidth);
    expect(metrics.bodyHeight).toBeLessThanOrEqual(metrics.viewportHeight);
    const content = await page.locator(".control-section").boundingBox();
    const right = await page.locator(".control-right-pane").boundingBox();
    expect(content).not.toBeNull(); expect(right).not.toBeNull();
    expect(right!.height).toBeGreaterThanOrEqual(content!.height - 12);
    for (const key of ["DIV", "SET", "GRP", "ENT"]) {
      const box = await page.getByRole("button", { name: key, exact: true }).boundingBox();
      expect(box?.height ?? 0).toBeGreaterThanOrEqual(viewport.width === 1024 ? 35 : 40);
    }
  }
});

test("preload storage and clear keep the active preload scene isolated", async ({ page, request }) => {
  await page.goto("/");
  await waitForConnected(page);
  await page.getByRole("button", { name: "BUILT-INS" }).click();
  await page.locator(".dock-entry").filter({ hasText: "Fixtures" }).click();
  await page.locator(".ui-data-table-row:not(.header):not(.empty)").first().click();
  await page.getByRole("button", { name: /^PRELOAD/ }).click();
  await setDimmerByTouch(page, 75);
  await page.getByRole("button", { name: "PRELOAD GO", exact: true }).click();
  await waitForDmx(request, 191);

  await page.getByRole("button", { name: /^PRELOAD/ }).click();
  await setDimmerByTouch(page, 50);
  await page.getByRole("button", { name: "REC", exact: true }).click();
  await page.locator(".dock-entry").filter({ hasText: "Presets" }).click();
  await page.locator(".preset-card.empty").first().click();
  await page.getByRole("button", { name: "CLR", exact: true }).click();
  await waitForDmx(request, 191);
  const releasePreload = page.getByTitle("Hold to release the active preload scene");
  await expect(releasePreload).toContainText("(Hold: release)");
  const box = await releasePreload.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
  await page.mouse.down();
  await page.waitForTimeout(700);
  await page.mouse.up();
  await expect(page.getByTitle("Hold to release the active preload scene")).toHaveCount(0);
});

declare global {
  interface Window { __lightAuditEvents: AuditEvent[]; __lightAuditSocket: WebSocket }
}
