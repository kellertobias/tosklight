import { expect, type Locator, type Page } from "@playwright/test";
import type {
	OperatorPaneType,
	PaneConfiguration,
	PaneType,
} from "./paneTypes";
import {
	PresetFamily,
	paneLabels,
	type StageRenderQuality,
	StageView,
} from "./paneTypes";

export interface PanePlacement {
	slug: string;
	column: number;
	row: number;
	width: number;
	height: number;
}

export type PaneGeometry = Omit<PanePlacement, "slug">;

interface PaneBinding {
	type: PaneType;
	runtimeId: string;
}

interface PaneDefinition<T extends OperatorPaneType = OperatorPaneType> {
	type: T;
	placement: PanePlacement;
	configuration?: PaneConfiguration<T>;
	handle: PaneHandle<T>;
}

const GRID_COLUMNS = 24;
const GRID_ROWS = 18;
const SLUG = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

export class DesktopConfiguration {
	private readonly panes: PaneDefinition[] = [];

	constructor(
		private readonly desktop: BrowserDesktops,
		readonly name: string,
	) {}

	addPane<T extends OperatorPaneType>(
		type: T,
		placement: PanePlacement,
		configuration?: PaneConfiguration<T>,
	): PaneHandle<T> {
		validatePlacement(
			placement,
			this.panes.map((pane) => pane.placement),
		);
		const handle = this.desktop.createHandle(type, placement.slug, this);
		this.panes.push({
			type,
			placement,
			configuration,
			handle,
		} as PaneDefinition);
		return handle;
	}

	setConfiguration<T extends OperatorPaneType>(
		handle: PaneHandle<T>,
		configuration: PaneConfiguration<T>,
	): void {
		const definition = this.panes.find((pane) => pane.handle === handle);
		if (!definition)
			throw new Error(
				`Pane "${handle.slug}" does not belong to this Desktop configuration`,
			);
		definition.configuration =
			configuration as PaneConfiguration<OperatorPaneType>;
	}

	async apply(): Promise<void> {
		await this.desktop.apply(this.name, this.panes);
	}
}

export class PaneHandle<T extends PaneType> {
	constructor(
		private readonly desktops: BrowserDesktops,
		readonly type: T,
		readonly slug: string,
		private readonly draft?: DesktopConfiguration,
	) {}

	async configure(configuration: PaneConfiguration<T>): Promise<void> {
		if (this.draft && !this.desktops.isBound(this.slug)) {
			this.draft.setConfiguration(
				this as unknown as PaneHandle<Extract<T, OperatorPaneType>>,
				configuration as PaneConfiguration<Extract<T, OperatorPaneType>>,
			);
			return;
		}
		await this.desktops.configurePane(this, configuration);
	}

	focus = () => this.desktops.focus(this);
	maximize = () => this.desktops.maximize(this, true);
	restore = () => this.desktops.maximize(this, false);
	remove = () => this.desktops.remove(this);
	screenshot = (name: string) => this.desktops.screenshot(this, name);
	root = () => this.desktops.locatorFor(this);
	move = (position: Pick<PaneGeometry, "column" | "row">) =>
		this.desktops.setGeometry(this, position);
	resize = (size: Pick<PaneGeometry, "width" | "height">) =>
		this.desktops.setGeometry(this, size);

	readonly expect = {
		geometry: (geometry: PaneGeometry) =>
			this.desktops.expectGeometry(this, geometry),
		visible: () => expect(this.desktops.locatorFor(this)).toBeVisible(),
		maximized: (value = true) =>
			expect(this.desktops.locatorFor(this)).toHaveAttribute(
				"aria-expanded",
				String(value),
			),
	};
}

export class BrowserDesktops {
	private readonly bindings = new Map<string, PaneBinding>();
	private readonly layouts = new Map<
		string,
		(configuration: DesktopConfiguration) => void
	>();

	constructor(
		private readonly page: Page,
		private readonly attach: (name: string, body: Buffer) => Promise<void>,
		private readonly pause: () => Promise<void> = async () => undefined,
	) {}

	configure(name: string): DesktopConfiguration {
		return new DesktopConfiguration(this, name);
	}

	define(
		name: string,
		recipe: (configuration: DesktopConfiguration) => void,
	): void {
		if (this.layouts.has(name))
			throw new Error(`Desktop "${name}" is already registered`);
		this.layouts.set(name, recipe);
	}

