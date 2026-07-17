import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";

test.describe("docs/testing/09-file-manager-and-text-editor.md", () => {
  test("FILE-001 @api › default root is confined and supports revision-safe UTF-8 text", async ({ api }) => {
    const roots = await api.request<any[]>("GET", "/api/v1/files/roots");
    expect(roots.some((root) => root.id === "shows" && root.label === "Shows")).toBe(true);
    const name = `operator-notes-${crypto.randomUUID()}.txt`;
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: "", name });
    const created = await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`);
    const saved = await api.request<any>("PUT", "/api/v1/files/shows/text", { path: name, text: "Preset check\nStandby cue 12\n", revision: created.revision });
    expect(saved.text).toBe("Preset check\nStandby cue 12\n");
    await expect(api.request("PUT", "/api/v1/files/shows/text", { path: name, text: "stale", revision: created.revision })).rejects.toThrow(/409.*changed since it was opened/);
    await expect(api.request("GET", "/api/v1/files/shows/entries?path=..%2F")).rejects.toThrow(/400.*may not traverse parents/);
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "delete", sources: [name] });
  });

  test("FILE-002 @ui › File Manager provides the three-column browsing workflow and text editor", async ({ api, bench, desk, page }) => {
    const name = `run-sheet-${crypto.randomUUID()}.md`;
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: "", name });
    const created = await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`);
    await api.request("PUT", "/api/v1/files/shows/text", { path: name, text: "House open", revision: created.revision });
    await desk.open(bench.baseUrl);
    const manager = await addPane(page, "File Manager");
    await expect(manager.locator(".file-columns")).toBeVisible();
    await expect(manager.getByRole("heading", { name: "Locations" })).toBeVisible();
    await expect(manager.getByRole("heading", { name: "Properties" })).toBeVisible();
    const row = manager.getByRole("button", { name: new RegExp(name) });
    await expect(row).toBeVisible();
    await row.dblclick();
    const editor = manager.locator(".file-editor");
    await expect(editor).toBeVisible();
    await editor.getByLabel("File text").fill("House open\nBeginners");
    await editor.getByRole("button", { name: "Save" }).click();
    await expect.poll(async () => (await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`)).text).toContain("Beginners");
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "delete", sources: [name] });
  });

  test("TEXT-001 @ui › dedicated Text Editor persists its file association and reports dirty state", async ({ api, bench, desk, page }) => {
    const name = `cue-notes-${crypto.randomUUID()}.txt`;
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: "", name });
    const created = await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`);
    await api.request("PUT", "/api/v1/files/shows/text", { path: name, text: "Cue 1", revision: created.revision });
    await desk.open(bench.baseUrl);
    const editor = await addPane(page, "Text Editor");
    await editor.getByRole("button", { name: "Choose File…" }).click();
    await page.getByRole("option", { name }).click();
    await expect(editor.getByLabel("File text")).toHaveValue("Cue 1");
    await editor.getByLabel("File text").fill("Cue 1\nCheck follow spot");
    await expect(editor.getByLabel("File text")).toHaveValue("Cue 1\nCheck follow spot");
    await expect(editor.locator(".text-save-state")).toHaveText("Unsaved");
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect(editor.locator(".text-save-state")).toHaveText("Saved");
    await expect.poll(async () => (await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`)).text).toContain("follow spot");
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "delete", sources: [name] });
  });
});

async function addPane(page: Page, name: "File Manager" | "Text Editor") {
  await page.getByRole("button", { name: "DESKTOPS" }).click();
  await page.getByRole("button", { name: /New desktop/ }).click();
  const grid = page.locator(".desk-grid");
  const box = await grid.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.click(box!.x + Math.min(180, box!.width / 4), box!.y + Math.min(120, box!.height / 4));
  await expect(page.getByRole("heading", { name: "Open Window" })).toBeVisible();
  await page.getByRole("button", { name, exact: true }).click();
  const pane = page.locator(".desk-pane").filter({ hasText: name });
  await expect(pane).toBeVisible();
  if (name === "File Manager") {
    const gridBox = await grid.boundingBox();
    const handleBox = await pane.locator(".pane-resize-handle").boundingBox();
    expect(gridBox).not.toBeNull();
    expect(handleBox).not.toBeNull();
    await page.mouse.move(handleBox!.x + handleBox!.width / 2, handleBox!.y + handleBox!.height / 2);
    await page.mouse.down();
    await page.mouse.move(gridBox!.x + gridBox!.width * 0.95, gridBox!.y + gridBox!.height * 0.9, { steps: 5 });
    await page.mouse.up();
    await expect.poll(async () => (await pane.boundingBox())?.width ?? 0).toBeGreaterThan(900);
  }
  return pane;
}
