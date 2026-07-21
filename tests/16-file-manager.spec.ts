import type { Locator, Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { ControllableHostedFilePickerDriver } from "../apps/control-ui/e2e/bench/hostedFilePicker";
import fs from "node:fs/promises";
import path from "node:path";

test.describe("docs/testing/09-file-manager-and-text-editor.md", () => {
  test("FILE-016 @api › confined file services authenticate, stream ranges, expose native capabilities, and resolve conflicts", async ({ api, bench, request }) => {
    const unauthenticated = await request.get(`${bench.baseUrl}/api/v1/files/roots`);
    expect(unauthenticated.status()).toBe(401);
    const authorization = { authorization: `Bearer ${api.session!.token}` };
    const rootsResponse = await request.get(`${bench.baseUrl}/api/v1/files/roots`, { headers: authorization });
    expect(rootsResponse.status()).toBe(200);
    const roots = await rootsResponse.json() as Array<Record<string, any>>;
    expect(roots).toEqual(expect.arrayContaining([expect.objectContaining({ id: "shows", label: "Shows", removable: false })]));
    expect(roots[0]).not.toHaveProperty("path");
    expect(roots[0].capabilities).toEqual(expect.objectContaining({ range_streaming: true, thumbnails: true, native_notes: expect.any(Boolean), trash: expect.any(Boolean) }));

    const workspace = `file-contract-${crypto.randomUUID()}`;
    try {
      await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_folder", sources: [], destination: "", name: workspace });
      await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_folder", sources: [], destination: workspace, name: "Destination" });
      await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: workspace, name: "range.txt" });
      await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: workspace, name: ".hidden" });
      const document = await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(`${workspace}/range.txt`)}`);
      await api.request("PUT", "/api/v1/files/shows/text", { path: `${workspace}/range.txt`, text: "0123456789", revision: document.revision });

      const ordinary = await api.request<any>("GET", `/api/v1/files/shows/entries?path=${encodeURIComponent(workspace)}`);
      expect(ordinary.entries.map((entry: any) => entry.name)).not.toContain(".hidden");
      const withHidden = await api.request<any>("GET", `/api/v1/files/shows/entries?path=${encodeURIComponent(workspace)}&hidden=true`);
      expect(withHidden.entries.map((entry: any) => entry.name)).toContain(".hidden");

      const traversal = await request.get(`${bench.baseUrl}/api/v1/files/shows/entries?path=${encodeURIComponent("../")}`, { headers: authorization });
      expect(traversal.status()).toBe(400);
      const range = await request.get(`${bench.baseUrl}/api/v1/files/shows/content?path=${encodeURIComponent(`${workspace}/range.txt`)}`, { headers: { ...authorization, range: "bytes=2-5" } });
      expect(range.status()).toBe(206);
      expect(range.headers()["accept-ranges"]).toBe("bytes");
      expect(range.headers()["content-range"]).toBe("bytes 2-5/10");
      expect(await range.text()).toBe("2345");
      const suffix = await request.get(`${bench.baseUrl}/api/v1/files/shows/content?path=${encodeURIComponent(`${workspace}/range.txt`)}`, { headers: { ...authorization, range: "bytes=-3" } });
      expect(await suffix.text()).toBe("789");

      const metadata = await api.request<any>("GET", `/api/v1/files/shows/metadata?path=${encodeURIComponent(`${workspace}/range.txt`)}`);
      expect(metadata).toEqual(expect.objectContaining({ name: "range.txt", mime: "text/plain; charset=utf-8" }));
      expect(metadata.created_millis === null || typeof metadata.created_millis === "number").toBe(true);
      if (metadata.note_supported) {
        await api.request("PUT", "/api/v1/files/shows/notes", { path: `${workspace}/range.txt`, note: "Operator metadata" });
        const note = await api.request<any>("GET", `/api/v1/files/shows/notes?path=${encodeURIComponent(`${workspace}/range.txt`)}`);
        expect(note).toEqual(expect.objectContaining({ supported: true, note: "Operator metadata" }));
        const names = (await api.request<any>("GET", `/api/v1/files/shows/entries?path=${encodeURIComponent(workspace)}&hidden=true`)).entries.map((entry: any) => entry.name as string);
        expect(names.some((name: string) => /tosklight.*note|\.note/i.test(name))).toBe(false);
      }

      await api.request("POST", "/api/v1/files/shows/operations", { operation: "copy", sources: [`${workspace}/range.txt`], destination: `${workspace}/Destination` });
      const conflict = await request.post(`${bench.baseUrl}/api/v1/files/shows/operations`, { headers: { ...authorization, "content-type": "application/json" }, data: { operation: "copy", sources: [`${workspace}/range.txt`], destination: `${workspace}/Destination` } });
      expect(conflict.status()).toBe(409);
      const keepBoth = await api.request<any>("POST", "/api/v1/files/shows/operations", { operation: "copy", sources: [`${workspace}/range.txt`], destination: `${workspace}/Destination`, conflict: "keep_both", apply_to_all: true });
      expect(keepBoth).toEqual(expect.objectContaining({ complete: true, paths: expect.arrayContaining([`${workspace}/Destination/range copy.txt`]) }));

      await api.setCommandLineText("COPY");
      const claimed = await api.request<any>("POST", "/api/v1/files/input-context", { instance_id: "acceptance-file-manager", action: "copy", origin: "pending" });
      expect(claimed).toEqual(expect.objectContaining({ instance_id: "acceptance-file-manager", action: "copy", session_id: api.session!.session_id, desk_id: api.session!.desk.id }));
      const programmers = await api.request<any[]>("GET", "/api/v1/programmers");
      expect(programmers.find((programmer) => programmer.session_id === api.session!.session_id)?.command_line).toBe("");
      const competingClaim = await request.post(`${bench.baseUrl}/api/v1/files/input-context`, { headers: { ...authorization, "content-type": "application/json" }, data: { instance_id: "another-pane", action: "copy", origin: "toolbar" } });
      expect(competingClaim.status()).toBe(409);
      const hardware = await bench.osc();
      try {
        const alias = api.session!.desk.osc_alias;
        await hardware.subscribe(`file-manager-${crypto.randomUUID()}`, alias);
        await hardware.send(`/light/${alias}/programmer/enter`, [true]);
        await expect.poll(() => api.request<any>("GET", "/api/v1/files/input-context"))
          .toEqual(expect.objectContaining({ instance_id: "acceptance-file-manager", action: "copy" }));
        expect((await api.request<any[]>("GET", "/api/v1/programmers")).find((programmer) => programmer.session_id === api.session!.session_id)?.command_line).toBe("");
        await hardware.send(`/light/${alias}/programmer/escape`, [true]);
        await expect.poll(() => api.request("GET", "/api/v1/files/input-context")).toBeNull();
      } finally {
        await hardware.close();
      }
    } finally {
      await api.request("POST", "/api/v1/files/shows/operations", { operation: "delete", sources: [workspace] }).catch(() => undefined);
    }
  });

  test("FILE-016 @ui › three-column browsing and file operations share one visible state machine", async ({ api, bench, desk, page }) => {
    const showsRoot = (await api.request<any[]>("GET", "/api/v1/files/roots")).find((root) => root.id === "shows");
    const workspace = `file-manager-${crypto.randomUUID()}`;
    const destination = `${workspace}/Destination`;
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_folder", sources: [], destination: "", name: workspace });
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_folder", sources: [], destination: workspace, name: "Destination" });
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: workspace, name: "alpha.txt" });
    await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: workspace, name: ".operator-note" });
    await fs.writeFile(path.join(bench.dataDir, "shows", workspace, "walk-in.wav"), minimalWave());

    await desk.open(bench.baseUrl);
    await addFileManagerPane(page);
    const manager = page.locator(".desk-pane").filter({ hasText: "File Manager" });
    await expect(manager.locator(".file-columns")).toBeVisible();
    await expect(manager.getByRole("heading", { name: "Locations" })).toBeVisible();
    await expect(manager.getByRole("heading", { name: "Properties" })).toBeVisible();
    const header = manager.locator(".file-manager-header-actions");
    await expect(header.getByRole("button", { name: "Edit", exact: true })).toBeVisible();
    await expect(header.getByRole("button", { name: "New", exact: true })).toBeVisible();
    await expect(header.getByRole("button", { name: "View", exact: true })).toBeVisible();
    await expect(manager.getByText("Browse and manage files", { exact: true })).toBeVisible();
    await expect(manager.getByRole("button", { name: "Close File Manager" })).toHaveCount(0);
    await expect(manager.getByRole("button", { name: "Show hidden files" })).toHaveCount(0);

    await manager.getByRole("button", { name: `${workspace}, folder` }).dblclick();
    await expect(manager.getByRole("navigation", { name: "Breadcrumb" })).toContainText(workspace);
    await expect(manager.getByRole("button", { name: `Current path /${workspace}` })).toHaveText(`/${workspace}`);
    const visibleRows = manager.locator("main[aria-label='Directory contents'] > button");
    await expect(visibleRows).toHaveCount(3);
    await expect(visibleRows.nth(0)).toHaveAttribute("aria-label", "Destination, folder");
    await expect(visibleRows.nth(1)).toHaveAttribute("aria-label", "alpha.txt, file");
    await expect(visibleRows.nth(2)).toHaveAttribute("aria-label", "walk-in.wav, file");

    await manager.getByRole("button", { name: "walk-in.wav, file" }).click();
    const player = propertiesFor(manager).getByLabel("Audio preview of walk-in.wav");
    await expect(player).toHaveAttribute("src", /ticket=/);
    await expect(player).not.toHaveAttribute("src", /^blob:/);
    const streamed = await player.evaluate(async (audio: HTMLAudioElement) => {
      const response = await fetch(audio.src, { headers: { Range: "bytes=0-3" } });
      return {
        status: response.status,
        range: response.headers.get("content-range"),
        bytes: [...new Uint8Array(await response.arrayBuffer())],
      };
    });
    expect(streamed).toEqual({ status: 206, range: `bytes 0-3/${minimalWave().length}`, bytes: [82, 73, 70, 70] });

    await header.getByRole("button", { name: "View", exact: true }).click();
    await page.getByRole("menu", { name: "View menu" }).getByRole("menuitemcheckbox", { name: "Show Hidden Files", exact: true }).click();
    await expect(manager.getByRole("button", { name: ".operator-note, file" })).toBeVisible();
    await manager.getByRole("button", { name: "alpha.txt, file" }).click();
    const properties = manager.getByRole("complementary", { name: "Selection properties" });
    await expect(properties).toContainText("alpha.txt");
    if (showsRoot?.capabilities?.native_notes) {
      await expect(properties.getByLabel("Notes")).toBeEnabled();
      await properties.getByLabel("Notes").fill("UI native note");
      await properties.getByRole("button", { name: "Save Note" }).click();
      await expect(manager.getByRole("status")).toContainText("Native filesystem note saved");
    } else {
      await expect(properties.getByLabel("Notes")).toBeDisabled();
    }

    await beginFileEdit(page, manager, "Copy");
    await expect(manager.getByRole("button", { name: "Copy Here" })).toBeVisible();
    await expect(manager.getByRole("button", { name: "Rename" })).toHaveCount(0);
    await manager.getByRole("button", { name: "Destination, folder" }).dblclick();
    await manager.getByRole("button", { name: "Copy Here" }).click();
    await expect.poll(async () => (await api.request<any>("GET", `/api/v1/files/shows/entries?path=${encodeURIComponent(destination)}`)).entries.map((entry: any) => entry.name)).toContain("alpha.txt");

    await manager.getByRole("navigation", { name: "Breadcrumb" }).getByRole("button", { name: `/ ${workspace}` }).click();
    await manager.getByRole("button", { name: "alpha.txt, file" }).click();
    await beginFileEdit(page, manager, "Copy");
    await manager.getByRole("button", { name: "Destination, folder" }).dblclick();
    await manager.getByRole("button", { name: "Copy Here" }).click();
    const conflict = manager.getByRole("dialog", { name: "Resolve name conflict" });
    await expect(conflict.getByRole("button", { name: "Replace" })).toBeVisible();
    await expect(conflict.getByRole("button", { name: "Skip" })).toBeVisible();
    await conflict.getByRole("button", { name: "Keep Both" }).click();
    await expect(manager.getByRole("button", { name: "alpha copy.txt, file" })).toBeVisible();

    const copied = manager.getByRole("button", { name: "alpha.txt, file" });
    await copied.click();
    await beginFileEdit(page, manager, "Rename");
    await manager.getByLabel("New name").fill("renamed.txt");
    await manager.getByRole("button", { name: "Rename" }).click();
    await expect(manager.getByRole("button", { name: "renamed.txt, file" })).toBeVisible();

    await manager.getByRole("button", { name: "renamed.txt, file" }).click();
    await beginFileEdit(page, manager, "Delete");
    const confirmation = manager.getByRole("dialog", { name: /Confirm (?:move to trash|permanent deletion)/ });
    await expect(confirmation).toContainText(/platform Trash|deletion is permanent/);
    await confirmation.getByRole("button", { name: "Cancel" }).click();
    await expect(manager.getByRole("button", { name: "renamed.txt, file" })).toBeVisible();
    await beginFileEdit(page, manager, "Delete");
    await manager.getByRole("button", { name: /Move to Trash|Delete Permanently/ }).click();
    await expect(manager.getByRole("button", { name: "renamed.txt, file" })).toHaveCount(0);

    await header.getByRole("button", { name: "View", exact: true }).click();
    await page.getByRole("menu", { name: "View menu" }).getByRole("menuitemradio", { name: "Grid", exact: true }).click();
    await expect(manager.locator("main.file-grid")).toBeVisible();
    await manager.getByRole("button", { name: "Back", exact: true }).click();
    await manager.getByRole("button", { name: "Forward", exact: true }).click();
    await expect(manager.getByRole("navigation", { name: "Breadcrumb" })).toContainText("Destination");

    await api.request("POST", "/api/v1/files/shows/operations", { operation: "delete", sources: [workspace] });
  });

  test("FILE-016 @ui › Shows and recovery launches File Manager and Fixture Library without dedicated setup sections", async ({ bench, desk, page }) => {
    await desk.open(bench.baseUrl);
    await page.getByRole("button", { name: /Open show menu/ }).click();
    await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
    const setupNav = page.locator(".setup-window nav");
    await expect(setupNav.getByRole("button", { name: "File Manager", exact: true })).toHaveCount(0);
    await expect(setupNav.getByRole("button", { name: "Fixture library", exact: true })).toHaveCount(0);

    await page.getByRole("button", { name: "Open File Manager", exact: true }).click();
    await expect(page.locator(".file-manager")).toBeVisible();
    await expect(page.getByRole("button", { name: "Close File Manager" })).toBeVisible();
    await page.getByRole("button", { name: "Close File Manager" }).click();
    await expect(page.locator(".file-manager")).toHaveCount(0);
    await expect(page.locator(".setup-window")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Shows & recovery" })).toBeVisible();

    await page.getByRole("button", { name: "Open Fixture Library", exact: true }).click();
    const fixtureLibrary = page.getByRole("dialog", { name: "Fixture Library" });
    await expect(fixtureLibrary).toBeVisible();

    const viewport = page.viewportSize();
    const libraryBox = await fixtureLibrary.boundingBox();
    expect(viewport).not.toBeNull();
    expect(libraryBox).not.toBeNull();
    expect(libraryBox!.x).toBeCloseTo(12, 0);
    expect(viewport!.width - (libraryBox!.x + libraryBox!.width)).toBeCloseTo(12, 0);

    const titleBar = fixtureLibrary.locator(":scope > .ui-modal-titlebar");
    const closeHeight = await titleBar.getByRole("button", { name: "Close Fixture Library" }).evaluate((button) => button.getBoundingClientRect().height);
    for (const name of ["Import fixture", "Import GDTF", "Create fixture"]) {
      const action = titleBar.getByRole("button", { name, exact: true });
      await expect(action).toBeVisible();
      await expect.poll(() => action.evaluate((button) => button.getBoundingClientRect().height)).toBe(closeHeight);
    }
  });

  test("FILE-016 @ui › hosted picker supports every target, cardinality, filter, initial location, Select, ENTER, and ESC", async ({ bench, desk, page }) => {
    const workspace = `picker-${crypto.randomUUID()}`;
    const directory = path.join(bench.dataDir, "shows", workspace);
    await fs.mkdir(path.join(directory, "Folder"), { recursive: true });
    await fs.writeFile(path.join(directory, "allowed.txt"), "allowed");
    await fs.writeFile(path.join(directory, "blocked.png"), "not really an image");
		const picker = new ControllableHostedFilePickerDriver(page);
		await picker.install();
		try {
			await desk.open(bench.baseUrl);

			const fileOutcome = picker.open({
				target: "files",
				multiple: false,
				allowedExtensions: ["txt"],
				initialRootId: "shows",
				initialDirectory: workspace,
			});
			let dialog = page.getByRole("dialog", {
				name: "Choose files or folders",
			});
			await expect(
				dialog.getByRole("heading", { name: "File Manager" }),
			).toBeVisible();
			await expect(dialog.getByText("Select a file", { exact: true })).toBeVisible();
			await expect(
				dialog.getByRole("button", { name: "Close File Manager" }),
			).toBeVisible();
			await expect(
				dialog.getByRole("button", { name: "Edit", exact: true }),
			).toBeVisible();
			await expect(
				dialog.getByRole("button", { name: "New", exact: true }),
			).toBeVisible();
			await expect(
				dialog.getByRole("button", { name: "View", exact: true }),
			).toBeVisible();
			await expect(
				dialog.getByRole("navigation", { name: "Breadcrumb" }),
			).toContainText(workspace);
			await dialog.getByRole("button", { name: "blocked.png, file" }).click();
			await expect(
				dialog.getByRole("button", { name: "Select", exact: true }),
			).toBeDisabled();
			expect(picker.pendingRequests).toBe(1);
			await dialog.getByRole("button", { name: "allowed.txt, file" }).click();
			await expect(dialog).toBeVisible();
			await expect(
				dialog.getByRole("button", { name: "Select", exact: true }),
			).toBeEnabled();
			await page.keyboard.press("Enter");
			await expect(dialog).toHaveCount(0);
			expect(await fileOutcome).toEqual({
				status: "selected",
				selections: [
					expect.objectContaining({ path: `${workspace}/allowed.txt` }),
				],
			});

			const folderOutcome = picker.open({
				target: "folders",
				multiple: false,
				initialRootId: "shows",
				initialDirectory: workspace,
			});
			dialog = page.getByRole("dialog", { name: "Choose files or folders" });
			await dialog.getByRole("button", { name: "Folder, folder" }).click();
			await page.keyboard.press("Escape");
			await expect(dialog).toHaveCount(0);
			expect(await folderOutcome).toEqual({ status: "cancelled" });

			const multipleOutcome = picker.open({
				target: "either",
				multiple: true,
				initialRootId: "shows",
				initialDirectory: workspace,
			});
			dialog = page.getByRole("dialog", { name: "Choose files or folders" });
			await dialog.getByRole("button", { name: "Folder, folder" }).click();
			await dialog
				.getByRole("button", { name: "allowed.txt, file" })
				.click({ modifiers: ["ControlOrMeta"] });
			await expect(dialog).toBeVisible();
			await dialog.getByRole("button", { name: "Select", exact: true }).click();
			await expect(dialog).toHaveCount(0);
			const result = await multipleOutcome;
			expect(result.status).toBe("selected");
			if (result.status === "selected") {
				expect(result.selections.map(({ path }) => path).sort()).toEqual(
					[`${workspace}/Folder`, `${workspace}/allowed.txt`].sort(),
				);
			}
		} finally {
			await picker.dispose();
		}
  });

  test("FILE-016 @ui › form fields use the confined picker first and expose a constrained system fallback only when enabled", async ({ api, bench, desk, page }) => {
    await desk.open(bench.baseUrl);
    expect((await api.request<any>("GET", "/api/v1/configuration")).configuration.file_manager_system_picker_fallback).toBe(false);

    await page.getByRole("button", { name: /Open show menu/ }).click();
    await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
    const setupNav = page.locator(".setup-window nav");
    await setupNav.getByRole("button", { name: "Screens & playback", exact: true }).click();
    await page.getByRole("button", { name: "Desk Lock", exact: true }).click();
    await page.getByRole("button", { name: "Choose lock wallpaper" }).click();
    let dialog = page.getByRole("dialog", { name: "Choose files or folders" });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("button", { name: "Open system file picker" })).toHaveCount(0);
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();

    const configuration = await api.request<any>("GET", "/api/v1/configuration");
    await api.request("PUT", "/api/v1/configuration", {
      ...configuration.configuration,
      file_manager_system_picker_fallback: true,
    });
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/configuration")).configuration.file_manager_system_picker_fallback).toBe(true);

    await page.getByRole("button", { name: "Choose lock wallpaper" }).click();
    dialog = page.getByRole("dialog", { name: "Choose files or folders" });
    await expect(dialog.getByRole("button", { name: "Open system file picker" })).toBeVisible();
    const systemInput = dialog.locator('input[type="file"]');
    await expect(systemInput).toHaveAttribute("accept", ".png,.jpg,.jpeg,.gif,.webp");
    await expect(systemInput).not.toHaveAttribute("multiple");
    await expect(systemInput).not.toHaveAttribute("webkitdirectory");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  });
});

function propertiesFor(manager: Locator) {
  return manager.getByRole("complementary", { name: "Selection properties" });
}

function minimalWave() {
  return Buffer.from([
    0x52, 0x49, 0x46, 0x46, 0x25, 0x00, 0x00, 0x00,
    0x57, 0x41, 0x56, 0x45, 0x66, 0x6d, 0x74, 0x20,
    0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00,
    0x40, 0x1f, 0x00, 0x00, 0x40, 0x1f, 0x00, 0x00,
    0x01, 0x00, 0x08, 0x00, 0x64, 0x61, 0x74, 0x61,
    0x01, 0x00, 0x00, 0x00, 0x80,
  ]);
}

async function addFileManagerPane(page: Page) {
  await page.getByRole("button", { name: "DESKTOPS" }).click();
  await page.getByRole("button", { name: /New desktop/ }).click();
  const grid = page.locator(".desk-grid");
  const box = await grid.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.click(box!.x + Math.min(180, box!.width / 4), box!.y + Math.min(120, box!.height / 4));
  await expect(page.getByRole("heading", { name: "Open Window" })).toBeVisible();
  await page.getByRole("button", { name: "File Manager", exact: true }).click();
  const pane = page.locator(".desk-pane").filter({ hasText: "File Manager" });
  await expect(pane).toBeVisible();
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

async function beginFileEdit(page: Page, manager: Locator, action: "Rename" | "Copy" | "Move" | "Delete") {
  await manager
    .locator(".file-manager-header-actions")
    .getByRole("button", { name: "Edit", exact: true })
    .click();
  await page.getByRole("menu", { name: "Edit menu" }).getByRole("menuitem", { name: action, exact: true }).click();
}