	async use(name: string): Promise<void> {
		const recipe = this.layouts.get(name);
		if (!recipe) throw new Error(`Desktop "${name}" has not been registered`);
		const configuration = this.configure(name);
		recipe(configuration);
		await configuration.apply();
	}

	async create(name: string): Promise<void> {
		await this.showDesktops();
		if (await this.desktopButton(name).count())
			throw new Error(`Desktop "${name}" already exists`);
		await this.page
			.getByRole("button", { name: "New desktop", exact: true })
			.click();
		await this.pause();
		const active = this.page.locator("[data-desktop-id][aria-current=page]");
		await expect(active).toBeVisible();
		await this.openSettings(active);
		await this.pause();
		const dialog = this.page.getByRole("dialog", { name: "Desktop settings" });
		await dialog.getByLabel("Name").fill(name);
		await dialog.getByLabel("Name").blur();
		await this.pause();
		await dialog
			.getByRole("button", { name: "Close Desktop settings" })
			.click();
		await this.pause();
		await expect(this.desktopRoot(name)).toBeVisible();
	}

	async open(name: string): Promise<void> {
		await this.showDesktops();
		const button = this.desktopButton(name);
		if (!(await button.count()))
			throw new Error(`Desktop "${name}" does not exist`);
		await button.click();
		await expect(this.desktopRoot(name)).toBeVisible();
	}

	async rename(name: string, nextName: string): Promise<void> {
		await this.open(name);
		await this.openSettings(this.desktopButton(name));
		const dialog = this.page.getByRole("dialog", { name: "Desktop settings" });
		await dialog.getByLabel("Name").fill(nextName);
		await dialog.getByLabel("Name").blur();
		await dialog
			.getByRole("button", { name: "Close Desktop settings" })
			.click();
	}

	async openSettingsFor(name: string): Promise<void> {
		await this.open(name);
		await this.openSettings(this.desktopButton(name));
	}

	async closeSettings(): Promise<void> {
		const dialog = this.page.getByRole("dialog", { name: "Desktop settings" });
		if (await dialog.count())
			await dialog
				.getByRole("button", { name: "Close Desktop settings" })
				.click();
	}

	async clone(name: string, cloneName: string): Promise<void> {
		await this.open(name);
		await this.openSettings(this.desktopButton(name));
		await this.page
			.getByRole("dialog", { name: "Desktop settings" })
			.getByRole("button", { name: "Clone current desktop" })
			.click();
		const active = this.page.locator("[data-desktop-id][aria-current=page]");
		await this.openSettings(active);
		const dialog = this.page.getByRole("dialog", { name: "Desktop settings" });
		await dialog.getByLabel("Name").fill(cloneName);
		await dialog.getByLabel("Name").blur();
		await dialog
			.getByRole("button", { name: "Close Desktop settings" })
			.click();
	}

	async delete(name: string): Promise<void> {
		await this.open(name);
		await this.openSettings(this.desktopButton(name));
		const dialog = this.page.getByRole("dialog", { name: "Desktop settings" });
		await dialog.getByRole("button", { name: "Delete desktop" }).click();
		await dialog.getByRole("button", { name: "Confirm delete" }).click();
		await expect(this.desktopButton(name)).toHaveCount(0);
	}

	getPane<T extends PaneType>(type: T, slug: string): PaneHandle<T> {
		const binding = this.bindings.get(slug);
		if (!binding)
			throw new Error(
				`No pane is bound to slug "${slug}" on the current Desktop`,
			);
		if (binding.type !== type)
			throw new Error(
				`Pane "${slug}" is ${binding.type}, not requested type ${type}`,
			);
		return new PaneHandle(this, type, slug);
	}

	createHandle<T extends PaneType>(
		type: T,
		slug: string,
		draft?: DesktopConfiguration,
	): PaneHandle<T> {
		return new PaneHandle(this, type, slug, draft);
	}

	isBound(slug: string): boolean {
		return this.bindings.has(slug);
	}

	async apply(name: string, panes: readonly PaneDefinition[]): Promise<void> {
		if (await this.desktopButton(name).count()) await this.open(name);
		else await this.create(name);
		for (const definition of panes) await this.addPane(definition);
	}

