import type { Locator, Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";

test.describe("docs/testing/09-file-manager-and-text-editor.md", () => {
  test("TEXT-015 @ui › two editors reflect clean saves, surface dirty conflicts, persist association, and recover a deleted file", async ({
    api,
    bench,
    desk,
    page,
    show,
  }) => {
    const name = `text-editor-${crypto.randomUUID()}.md`;
    const renamedName = `text-editor-renamed-${crypto.randomUUID()}.md`;
    await api.request("POST", "/api/v1/files/shows/operations", {
      operation: "create_file",
      sources: [],
      destination: "",
      name,
    });
    const empty = await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`);
    await api.request("PUT", "/api/v1/files/shows/text", {
      path: name,
      text: "Initial run sheet\n",
      revision: empty.revision,
    });

    await desk.open(bench.baseUrl);
    await page.getByRole("button", { name: "DESKTOPS" }).click();
    await page.getByRole("button", { name: /New desktop/ }).click();
    await addTextEditor(page, 0.12);
    await addTextEditor(page, 0.62);

    const editors = page.locator(".desk-pane:has(.text-editor)");
    await expect(editors).toHaveCount(2);
    await chooseFile(page, editors.nth(0), name);
    await chooseFile(page, editors.nth(1), name);
    await expect(editors.nth(0).getByLabel("File text")).toHaveValue("Initial run sheet\n");
    await expect(editors.nth(1).getByLabel("File text")).toHaveValue("Initial run sheet\n");

    await editors.nth(0).getByLabel("File text").fill("House open\nBeginners\n");
    await editors.nth(0).getByRole("button", { name: "Save", exact: true }).click();
    await expect(editors.nth(1).getByLabel("File text")).toHaveValue("House open\nBeginners\n");
    await expect(editors.nth(1).locator(".text-save-state")).toHaveText("Saved");

    await editors.nth(0).getByLabel("File text").fill("External version\n");
    await editors.nth(1).getByLabel("File text").fill("Operator draft\n");
    await editors.nth(0).getByRole("button", { name: "Save", exact: true }).click();
    await expect(editors.nth(1).locator(".text-save-state")).toHaveText("Conflict");
    await expect(editors.nth(1).getByLabel("File text")).toHaveValue("Operator draft\n");
    await editors.nth(1).getByText("Compare versions").click();
    await expect(editors.nth(1).getByLabel("Your unsaved version")).toHaveValue("Operator draft\n");
    await expect(editors.nth(1).getByLabel("Newer file version")).toHaveValue("External version\n");

    page.once("dialog", (dialog) => dialog.accept());
    await editors.nth(1).getByRole("button", { name: "Reload Newer Version" }).click();
    await expect(editors.nth(1).getByLabel("File text")).toHaveValue("External version\n");
    await expect.poll(async () => (await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`)).text)
      .toBe("External version\n");

    const beforeExternalWrite = await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`);
    await api.request("PUT", "/api/v1/files/shows/text", {
      path: name,
      text: "External API update\n",
      revision: beforeExternalWrite.revision,
    });
    await expect(editors.nth(0).getByLabel("File text")).toHaveValue("External API update\n", { timeout: 5_000 });
    await expect(editors.nth(1).getByLabel("File text")).toHaveValue("External API update\n", { timeout: 5_000 });

    await editors.nth(1).getByLabel("File text").fill("Unsaved during external write\n");
    const beforeConflictingWrite = await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`);
    await api.request("PUT", "/api/v1/files/shows/text", {
      path: name,
      text: "Second external API update\n",
      revision: beforeConflictingWrite.revision,
    });
    await expect(editors.nth(0).getByLabel("File text")).toHaveValue("Second external API update\n", { timeout: 5_000 });
    await expect(editors.nth(1).locator(".text-save-state")).toHaveText("Conflict", { timeout: 5_000 });
    await expect(editors.nth(1).getByLabel("File text")).toHaveValue("Unsaved during external write\n");
    page.once("dialog", (dialog) => dialog.accept());
    await editors.nth(1).getByRole("button", { name: "Reload Newer Version" }).click();
    await expect(editors.nth(1).getByLabel("File text")).toHaveValue("Second external API update\n");

    // A completed File Manager/API rename updates the persisted association
    // without discarding an editor's independent unsaved draft.
    await editors.nth(1).getByLabel("File text").fill("Draft retained across rename\n");
    await api.request("POST", "/api/v1/files/shows/operations", {
      operation: "rename",
      sources: [name],
      name: renamedName,
    });
    await expect(editors.nth(0).getByRole("button", { name: renamedName, exact: true })).toBeVisible({ timeout: 5_000 });
    await expect(editors.nth(1).getByRole("button", { name: renamedName, exact: true })).toBeVisible({ timeout: 5_000 });
    await expect(editors.nth(1).getByLabel("File text")).toHaveValue("Draft retained across rename\n");
    await expect(editors.nth(1).locator(".text-save-state")).toHaveText("Unsaved");
    await editors.nth(1).getByRole("button", { name: "Save", exact: true }).click();
    await expect.poll(async () => (await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(renamedName)}`)).text)
      .toBe("Draft retained across rename\n");

    // The root/path association belongs to persisted pane configuration,
    // while the editor text itself remains authoritative in the normal file.
    await expect.poll(async () => {
      const layouts = await api.request<any[]>("GET", `/api/v1/shows/${show.id}/objects/user_layout`);
      return layouts.flatMap((layout) => layout.body.desks)
        .flatMap((configuredDesk: any) => configuredDesk.panes)
        .filter((pane: any) => pane.kind === "text_editor" && pane.textFileRoot === "shows" && pane.textFilePath === renamedName)
        .length;
    }).toBe(2);

    await api.request("POST", "/api/v1/files/shows/operations", { operation: "delete", sources: [renamedName] });
    await expect(editors.nth(0).locator(".text-save-state")).toHaveText("Missing", { timeout: 5_000 });
    await expect(editors.nth(0).getByLabel("File text")).toHaveValue("Draft retained across rename\n");
    page.once("dialog", (dialog) => dialog.accept());
    await editors.nth(0).getByRole("button", { name: "Recreate File" }).click();
    await expect.poll(async () => (await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(renamedName)}`)).text)
      .toBe("Draft retained across rename\n");
    await expect(editors.nth(1).locator(".text-save-state")).toHaveText("Saved");

    await api.request("POST", "/api/v1/files/shows/operations", { operation: "delete", sources: [renamedName] });
  });

  test("TEXT-015 @ui › Open File is root-confined and Pane Settings control read-only and Markdown views", async ({
    api,
    bench,
    desk,
    page,
    show,
  }) => {
    const name = `text-editor-settings-${crypto.randomUUID()}.md`;
    await api.request("POST", "/api/v1/files/shows/operations", {
      operation: "create_file",
      sources: [],
      destination: "",
      name,
    });
    const empty = await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`);
    await api.request("PUT", "/api/v1/files/shows/text", {
      path: name,
      text: "# Cue Sheet\n\n- House open\n- Beginners\n",
      revision: empty.revision,
    });

    await desk.open(bench.baseUrl);
    await page.getByRole("button", { name: "DESKTOPS" }).click();
    await page.getByRole("button", { name: /New desktop/ }).click();
    await addTextEditor(page, 0.3);

    const editor = page.locator(".desk-pane:has(.text-editor)");
    await editor.getByRole("button", { name: "Open File", exact: true }).click();
    const picker = page.getByRole("dialog", { name: "Choose files or folders" });
    await expect(picker).toBeVisible();
    await expect(picker.getByRole("button", { name: "Open system file picker" })).toHaveCount(0);
    await picker.getByRole("button", { name: `${name}, file` }).click();
    await picker.getByRole("button", { name: "Select", exact: true }).click();
    await expect(editor.getByLabel("File text")).toHaveValue("# Cue Sheet\n\n- House open\n- Beginners\n");

    await editor.getByRole("button", { name: "Settings", exact: true }).click();
    let settings = page.getByRole("dialog", { name: "Pane Settings" });
    await settings.getByRole("tab", { name: "Text Editor", exact: true }).click();
    let readOnlySwitch = settings.getByRole("switch", { name: "Read-only pane" });
    await readOnlySwitch.locator("xpath=..").click();
    await expect(readOnlySwitch).toBeChecked();
    await settings.getByRole("radio", { name: "Rendered Markdown" }).click();
    await settings.getByRole("button", { name: "Close settings" }).click();

    await expect(editor.getByLabel("File text")).toHaveCount(0);
    const rendered = editor.getByRole("article", { name: "Rendered Markdown" });
    await expect(rendered.getByRole("heading", { name: "Cue Sheet" })).toBeVisible();
    await expect(rendered).toContainText("House open");
    await expect(editor.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
    await expect(editor.getByRole("button", { name: "Save As" })).toBeDisabled();
    await expect(editor.getByText("This pane is configured read-only.", { exact: false })).toBeVisible();

    await expect.poll(async () => {
      const layouts = await api.request<any[]>("GET", `/api/v1/shows/${show.id}/objects/user_layout`);
      return layouts.flatMap((layout) => layout.body.desks)
        .flatMap((configuredDesk: any) => configuredDesk.panes)
        .find((pane: any) => pane.kind === "text_editor" && pane.textFilePath === name);
    }).toEqual(expect.objectContaining({ textEditorReadOnly: true, textEditorMode: "markdown" }));

    await editor.getByRole("button", { name: "Settings", exact: true }).click();
    settings = page.getByRole("dialog", { name: "Pane Settings" });
    await settings.getByRole("tab", { name: "Text Editor", exact: true }).click();
    readOnlySwitch = settings.getByRole("switch", { name: "Read-only pane" });
    await readOnlySwitch.locator("xpath=..").click();
    await expect(readOnlySwitch).not.toBeChecked();
    await settings.getByRole("radio", { name: "Edit + Markdown" }).click();
    await settings.getByRole("button", { name: "Close settings" }).click();

    await expect(editor.getByLabel("File text")).toBeEditable();
    await expect(editor.getByRole("article", { name: "Rendered Markdown" })).toBeVisible();
    await expect(editor.locator(".text-editor-content.mode-split")).toBeVisible();
    await editor.getByLabel("File text").fill("# Updated Cue Sheet\n\nStand by.\n");
    await expect(editor.getByRole("article", { name: "Rendered Markdown" }).getByRole("heading", { name: "Updated Cue Sheet" })).toBeVisible();
    await editor.getByRole("button", { name: "Save", exact: true }).click();
    await expect.poll(async () => (await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(name)}`)).text)
      .toBe("# Updated Cue Sheet\n\nStand by.\n");

    await editor.getByRole("button", { name: "Settings", exact: true }).click();
    settings = page.getByRole("dialog", { name: "Pane Settings" });
    await settings.getByRole("tab", { name: "Text Editor", exact: true }).click();
    await settings.getByRole("radio", { name: "Plain Text" }).click();
    await settings.getByRole("button", { name: "Close settings" }).click();
    await expect(editor.getByLabel("File text")).toBeVisible();
    await expect(editor.getByRole("article", { name: "Rendered Markdown" })).toHaveCount(0);

    await api.request("POST", "/api/v1/files/shows/operations", { operation: "delete", sources: [name] });
  });
});

async function chooseFile(page: Page, editor: Locator, name: string) {
  await editor.getByRole("button", { name: "Choose File…" }).click();
  await page.getByRole("option", { name, exact: true }).click();
}

async function addTextEditor(page: Page, horizontalPosition: number) {
  const grid = page.locator(".desk-grid");
  const box = await grid.boundingBox();
  expect(box).not.toBeNull();
  const before = await page.locator(".desk-pane:has(.text-editor)").count();
  await page.mouse.click(box!.x + box!.width * horizontalPosition, box!.y + box!.height * 0.15);
  await expect(page.getByRole("heading", { name: "Open Window" })).toBeVisible();
  await page.getByRole("button", { name: "Text Editor", exact: true }).click();
  await expect(page.locator(".desk-pane:has(.text-editor)")).toHaveCount(before + 1);
}
