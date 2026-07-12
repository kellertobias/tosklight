import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

interface Session { session_id: string; token: string }
interface AuditEvent { revision: number; kind: string; payload: Record<string, unknown> }
test.describe.configure({ mode: "serial" });

async function jsonRequest<T>(request: APIRequestContext, method: "get" | "post" | "put", url: string, session?: Session, data?: unknown): Promise<T> {
  const headers = session ? { authorization: `Bearer ${session.token}`, ...(method === "put" ? { "if-match": "0" } : {}) } : undefined;
  const response = await request[method](url, { data, headers });
  expect(response.ok(), `${method.toUpperCase()} ${url}: ${await response.text()}`).toBeTruthy();
  return response.status() === 204 ? undefined as T : response.json() as Promise<T>;
}

async function waitForDmx(request: APIRequestContext, expected: number) {
  await expect.poll(async () => {
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

test("touch programmer path is audited and reaches the rendered DMX output", async ({ page, request }) => {
  const setupSession = await jsonRequest<Session>(request, "post", "/api/v1/sessions", undefined, { username: "Operator" });
  const show = await jsonRequest<{ id: string }>(request, "post", "/api/v1/shows", setupSession, { name: `E2E-${crypto.randomUUID()}`, data_base64: null, overwrite: false });
  const fixtureIds = [crypto.randomUUID(), crypto.randomUUID(), crypto.randomUUID()];
  for (const [index, fixtureId] of fixtureIds.entries()) await jsonRequest(request, "put", `/api/v1/shows/${show.id}/objects/patched_fixture/dimmer-${index + 1}`, setupSession, {
    fixture_id: fixtureId,
    definition: {
      schema_version: 1, id: crypto.randomUUID(), revision: 1, manufacturer: "E2E", model: `Dimmer ${index + 1}`, mode: "1ch", footprint: 1,
      heads: [{ index: 0, name: "Main", shared: true, parameters: [{ attribute: "intensity", components: [{ offset: 0, byte_order: "msb_first" }], default: 0, virtual_dimmer: false, capabilities: [] }] }],
      color_calibration: null, hazardous: false, signal_loss_policy: { type: "hold_last" }, safe_values: {},
    }, universe: 1, address: index + 1, logical_heads: [],
  });
  await jsonRequest(request, "put", `/api/v1/shows/${show.id}/objects/group/1`, setupSession, { name: "All Dimmers", fixtures: fixtureIds, master: 1, playback_fader: 1 });
  await jsonRequest(request, "post", `/api/v1/shows/${show.id}/open`, setupSession, { transition: "hold_current" });

  await page.goto("/");
  await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
  await registerAuditReceiver(page, setupSession);

  await page.getByRole("button", { name: /Dimmer 1, \d+%/ }).click();
  await page.locator(".touch-surface").filter({ hasText: "Dimmer" }).click();

  await expect.poll(() => page.evaluate(() => window.__lightAuditEvents.some((event) => event.kind === "command_applied" && event.payload.command === "programmer.set"))).toBeTruthy();
  await waitForDmx(request, 191);

  const audit = await jsonRequest<AuditEvent[]>(request, "get", "/api/v1/audit?after=0", setupSession);
  expect(audit.some((event) => event.kind === "command_applied" && event.payload.command === "programmer.set")).toBeTruthy();

  await page.getByTitle("Open output and programmer controls").click();
  await page.getByRole("button", { name: "BLACKOUT", exact: true }).click();
  await waitForDmx(request, 0);
  await expect.poll(() => page.evaluate(() => window.__lightAuditEvents.some((event) => event.kind === "command_applied" && event.payload.command === "master.set"))).toBeTruthy();
  await page.getByRole("button", { name: "RELEASE BLACKOUT", exact: true }).click();
  await waitForDmx(request, 191);

  await page.locator(".modal-close").click();
  await page.getByLabel("Command line").fill("FIXTURE 1 AT 50");
  await page.getByRole("button", { name: "ENTER", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__lightAuditEvents.some((event) => event.kind === "command_applied" && event.payload.command === "programmer.execute"))).toBeTruthy();
  await waitForDmx(request, 128);
});

test("all built-in windows and contextual dialogs are reachable by touch", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
  await page.getByRole("button", { name: "BUILT-INS" }).click();
  for (const [button, heading] of [["Stage", "Stage"], ["Groups", "Group Pool"], ["Fixtures", "Fixture Sheet"], ["Presets", "Preset"], ["Playback", "Sequence"], ["Dynamics", "Attribute Dynamics"], ["Channels", "Channels"], ["DMX", "DMX Output"], ["Setup", "Setup"]] as const) {
    await page.locator(".dock-entry").filter({ hasText: button }).click();
    await expect(page.getByRole("heading", { name: new RegExp(heading), exact: false }).first()).toBeVisible();
  }

  await page.getByRole("button", { name: "DESKS" }).click();
  await page.getByRole("button", { name: /Programming/ }).click();
  await page.getByRole("button", { name: "Color", exact: true }).click();
  await expect(page.getByRole("button", { name: /Special Dialog/ })).toBeVisible();
  await page.getByRole("button", { name: /Special Dialog/ }).click();
  await expect(page.getByRole("heading", { name: "Color · Special Dialog" })).toBeVisible();
});

test("stage gestures and responsive control acceptance paths are operational", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
  await page.getByRole("button", { name: "BUILT-INS" }).click();
  await page.locator(".dock-entry").filter({ hasText: "Stage" }).click();
  const fixtures = page.locator(".stage-fixture[data-fixture-id]");
  await expect(fixtures).toHaveCount(3);
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
  for (const key of ["STORE", "SET", "GROUPS"]) await expect(page.getByRole("button", { name: key, exact: true })).toBeVisible();
  await expect(page.locator(".numeric-pad")).toHaveCSS("height", /\d+px/);
  const iconBefore = await page.locator(".mode-icon").textContent();
  await page.locator(".mode-toggle").click();
  await expect(page.locator(".mode-icon")).not.toHaveText(iconBefore ?? "");
  await expect(page.locator(".mode-toggle .mode-icon")).toHaveCount(1);
  await expect(page.locator(".save-desk:visible, .dock-operator:visible")).toHaveCount(0);

  await page.getByRole("button", { name: "BUILT-INS" }).click();
  await page.locator(".dock-entry").filter({ hasText: "Groups" }).click();
  const group = page.locator(".group-card").filter({ hasText: "All Dimmers" });
  await group.dispatchEvent("pointerdown");
  await page.waitForTimeout(650);
  await expect(page.getByRole("button", { name: "Select frozen group" })).toBeVisible();
  await expect(page.locator(".group-order")).toContainText("Ordered members: 1.");
  await page.getByRole("button", { name: "Select frozen group" }).click();
});

test("programmer and playback controls keep a stable control-section frame", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
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
    await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
    const metrics = await page.evaluate(() => ({ bodyWidth: document.body.scrollWidth, bodyHeight: document.body.scrollHeight, viewportWidth: innerWidth, viewportHeight: innerHeight }));
    expect(metrics.bodyWidth).toBeLessThanOrEqual(metrics.viewportWidth);
    expect(metrics.bodyHeight).toBeLessThanOrEqual(metrics.viewportHeight);
    const content = await page.locator(".control-content").boundingBox();
    const right = await page.locator(".control-right-pane").boundingBox();
    expect(content).not.toBeNull(); expect(right).not.toBeNull();
    expect(Math.abs(content!.height - right!.height)).toBeLessThanOrEqual(1);
    for (const key of ["STORE", "SET", "GROUPS", "ENTER"]) {
      const box = await page.getByRole("button", { name: key, exact: true }).boundingBox();
      expect(box?.height ?? 0).toBeGreaterThanOrEqual(viewport.width === 1024 ? 35 : 40);
    }
  }
});

test("preload storage and clear keep the active preload scene isolated", async ({ page, request }) => {
  await page.goto("/");
  await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
  await page.getByRole("button", { name: /Dimmer 1, \d+%/ }).click();
  await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
  await page.locator(".touch-surface").filter({ hasText: "Dimmer" }).click();
  await page.getByRole("button", { name: "PRELOAD GO", exact: true }).click();
  await waitForDmx(request, 191);

  await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
  await page.locator(".touch-surface").filter({ hasText: "Dimmer" }).click();
  await page.getByRole("button", { name: "STORE", exact: true }).click();
  await page.getByLabel("Preset slot").fill(`preload-${Date.now()}`);
  await page.getByRole("button", { name: /Store to Preset/ }).click();
  await page.getByRole("button", { name: "CLEAR", exact: true }).click();
  await waitForDmx(request, 191);
  await expect(page.getByRole("button", { name: /Preload Scene/ })).toBeVisible();
  await expect(page.locator(".preload-scene + .preload-button")).toBeVisible();
  await page.getByRole("button", { name: /Preload Scene/ }).click();
});

declare global {
  interface Window { __lightAuditEvents: AuditEvent[]; __lightAuditSocket: WebSocket }
}