	async configurePane<T extends PaneType>(
		handle: PaneHandle<T>,
		configuration: PaneConfiguration<T>,
	): Promise<void> {
		const pane = this.locatorFor(handle);
		if (!(await pane.count()))
			throw new Error(`Pane "${handle.slug}" is no longer present`);
		await this.openPaneSettings(pane);
		await this.pause();
		await applyPaneConfiguration(this.page, handle.type, configuration);
		await this.pause();
		await this.closePaneSettings();
	}

	async setGeometry<T extends PaneType>(
		handle: PaneHandle<T>,
		geometry: Partial<PaneGeometry>,
	): Promise<void> {
		const current = await this.readGeometry(this.locatorFor(handle));
		const next = { ...current, ...geometry };
		validatePlacement({ slug: handle.slug, ...next }, []);
		await this.openPaneSettings(this.locatorFor(handle));
		await this.pause();
		const dialog = this.page.getByRole("dialog", { name: "Pane Settings" });
		for (const [label, value] of [
			["Grid column", geometry.column],
			["Grid row", geometry.row],
			["Grid width", geometry.width],
			["Grid height", geometry.height],
		] as const) {
			if (value !== undefined)
				await chooseOption(this.page, dialog, label, String(value));
		}
		await this.closePaneSettings();
		await this.pause();
	}

	async focus<T extends PaneType>(handle: PaneHandle<T>): Promise<void> {
		const pane = this.locatorFor(handle);
		await pane.focus();
		await expect(pane).toBeFocused();
	}

	async maximize<T extends PaneType>(
		handle: PaneHandle<T>,
		maximized: boolean,
	): Promise<void> {
		const pane = this.locatorFor(handle);
		if ((await pane.getAttribute("aria-expanded")) === String(maximized))
			return;
		await this.openPaneSettings(pane);
		await this.page
			.getByRole("dialog", { name: "Pane Settings" })
			.getByRole("button", {
				name: maximized ? "Maximize pane" : "Restore pane",
			})
			.click();
		await this.closePaneSettings();
		await expect(pane).toHaveAttribute("aria-expanded", String(maximized));
	}

	async remove<T extends PaneType>(handle: PaneHandle<T>): Promise<void> {
		const pane = this.locatorFor(handle);
		await this.openPaneSettings(pane);
		await this.page
			.getByRole("dialog", { name: "Pane Settings" })
			.getByRole("button", { name: "Remove pane" })
			.click();
		await expect(pane).toHaveCount(0);
		this.bindings.delete(handle.slug);
	}

	async screenshot<T extends PaneType>(
		handle: PaneHandle<T>,
		name: string,
	): Promise<void> {
		await this.attach(name, await this.locatorFor(handle).screenshot());
	}

	async expectGeometry<T extends PaneType>(
		handle: PaneHandle<T>,
		geometry: PaneGeometry,
	): Promise<void> {
		const pane = this.locatorFor(handle);
		for (const [attribute, value] of [
			["data-grid-column", geometry.column],
			["data-grid-row", geometry.row],
			["data-grid-width", geometry.width],
			["data-grid-height", geometry.height],
		] as const) {
			await expect(pane).toHaveAttribute(attribute, String(value));
		}
	}

	locatorFor<T extends PaneType>(handle: PaneHandle<T>): Locator {
		const binding = this.bindings.get(handle.slug);
		if (!binding)
			throw new Error(
				`Pane handle "${handle.slug}" is unbound; apply its Desktop configuration first`,
			);
		if (binding.type !== handle.type)
			throw new Error(
				`Pane "${handle.slug}" is ${binding.type}, not requested type ${handle.type}`,
			);
		return this.page.locator(`[data-pane-id="${binding.runtimeId}"]`);
	}

