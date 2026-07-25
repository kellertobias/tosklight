import { expect, type Locator, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { LightBench } from "../core/lightBench";
import {
	type ActiveShow,
	type ResolvedShowTarget,
	type ShowHandle,
	showHandle,
} from "./showIdentity";

export interface ShowRevisionExpectation {
	readonly number?: number;
	readonly name?: string;
}

interface ShowLibraryEntry extends ActiveShow {
	path: string;
	revisions?: Array<{ revision: number; name: string }>;
}

interface BootstrapSnapshot {
	active_show: ActiveShow | null;
	active_show_error: string | null;
}

export class ShowOperatorAdapter {
	constructor(
		private readonly route: "ui" | "api",
		private readonly api: ApiDriver,
		private readonly bench: LightBench,
		private readonly desk: DeskDriver,
		private readonly page?: Page,
	) {}

	async create(name: string): Promise<ShowHandle> {
		const normalized = requiredName(name, "Show");
		await this.step(
			"SHOW CREATE",
			`Create and activate "${normalized}" via ${this.route}.`,
		);
		if (this.route === "api") {
			const created = await this.api.createShow<ActiveShow>({
				name: normalized,
			});
			await this.api.openShow(created.id, { transition: "hold_current" });
			return showHandle({ id: created.id, name: created.name });
		}
		const showMenu = await this.openShowMenu();
		await showMenu
			.getByRole("button", { name: "New Show", exact: true })
			.click();
		const dialog = this.browser().getByRole("dialog", {
			name: "New show",
			exact: true,
		});
		await dialog
			.getByRole("button", { name: "Create Empty Show", exact: true })
			.click();
		await expect
			.poll(async () => (await this.active()).name)
			.toMatch(/^New Empty Show(?: [1-9]\d*)?$/);
		return this.saveAs(normalized);
	}

	async load(target: ResolvedShowTarget): Promise<ShowHandle> {
		const resolved = await this.libraryTarget(target);
		await this.step("SHOW LOAD", `Load "${resolved.name}" via ${this.route}.`);
		if (this.route === "api") {
			await this.api.openShow(resolved.id, { transition: "safe_blackout" });
		} else {
			const dialog = await this.openLoadDialog();
			await this.libraryCard(dialog, resolved.name)
				.getByRole("button", { name: "Load Latest Autosave", exact: true })
				.click();
			await expect(dialog).toBeHidden();
		}
		await this.waitForActiveId(resolved.id);
		return showHandle(resolved);
	}

	async save(): Promise<ShowHandle> {
		if (this.route === "api") {
			throw new Error(
				"show.save has no independent API route: ToskLight commits ordinary show changes automatically",
			);
		}
		const active = await this.active();
		await this.step(
			"SHOW SAVE",
			active.revision_copy
				? `Confirm that revision copy "${active.name}" remains separately autosaved.`
				: `Confirm autosave convergence for "${active.name}"; ordinary shows have no manual Save mutation.`,
		);
		const menu = await this.openShowMenu();
		if (active.revision_copy) {
			await menu.getByRole("button", { name: "Save", exact: true }).click();
			await this.browser()
				.getByRole("dialog", { name: "Save revision copy", exact: true })
				.getByRole("button", { name: "Keep as Separate Show", exact: true })
				.click();
		} else {
			await expect(menu).toContainText(
				"Changes are saved automatically as they are made.",
			);
		}
		await this.assertAutosaved();
		return showHandle({ id: active.id, name: active.name });
	}

	async saveAs(name: string): Promise<ShowHandle> {
		const normalized = requiredName(name, "Show");
		const source = await this.active();
		await this.step(
			"SHOW SAVE AS",
			`Save the active show as "${normalized}" via ${this.route}.`,
		);
		if (this.route === "api") {
			const bytes = await this.api.downloadShow(source.id);
			const created = await this.api.createShow<ActiveShow>({
				name: normalized,
				data_base64: bytes.toString("base64"),
				overwrite: false,
			});
			await this.api.openShow(created.id, { transition: "hold_current" });
			return showHandle({ id: created.id, name: created.name });
		}
		const menu = await this.openShowMenu();
		await menu.getByRole("button", { name: "Save As", exact: true }).click();
		const dialog = this.browser().getByRole("dialog", {
			name: "Save show",
			exact: true,
		});
		await dialog.getByRole("textbox", { name: "Show name" }).fill(normalized);
		await dialog
			.getByRole("button", {
				name: /^(?:Name Empty Show|Save as New Show)$/,
			})
			.click();
		await expect(dialog).toBeHidden();
		await expect.poll(async () => (await this.active()).name).toBe(normalized);
		const active = await this.active();
		return showHandle({ id: active.id, name: active.name });
	}

	async saveRevision(name: string): Promise<number> {
		const normalized = requiredName(name, "Revision");
		const active = await this.active();
		await this.step(
			"SHOW REVISION",
			`Save named revision "${normalized}" via ${this.route}.`,
		);
		if (this.route === "api") {
			const saved = await this.api.saveShowRevision<{ revision: number }>(
				active.id,
				normalized,
			);
			return saved.revision;
		}
		const menu = await this.openShowMenu();
		await menu
			.getByRole("button", { name: "Save Named Revision", exact: true })
			.click();
		const dialog = this.browser().getByRole("dialog", {
			name: "Save named revision",
			exact: true,
		});
		await dialog
			.getByRole("textbox", { name: "Revision name" })
			.fill(normalized);
		await dialog.getByRole("button", { name: /^Save Revision \d+$/ }).click();
		await expect(dialog).toBeHidden();
		const revision = await this.findRevision(active.id, { name: normalized });
		return revision.revision;
	}

	async loadRevision(
		target: ResolvedShowTarget,
		revision: number,
	): Promise<ShowHandle> {
		const resolved = await this.libraryTarget(target);
		await this.step(
			"SHOW LOAD REVISION",
			`Load revision ${revision} of "${resolved.name}" as a separate copy via ${this.route}.`,
		);
		let copy: ActiveShow;
		if (this.route === "api") {
			copy = await this.api.openShowRevision<ActiveShow>(
				resolved.id,
				revision,
				{
					transition: "safe_blackout",
				},
			);
		} else {
			const dialog = await this.openLoadDialog();
			const revisionButton = this.libraryCard(dialog, resolved.name)
				.locator(".named-revision-list button")
				.filter({ hasText: `Revision ${revision} ·` });
			await revisionButton.click();
			await expect(dialog).toBeHidden();
			await expect
				.poll(async () => (await this.active()).revision_copy?.revision)
				.toBe(revision);
			copy = await this.active();
		}
		expect(copy.id).not.toBe(resolved.id);
		expect(copy.revision_copy).toMatchObject({
			show_id: resolved.id,
			show_name: resolved.name,
			revision,
		});
		return showHandle({ id: copy.id, name: copy.name });
	}

	async loadCleanDefault(): Promise<ShowHandle> {
		const before = (await this.active()).id;
		await this.step(
			"SHOW LOAD DEFAULT",
			`Load a clean built-in default through the ${this.route} operator route.`,
		);
		if (this.route === "api") {
			await this.api.openDefaultShow({ transition: "safe_blackout" });
		} else {
			const recovery = this.browser().getByRole("alertdialog", {
				name: "Show recovery required",
			});
			const surface = (await recovery.isVisible())
				? recovery
				: await this.openLoadDialog();
			await surface
				.getByRole("button", {
					name: "Load Clean Built-in Default",
					exact: true,
				})
				.click();
		}
		await expect.poll(async () => (await this.active()).id).not.toBe(before);
		const active = await this.active();
		return showHandle({ id: active.id, name: active.name });
	}

	async expectRevision(expectation: ShowRevisionExpectation): Promise<void> {
		const active = await this.active();
		await this.findRevision(active.id, expectation);
	}

	async expectAutosaved(expected: boolean): Promise<void> {
		if (expected) {
			throw new Error(
				"show.expect.dirty(true) is unsupported: ToskLight commits show changes automatically and has no dirty projection",
			);
		}
		await this.assertAutosaved();
	}

	async active(): Promise<ActiveShow> {
		const active = (await this.bootstrap()).active_show;
		if (!active) throw new Error("No show is active");
		return active;
	}

	private async assertAutosaved(): Promise<void> {
		const active = await this.active();
		const listed = (await this.library()).find((show) => show.id === active.id);
		expect(listed).toBeDefined();
		expect(listed?.revision).toBe(active.revision);
	}

	private async findRevision(
		showId: string,
		expectation: ShowRevisionExpectation,
	): Promise<{ revision: number; name: string }> {
		const revisions = await this.api.showRevisions<{
			revision: number;
			name: string;
		}>(showId);
		const found = revisions.find(
			(revision) =>
				(expectation.number === undefined ||
					revision.revision === expectation.number) &&
				(expectation.name === undefined || revision.name === expectation.name),
		);
		expect(found).toBeDefined();
		if (!found) throw new Error("Expected named show revision was not found");
		return found;
	}

	private async libraryTarget(
		target: ResolvedShowTarget,
	): Promise<Required<ResolvedShowTarget>> {
		const entries = await this.library();
		const matches = entries.filter((show) =>
			target.id ? show.id === target.id : show.name === target.name,
		);
		if (matches.length !== 1) {
			throw new Error(
				`Expected one show named "${target.name}", found ${matches.length}. Available shows: ${entries.map((show) => show.name).join(", ")}`,
			);
		}
		return { id: matches[0].id, name: matches[0].name };
	}

	private async library(): Promise<ShowLibraryEntry[]> {
		return this.api.shows<ShowLibraryEntry>();
	}

	private async bootstrap(): Promise<BootstrapSnapshot> {
		return this.api.request("GET", "/api/v2/bootstrap", undefined, false);
	}

	private async waitForActiveId(id: string): Promise<void> {
		await expect.poll(async () => (await this.active()).id).toBe(id);
	}

	private async openLoadDialog(): Promise<Locator> {
		const menu = await this.openShowMenu();
		await menu.getByRole("button", { name: "Load", exact: true }).click();
		const dialog = this.browser().getByRole("dialog", {
			name: "Load show",
			exact: true,
		});
		await expect(dialog).toBeVisible();
		return dialog;
	}

	private async openShowMenu(): Promise<Locator> {
		const page = this.browser();
		await this.ensureBrowserOpen();
		const menu = page.getByRole("dialog", { name: "Show", exact: true });
		if (!(await menu.isVisible())) {
			await page.getByRole("button", { name: /Open show menu/ }).click();
		}
		await expect(menu).toBeVisible();
		return menu;
	}

	private libraryCard(dialog: Locator, name: string): Locator {
		return dialog
			.locator(".revision-show-library article")
			.filter({ has: this.browser().getByText(name, { exact: true }) });
	}

	private async ensureBrowserOpen(): Promise<void> {
		const page = this.browser();
		if (
			(await page.locator('[data-light-surface="application"]').count()) === 0
		) {
			await this.desk.open(this.bench.baseUrl);
		}
	}

	private browser(): Page {
		if (!this.page) {
			throw new Error(`The ${this.route} show adapter has no browser page`);
		}
		return this.page;
	}

	private step(action: string, detail: string): Promise<void> {
		return this.desk.recordStep(action, detail);
	}
}

function requiredName(value: string, label: string): string {
	const normalized = value.trim();
	if (!normalized) throw new Error(`${label} name must not be empty`);
	return normalized;
}
