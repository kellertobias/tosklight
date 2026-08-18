import fs from "node:fs/promises";
import path from "node:path";
import { expect, type Locator, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { LightBench } from "../core/lightBench";
import { ControllableHostedFilePickerDriver } from "../window-system/hostedFilePicker";

/** Public semantic workflows for the File Manager and Text Editor panes. */
export class BrowserFiles {
	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly bench: LightBench,
	) {}

	async expectManagerEditsText(): Promise<void> {
		const name = await this.seedText("run-sheet", "md", "House open");
		try {
			const manager = this.manager();
			await this.refreshManager(manager);
			await expect(manager.locator(".file-columns")).toBeVisible();
			await expect(
				manager.getByRole("heading", { name: "Locations" }),
			).toBeVisible();
			await expect(
				manager.getByRole("heading", { name: "Properties" }),
			).toBeVisible();
			await manager.getByRole("button", { name: new RegExp(name) }).dblclick();
			const editor = manager.locator(".file-editor");
			await editor.getByLabel("File text").fill("House open\nBeginners");
			await this.desk.click(
				editor.getByRole("button", { name: "Save", exact: true }),
			);
			await this.expectText(name, "House open\nBeginners");
		} finally {
			await this.delete(name);
		}
	}

	async expectEditorDirtySave(): Promise<void> {
		const name = await this.seedText("cue-notes", "txt", "Cue 1");
		try {
			const editor = this.editor().first();
			await this.choose(editor, name);
			await expect(editor.getByLabel("File text")).toHaveValue("Cue 1");
			await editor.getByLabel("File text").fill("Cue 1\nCheck follow spot");
			await expect(editor.locator(".text-save-state")).toHaveText("Unsaved");
			await this.desk.click(
				editor.getByRole("button", { name: "Save", exact: true }),
			);
			await expect(editor.locator(".text-save-state")).toHaveText("Saved");
			await this.expectText(name, "Cue 1\nCheck follow spot");
		} finally {
			await this.delete(name);
		}
	}

	async expectTwoEditorLifecycle(): Promise<void> {
		const name = await this.seedText(
			"text-editor",
			"md",
			"Initial run sheet\n",
		);
		const renamed = `text-editor-renamed-${crypto.randomUUID()}.md`;
		try {
			const editors = this.editor();
			await expect(editors).toHaveCount(2);
			await this.choose(editors.nth(0), name);
			await this.choose(editors.nth(1), name);
			await editors
				.nth(0)
				.getByLabel("File text")
				.fill("House open\nBeginners\n");
			await this.desk.click(
				editors.nth(0).getByRole("button", { name: "Save", exact: true }),
			);
			await expect(editors.nth(1).getByLabel("File text")).toHaveValue(
				"House open\nBeginners\n",
			);

			await editors.nth(0).getByLabel("File text").fill("External version\n");
			await editors.nth(1).getByLabel("File text").fill("Operator draft\n");
			await this.desk.click(
				editors.nth(0).getByRole("button", { name: "Save", exact: true }),
			);
			await expect(editors.nth(1).locator(".text-save-state")).toHaveText(
				"Conflict",
			);
			await this.desk.click(editors.nth(1).getByText("Compare versions"));
			await expect(
				editors.nth(1).getByLabel("Your unsaved version"),
			).toHaveValue("Operator draft\n");
			await expect(editors.nth(1).getByLabel("Newer file version")).toHaveValue(
				"External version\n",
			);
			this.acceptNextDialog();
			await this.desk.click(
				editors.nth(1).getByRole("button", { name: "Reload Newer Version" }),
			);

			await editors
				.nth(1)
				.getByLabel("File text")
				.fill("Draft retained across rename\n");
			await this.operation("rename", [name], { name: renamed });
			// A rename is reported in the editor's own status: the association follows the file and
			// the unsaved draft stays put.
			await expect(editors.nth(1)).toContainText(renamed);
			await this.desk.click(
				editors.nth(1).getByRole("button", { name: "Save", exact: true }),
			);
			await this.expectText(renamed, "Draft retained across rename\n");

			await this.delete(renamed);
			await expect(editors.nth(0).locator(".text-save-state")).toHaveText(
				"Missing",
			);
			this.acceptNextDialog();
			await this.desk.click(
				editors.nth(0).getByRole("button", { name: "Recreate File" }),
			);
			await this.expectText(renamed, "Draft retained across rename\n");
		} finally {
			await this.delete(renamed).catch(() => undefined);
			await this.delete(name).catch(() => undefined);
		}
	}

	async expectEditorModes(): Promise<void> {
		const name = await this.seedText(
			"text-editor-settings",
			"md",
			"# Cue Sheet\n\n- House open\n- Beginners\n",
		);
		try {
			const editor = this.editor().first();
			await this.desk.click(
				editor.getByRole("button", { name: "Open File", exact: true }),
			);
			const picker = this.page.getByRole("dialog", {
				name: "Choose files or folders",
			});
			await expect(
				picker.getByRole("button", { name: "Open system file picker" }),
			).toHaveCount(0);
			await this.desk.click(
				picker.getByRole("button", { name: `${name}, file` }),
			);
			await this.desk.click(
				picker.getByRole("button", { name: "Select", exact: true }),
			);
			await this.configureEditor("Rendered Markdown", true);
			const rendered = editor.getByRole("article", {
				name: "Rendered Markdown",
			});
			await expect(
				rendered.getByRole("heading", { name: "Cue Sheet" }),
			).toBeVisible();
			await expect(
				editor.getByRole("button", { name: "Save", exact: true }),
			).toBeDisabled();

			await this.configureEditor("Edit + Markdown", false);
			await editor
				.getByLabel("File text")
				.fill("# Updated Cue Sheet\n\nStand by.\n");
			await expect(
				rendered.getByRole("heading", { name: "Updated Cue Sheet" }),
			).toBeVisible();
			await this.desk.click(
				editor.getByRole("button", { name: "Save", exact: true }),
			);
			await this.expectText(name, "# Updated Cue Sheet\n\nStand by.\n");
			await this.configureEditor("Plain Text", false);
			await expect(editor.getByLabel("File text")).toBeVisible();
			await expect(rendered).toHaveCount(0);
		} finally {
			await this.delete(name);
		}
	}

	async expectManagerOperations(): Promise<void> {
		const workspace = `file-manager-${crypto.randomUUID()}`;
		try {
			await this.operation("create_folder", [], {
				destination: "",
				name: workspace,
			});
			await this.operation("create_folder", [], {
				destination: workspace,
				name: "Destination",
			});
			await this.operation("create_file", [], {
				destination: workspace,
				name: "alpha.txt",
			});
			await this.operation("create_file", [], {
				destination: workspace,
				name: ".operator-note",
			});
			const manager = this.manager();
			await this.refreshManager(manager);
			await manager
				.getByRole("button", { name: `${workspace}, folder` })
				.dblclick();
			await expect(
				manager.getByRole("button", { name: "alpha.txt, file" }),
			).toBeVisible();
			await expect(
				manager.getByRole("button", { name: ".operator-note, file" }),
			).toHaveCount(0);
			await this.configureManagerHiddenFiles(manager, true);
			await expect(
				manager.getByRole("button", { name: ".operator-note, file" }),
			).toBeVisible();

			await this.desk.click(
				manager.getByRole("button", { name: "alpha.txt, file" }),
			);
			await this.beginEdit(manager, "Copy", "alpha.txt, file");
			await expect(
				manager.getByRole("button", { name: "Copy Here" }),
			).toBeVisible();
			await manager
				.getByRole("button", { name: "Destination, folder" })
				.dblclick();
			await this.desk.click(manager.getByRole("button", { name: "Copy Here" }));
			await manager
				.getByRole("navigation", { name: "Breadcrumb" })
				.getByRole("button", { name: `/ ${workspace}` })
				.click();
			await this.desk.click(
				manager.getByRole("button", { name: "alpha.txt, file" }),
			);
			await this.beginEdit(manager, "Copy", "alpha.txt, file");
			await expect(
				manager.getByRole("button", { name: "Copy Here" }),
			).toBeVisible();
			await manager
				.getByRole("button", { name: "Destination, folder" })
				.dblclick();
			await this.desk.click(manager.getByRole("button", { name: "Copy Here" }));
			await this.desk.click(
				manager
					.getByRole("dialog", { name: "Resolve name conflict" })
					.getByRole("button", { name: "Keep Both" }),
			);
			await expect(
				manager.getByRole("button", { name: "alpha copy.txt, file" }),
			).toBeVisible();
		} finally {
			await this.delete(workspace).catch(() => undefined);
		}
	}

	async expectSetupLaunchers(): Promise<void> {
		await this.page.getByRole("button", { name: /Open show menu/ }).click();
		await this.desk.click(
			this.page.getByRole("button", { name: "Enter Setup", exact: true }),
		);
		const nav = this.page.locator(".setup-window nav");
		await expect(
			nav.getByRole("button", { name: "File Manager", exact: true }),
		).toHaveCount(0);
		await expect(
			nav.getByRole("button", { name: "Fixture library", exact: true }),
		).toHaveCount(0);
		await this.desk.click(
			this.page.getByRole("button", {
				name: "Open File Manager",
				exact: true,
			}),
		);
		await expect(this.page.locator(".file-manager")).toBeVisible();
		await this.desk.click(
			this.page.getByRole("button", { name: "Close File Manager" }),
		);
		await expect(
			this.page.getByRole("heading", { name: "Shows & recovery" }),
		).toBeVisible();
		await this.desk.click(
			this.page.getByRole("button", {
				name: "Open Fixture Library",
				exact: true,
			}),
		);
		const library = this.page.getByRole("dialog", { name: "Fixture Library" });
		await expect(library).toBeVisible();
		for (const name of ["Import fixture", "Import GDTF", "Create fixture"])
			await expect(
				library.getByRole("button", { name, exact: true }),
			).toBeVisible();
		await this.desk.click(
			library.getByRole("button", { name: "Close Fixture Library" }),
		);
	}

	async expectHostedPicker(): Promise<void> {
		const workspace = `picker-${crypto.randomUUID()}`;
		const directory = path.join(this.bench.dataDir, "shows", workspace);
		await fs.mkdir(path.join(directory, "Folder"), { recursive: true });
		await fs.writeFile(path.join(directory, "allowed.txt"), "allowed");
		await fs.writeFile(path.join(directory, "blocked.png"), "blocked");
		const picker = new ControllableHostedFilePickerDriver(this.page);
		await picker.install();
		try {
			await this.page.reload();
			await expect(this.page.locator(".connection-cover")).toBeHidden({
				timeout: 15_000,
			});
			const outcome = picker.open({
				target: "files",
				multiple: false,
				allowedExtensions: ["txt"],
				initialRootId: "shows",
				initialDirectory: workspace,
			});
			const dialog = this.page.getByRole("dialog", {
				name: "Choose files or folders",
			});
			await this.desk.click(
				dialog.getByRole("button", { name: "blocked.png, file" }),
			);
			await expect(
				dialog.getByRole("button", { name: "Select", exact: true }),
			).toBeDisabled();
			await this.desk.click(
				dialog.getByRole("button", { name: "allowed.txt, file" }),
			);
			await this.page.keyboard.press("Enter");
			expect(await outcome).toEqual({
				status: "selected",
				selections: [
					expect.objectContaining({ path: `${workspace}/allowed.txt` }),
				],
			});
		} finally {
			await picker.dispose();
			await this.delete(workspace).catch(() => undefined);
		}
	}

	async expectSystemPickerFallback(): Promise<void> {
		await this.page.getByRole("button", { name: /Open show menu/ }).click();
		await this.desk.click(
			this.page.getByRole("button", { name: "Enter Setup", exact: true }),
		);
		await this.desk.click(
			this.page.locator(".setup-window nav").getByRole("button", {
				name: "Screens & playback",
				exact: true,
			}),
		);
		await this.desk.click(
			this.page.getByRole("button", {
				name: "Configure desk lock",
				exact: true,
			}),
		);
		await this.desk.click(
			this.page.getByRole("button", { name: "Choose lock wallpaper" }),
		);
		let dialog = this.page.getByRole("dialog", {
			name: "Choose files or folders",
		});
		await expect(
			dialog.getByRole("button", { name: "Open system file picker" }),
		).toHaveCount(0);
		await this.desk.click(
			dialog.getByRole("button", { name: "Cancel", exact: true }),
		);
		const configuration = await this.api.request<any>(
			"GET",
			"/api/v2/configuration",
		);
		await this.api.request("PUT", "/api/v2/configuration", {
			...configuration.configuration,
			file_manager_system_picker_fallback: true,
		});
		await this.desk.click(
			this.page.getByRole("button", { name: "Choose lock wallpaper" }),
		);
		dialog = this.page.getByRole("dialog", {
			name: "Choose files or folders",
		});
		await expect(
			dialog.getByRole("button", { name: "Open system file picker" }),
		).toBeVisible();
		const input = dialog.locator('input[type="file"]');
		await expect(input).toHaveAttribute("accept", ".png,.jpg,.jpeg,.gif,.webp");
		await expect(input).not.toHaveAttribute("multiple");
		await expect(input).not.toHaveAttribute("webkitdirectory");
	}

	private manager(): Locator {
		return this.page.locator('[data-pane-type="file_manager"]');
	}

	private editor(): Locator {
		return this.page.locator('[data-pane-type="text_editor"]');
	}

	private async choose(editor: Locator, name: string): Promise<void> {
		// The editor toolbar offers Refresh only while it already holds a file association.
		const refresh = editor.getByRole("button", { name: "Refresh", exact: true });
		if (await refresh.count()) await this.desk.click(refresh);
		const quickChoice = editor.getByRole("button", {
			name: "Choose File…",
			exact: true,
		});
		if (await quickChoice.count()) await this.desk.click(quickChoice);
		else
			await this.desk.click(
				editor.getByRole("button", { name: "Open File", exact: true }),
			);
		const option = this.page.getByRole("option", { name, exact: true });
		if (await option.count()) await this.desk.click(option);
		else {
			const picker = this.page.getByRole("dialog", {
				name: "Choose files or folders",
			});
			await this.desk.click(
				picker.getByRole("button", { name: `${name}, file` }),
			);
			await this.desk.click(
				picker.getByRole("button", { name: "Select", exact: true }),
			);
		}
	}

	private async configureEditor(mode: string, readOnly: boolean) {
		const editor = this.editor().first();
		await this.desk.click(
			editor.getByRole("button", { name: "Settings", exact: true }),
		);
		const settings = this.page.getByRole("dialog", { name: "Pane Settings" });
		await this.desk.click(
			settings.getByRole("tab", { name: "Text Editor", exact: true }),
		);
		const toggle = settings.getByRole("switch", {
			name: "Editing",
			exact: true,
		});
		if ((await toggle.isChecked()) !== readOnly)
			await this.desk.click(toggle.locator("xpath=.."));
		await this.desk.click(settings.getByRole("radio", { name: mode }));
		await this.desk.click(
			settings.getByRole("button", { name: "Close settings" }),
		);
	}

	private async beginEdit(manager: Locator, action: string, entry?: string) {
		const open = async () =>
			manager
				.locator(".file-manager-header-actions")
				.getByRole("button", { name: "Edit", exact: true })
				.click();
		const menu = this.page.getByRole("menu", { name: "Edit menu" });
		const item = menu.getByRole("menuitem", { name: action, exact: true });
		await open();
		// A row reads as pressed while it is a pending operation's source as well as while it is
		// selected, so a click meant to select can toggle the selection away instead. The menu
		// itself is the honest signal: reach the row again when the action stays unavailable.
		if (entry && (await item.isDisabled())) {
			await this.page.locator(".file-header-menu-layer").click();
			await expect(menu).toBeHidden();
			await this.desk.click(manager.getByRole("button", { name: entry }));
			await open();
		}
		await item.click();
		await expect(menu).toBeHidden();
	}

	private async seedText(prefix: string, extension: string, text: string) {
		const name = `${prefix}-${crypto.randomUUID()}.${extension}`;
		await this.operation("create_file", [], { destination: "", name });
		const empty = await this.readText(name);
		await this.api.request("PUT", "/api/v2/files/shows/text", {
			path: name,
			text,
			revision: empty.revision,
		});
		return name;
	}

	private async refreshManager(manager: Locator): Promise<void> {
		await this.configureManagerHiddenFiles(manager, true);
		await this.configureManagerHiddenFiles(manager, false);
	}

	private async configureManagerHiddenFiles(
		manager: Locator,
		visible: boolean,
	): Promise<void> {
		await this.desk.click(
			manager.getByRole("button", { name: "Settings", exact: true }),
		);
		const settings = this.page.getByRole("dialog", {
			name: "Pane Settings",
			exact: true,
		});
		await this.desk.click(
			settings.getByRole("tab", { name: "File Manager", exact: true }),
		);
		const toggle = settings.getByRole("switch", {
			name: "Hidden files",
			exact: true,
		});
		if ((await toggle.isChecked()) !== visible)
			await this.desk.click(toggle.locator("..").locator(".ui-switch-track"));
		await this.desk.click(
			settings.getByRole("button", { name: "Close settings", exact: true }),
		);
	}

	private readText(path: string): Promise<{ text: string; revision: number }> {
		return this.api.request(
			"GET",
			`/api/v2/files/shows/text?path=${encodeURIComponent(path)}`,
		);
	}

	private async expectText(path: string, text: string) {
		await expect
			.poll(async () => {
				try {
					return (await this.readText(path)).text;
				} catch (error) {
					if (
						error instanceof Error &&
						error.message.includes("returned 404:") &&
						error.message.includes('"error":"file not found"')
					)
						return null;
					throw error;
				}
			})
			.toBe(text);
	}

	private operation(
		operation: string,
		sources: string[],
		extra: Record<string, unknown> = {},
	) {
		return this.api.request("POST", "/api/v2/files/shows/operations", {
			operation,
			sources,
			...extra,
		});
	}

	private delete(path: string) {
		return this.operation("delete", [path]);
	}

	private acceptNextDialog() {
		this.page.once("dialog", (dialog) => dialog.accept());
	}
}