	private async addPane(definition: PaneDefinition): Promise<void> {
		const before = await this.page
			.locator("[data-pane-id]")
			.evaluateAll((elements) =>
				elements.map((element) => element.getAttribute("data-pane-id")),
			);
		const grid = this.page.locator(".desk-grid");
		const box = await grid.boundingBox();
		if (!box) throw new Error("The active Desktop grid is not visible");
		const x =
			box.x + ((definition.placement.column - 0.5) * box.width) / GRID_COLUMNS;
		const y =
			box.y + ((definition.placement.row - 0.5) * box.height) / GRID_ROWS;
		await grid.click({ position: { x: x - box.x, y: y - box.y } });
		await this.pause();
		await this.page
			.getByRole("dialog", { name: "Open Window" })
			.getByRole("button", { name: paneLabels[definition.type], exact: true })
			.click();
		await this.pause();
		await expect(this.page.locator("[data-pane-id]")).toHaveCount(
			before.length + 1,
		);
		const runtimeId = await this.page
			.locator("[data-pane-id]")
			.evaluateAll(
				(elements, previous) =>
					elements
						.map((element) => element.getAttribute("data-pane-id"))
						.find((id) => id && !previous.includes(id)) ?? null,
				before,
			);
		const pane = this.page.locator(`[data-pane-id="${runtimeId}"]`);
		await expect(pane).toBeVisible();
		if (!runtimeId)
			throw new Error(
				`Created ${definition.type} pane has no runtime identity`,
			);
		this.bindings.set(definition.placement.slug, {
			type: definition.type,
			runtimeId,
		});
		await this.setGeometry(definition.handle, definition.placement);
		if (definition.configuration)
			await this.configurePane(definition.handle, definition.configuration);
	}

	private async readGeometry(pane: Locator): Promise<PaneGeometry> {
		const values = await Promise.all(
			[
				"data-grid-column",
				"data-grid-row",
				"data-grid-width",
				"data-grid-height",
			].map((name) => pane.getAttribute(name)),
		);
		return {
			column: Number(values[0]),
			row: Number(values[1]),
			width: Number(values[2]),
			height: Number(values[3]),
		};
	}

	private async openPaneSettings(pane: Locator): Promise<void> {
		await pane.getByRole("button", { name: "Settings", exact: true }).click();
		await expect(
			this.page.getByRole("dialog", { name: "Pane Settings" }),
		).toBeVisible();
	}

	private async closePaneSettings(): Promise<void> {
		const dialog = this.page.getByRole("dialog", { name: "Pane Settings" });
		if (await dialog.count())
			await dialog.getByRole("button", { name: "Close settings" }).click();
	}

	private async showDesktops(): Promise<void> {
		const toggle = this.page.getByRole("button", {
			name: "Desktops / Built-ins",
			exact: true,
		});
		if ((await toggle.getAttribute("data-dock-mode")) !== "desks")
			await toggle.click();
	}

	private desktopButton(name: string): Locator {
		return this.page
			.locator("[aria-label=Desktops]")
			.getByRole("button", { name, exact: true });
	}

	private desktopRoot(name: string): Locator {
		return this.page.locator(
			`[data-light-surface="desktop"][aria-label="Desktop ${name}"]`,
		);
	}

	private async openSettings(button: Locator): Promise<void> {
		await button.hover();
		await this.page.mouse.down();
		await this.page.waitForTimeout(700);
		await this.page.mouse.up();
		await expect(
			this.page.getByRole("dialog", { name: "Desktop settings" }),
		).toBeVisible();
	}
}

function validatePlacement(
	candidate: PanePlacement,
	existing: readonly PanePlacement[],
): void {
	if (!SLUG.test(candidate.slug))
		throw new Error(`Pane slug "${candidate.slug}" must be unique kebab-case`);
	if (existing.some((pane) => pane.slug === candidate.slug))
		throw new Error(`Duplicate pane slug "${candidate.slug}"`);
	if (
		[candidate.column, candidate.row, candidate.width, candidate.height].some(
			(value) => !Number.isInteger(value) || value < 1,
		) ||
		candidate.column + candidate.width - 1 > GRID_COLUMNS ||
		candidate.row + candidate.height - 1 > GRID_ROWS
	) {
		throw new Error(
			`Pane "${candidate.slug}" must fit inside the 24 × 18 Desktop grid`,
		);
	}
	if (
		existing.some(
			(pane) =>
				candidate.column < pane.column + pane.width &&
				candidate.column + candidate.width > pane.column &&
				candidate.row < pane.row + pane.height &&
				candidate.row + candidate.height > pane.row,
		)
	) {
		throw new Error(
			`Pane "${candidate.slug}" collides with another configured pane`,
		);
	}
}

