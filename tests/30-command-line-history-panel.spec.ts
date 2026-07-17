import type { Locator, Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { loadCanonicalCopy } from "./support/catalog";

type HistoryEntry = {
  id: string;
  command: string;
  status: "accepted" | "rejected";
  feedback: string;
  source: "software" | "osc";
};

type HistoryState = { observed?: HistoryEntry[] };

pairedScenario<HistoryState>({
  id: "COMMAND-HISTORY-001",
  title: "Command Line history shows accepted and rejected desk commands once",
  arrange: async ({ api, bench }, surface) => {
    await loadCanonicalCopy(api, bench, `command-history-001-${surface}`, "default-stage");
    return {};
  },
  api: async ({ api }, state) => {
    await execute(api, "FIXTURE 1 AT 25");
    await executeRejected(api, "FIXTURE 1 AT 101");
    state.observed = await history(api);
  },
  ui: async ({ api, desk, page }, state) => {
    await desk.open(api.baseUrl);
    await enterCommand(page, "FIXTURE 1 AT 25");
    await enterCommand(page, "FIXTURE 1 AT 101");
    await expect.poll(async () => (await history(api)).length).toBe(2);

    const input = page.getByRole("textbox", { name: "Command line" });
    await input.click();
    const panel = page.getByRole("dialog", { name: "Command line history" });
    await expect(panel).toBeVisible();
    await expect(panel.locator(".command-history-entry")).toHaveCount(2);
    state.observed = await history(api);
  },
  assert: async (_context, state) => {
    expect(state.observed?.map((entry) => ({ command: entry.command, status: entry.status }))).toEqual([
      { command: "FIXTURE 1 AT 101", status: "rejected" },
      { command: "FIXTURE 1 AT 25", status: "accepted" },
    ]);
    expect(state.observed?.[0].feedback).toMatch(/within 0-100/i);
    expect(state.observed?.[1].feedback).toBe("Applied to 1 target(s)");
  },
});

test("COMMAND-HISTORY-001 @supplemental-ui › inspection, reuse, dismissal, reconnect, and hardware layout preserve unfinished input", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "command-history-001-ui-boundaries", "default-stage");
  await execute(api, "FIXTURE 1 AT 25");
  await desk.open(api.baseUrl);

  const input = page.getByRole("textbox", { name: "Command line" });
  await input.fill("GROUP 3 AT");
  const geometry = await commandGeometry(page);
  await input.click();
  let panel = page.getByRole("dialog", { name: "Command line history" });
  await expect(panel).toBeVisible();
  await expect(input).toHaveValue("GROUP 3 AT");
  await expect(panel.getByText("Accepted", { exact: true })).toBeVisible();
  await expect(panel.getByText("FIXTURE 1 AT 25", { exact: true })).toBeVisible();
  expect(await commandGeometry(page)).toEqual(geometry);
  await expectPanelAboveCommand(panel, page);

  await page.keyboard.press("Escape");
  await expect(panel).toBeHidden();
  await expect(input).toHaveValue("GROUP 3 AT");

  await input.click();
  await panel.getByText("FIXTURE 1 AT 25", { exact: true }).click();
  await expect(input).toHaveValue("GROUP 3 AT");
  expect(await history(api)).toHaveLength(1);
  await panel.getByRole("button", { name: "Reuse", exact: true }).click();
  await expect(input).toHaveValue("FIXTURE 1 AT 25");
  expect(await history(api)).toHaveLength(1);
  await input.press("Enter");
  await expect.poll(async () => (await history(api)).length).toBe(2);

  await page.locator(".command-escape").click();
  await input.fill("FIXTURE 7 AT");
  await input.click();
  await page.locator(".mode-toggle").dispatchEvent("pointerdown");
  await expect(panel).toBeHidden();
  await expect(input).toHaveValue("FIXTURE 7 AT");

  await page.reload();
  await expect(page.getByRole("textbox", { name: "Command line" })).toHaveValue("FIXTURE 7 AT");
  await page.getByRole("textbox", { name: "Command line" }).click();
  panel = page.getByRole("dialog", { name: "Command line history" });
  await expect(panel.locator(".command-history-entry")).toHaveCount(2);
  await panel.getByRole("button", { name: "Close command line history" }).click();

  const hardware = await bench.osc();
  const clientId = `command-history-${crypto.randomUUID()}`;
  try {
    await hardware.subscribe(clientId, api.session!.desk.osc_alias);
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);
    const alias = api.session!.desk.osc_alias;
    await hardware.send(`/light/${alias}/programmer/clear`, [true]);
    for (const action of ["fixture", "digit-1", "at", "digit-3", "digit-5", "enter"])
      await hardware.send(`/light/${alias}/programmer/${action}`, [true]);
    await expect.poll(async () => (await history(api))[0]?.source).toBe("osc");
    await page.getByRole("textbox", { name: "Command line" }).click();
    panel = page.getByRole("dialog", { name: "Command line history" });
    await expect(panel.getByText("attached hardware", { exact: false }).first()).toBeVisible();
    await expectPanelAboveCommand(panel, page);
  } finally {
    await hardware.send("/light/unsubscribe", [clientId]).catch(() => undefined);
    await hardware.close();
  }
});

async function execute(api: ApiDriver, value: string) {
  await api.command("programmer.execute", { value });
}

async function executeRejected(api: ApiDriver, value: string) {
  await expect(execute(api, value)).rejects.toThrow();
}

async function history(api: ApiDriver): Promise<HistoryEntry[]> {
  return api.request("GET", "/api/v1/command-history");
}

async function enterCommand(page: Page, value: string) {
  const input = page.getByRole("textbox", { name: "Command line" });
  if (await input.evaluate((element) => element.classList.contains("completed")))
    await page.locator(".command-escape").click();
  await input.fill(value);
  await input.press("Enter");
}

async function commandGeometry(page: Page) {
  return page.locator(".command-line-bar").evaluate((element) => {
    const bounds = element.getBoundingClientRect();
    return { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height };
  });
}

async function expectPanelAboveCommand(panel: Locator, page: Page) {
  const panelBounds = await panel.boundingBox();
  const commandBounds = await page.locator(".command-line-bar").boundingBox();
  expect(panelBounds).not.toBeNull();
  expect(commandBounds).not.toBeNull();
  expect(panelBounds!.y).toBeLessThanOrEqual(16);
  expect(panelBounds!.y + panelBounds!.height).toBeLessThan(commandBounds!.y);
}