async function applyPaneConfiguration<T extends PaneType>(
	page: Page,
	type: T,
	configuration: PaneConfiguration<T>,
): Promise<void> {
	const dialog = page.getByRole("dialog", { name: "Pane Settings" });
	const options = configuration as Record<string, unknown>;
	if (type === "stage") {
		await dialog.getByRole("tab", { name: "Stage" }).click();
		if (options.view) {
			const view = dialog.getByRole("radio", {
				name: options.view === StageView.ThreeDimensional ? "3D" : "2D",
			});
			await view.click();
			await expect(view).toBeChecked();
		}
		await setSwitch(dialog, "Preload source", options.followPreload);
		await setSwitch(dialog, "Beam direction guidelines", options.beamGuides);
		if (
			options.renderQuality &&
			options.view !== StageView.TwoDimensional
		) {
			const quality = dialog.getByRole("radio", {
				name: String(options.renderQuality as StageRenderQuality),
				exact: true,
			});
			await expect(quality).toBeVisible();
			await quality.click();
		}
	}
	if (type === "layout" && options.groupId !== undefined) {
		await dialog.getByRole("tab", { name: "Layout" }).click();
		await dialog.getByLabel("Group").selectOption(String(options.groupId));
	}
	if (type === "presets") {
		await dialog.getByRole("tab", { name: "Pool" }).click();
		if (options.family)
			await dialog
				.getByRole("button", {
					name: String(
						options.family === PresetFamily.Mixed ? "Mixed" : options.family,
					),
					exact: true,
				})
				.click();
		if (options.poolColors !== undefined)
			await dialog
				.getByRole("button", {
					name: options.poolColors ? "Type colors" : "Individual colors",
					exact: true,
				})
				.click();
	}
	if (type === "groups" && options.columns !== undefined) {
		await dialog.getByRole("tab", { name: "Pool" }).click();
		await dialog.getByLabel("Columns").fill(String(options.columns));
		await dialog.getByLabel("Columns").blur();
	}
	if (type === "virtual_playbacks") {
		await dialog.getByRole("tab", { name: "Virtual Playbacks" }).click();
		if (options.rows !== undefined)
			await dialog.getByLabel("Rows").fill(String(options.rows));
		if (options.columns !== undefined)
			await dialog.getByLabel("Columns").fill(String(options.columns));
		if (options.pageMode !== undefined)
			await dialog
				.getByRole("radio", {
					name: options.pageMode === "follow_main" ? "Follow Main" : "Pinned",
				})
				.click();
		if (options.pageMode === "pinned" && options.pinnedPage !== undefined)
			await dialog.getByLabel("Pinned page").fill(String(options.pinnedPage));
	}
	if (type === "fixtures" && options.activeOnly !== undefined) {
		await dialog.getByRole("tab", { name: "Fixture Sheet" }).click();
		await setSwitch(dialog, "Show active fixtures only", options.activeOnly);
	}
	if (type === "cues") {
		await dialog.getByRole("tab", { name: "Cues" }).click();
		if (options.cueListSource !== undefined)
			await dialog
				.getByRole("radio", {
					name:
						options.cueListSource === "follow-selection"
							? "Follow selection"
							: "Fixed",
				})
				.click();
		if (
			options.cueListSource !== "follow-selection" &&
			options.fixedCueListNumber !== undefined
		)
			await dialog
				.getByLabel("Cuelist")
				.selectOption(String(options.fixedCueListNumber));
		await setSwitch(dialog, "Cue sidebar", options.showCueSidebar);
	}
	if (options.showGroupShortcuts !== undefined) {
		await dialog.getByRole("tab", { name: "Shortcuts" }).click();
		await setSwitch(dialog, "Group shortcuts", options.showGroupShortcuts);
	}
}

async function setSwitch(
	dialog: Locator,
	name: string,
	value: unknown,
): Promise<void> {
	if (typeof value !== "boolean") return;
	const control = dialog.getByRole("switch", { name });
	if ((await control.isChecked()) !== value)
		await control.locator("..").locator(".ui-switch-track").click();
}

async function chooseOption(
	page: Page,
	root: Locator,
	label: string,
	option: string,
): Promise<void> {
	await root
		.getByText(label, { exact: true })
		.locator("..")
		.getByRole("button")
		.click();
	await page
		.getByRole("listbox", { name: label })
		.getByRole("option", { name: option, exact: true })
		.click();
}
