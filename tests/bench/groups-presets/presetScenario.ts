import { expect, type Page } from "@playwright/test";
import { HttpPresetRecordingTransport } from "../../../apps/light-desktop/src/api/PresetRecordingTransport";
import type { StoredPreset } from "../../../apps/light-desktop/src/api/types";
import {
	presetAddress,
	presetStorageKey,
} from "../../../apps/light-desktop/src/presetFamilies";
import type { ApiDriver } from "../core/api";
import type { BrowserCommands } from "../command-selection/commandScenario";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import { recallPreset } from "./presetRecall";
import {
	presetAddressKeys,
	presetCommandAddress,
	productPresetFamily,
	stablePresetRouteIndex,
	validPresetNumber,
} from "./presetScenarioSupport";

export enum PresetFamily {
	Mixed = "Mixed",
	Intensity = "Intensity",
	Color = "Color",
	Position = "Position",
	Beam = "Beam",
}

export interface PresetProperties {
	title?: string;
	color?: string;
	icon?: string;
}

type PresetRoute = "pool" | "keypad" | "api" | "osc";
type PresetAction = "store" | "recall" | "edit" | "delete";

export interface PresetRouteReport {
	seed: string;
	actionIndex: number;
	action: PresetAction;
	family: PresetFamily;
	candidates: readonly PresetRoute[];
	selected: PresetRoute;
}

class PresetActionSurface {
	constructor(
		private readonly owner: BrowserPresets,
		private readonly route: PresetRoute,
	) {}

	store(family: PresetFamily, number: number, options: { mode: "merge" | "overwrite" }) {
		return this.owner.storeVia(this.route, family, number, options.mode);
	}

	recall(family: PresetFamily, number: number) {
		return this.owner.recallVia(this.route, family, number);
	}

	edit(family: PresetFamily, number: number, properties: PresetProperties) {
		return this.owner.editVia(this.route, family, number, properties);
	}

	delete(family: PresetFamily, number: number) {
		return this.owner.deleteVia(this.route, family, number);
	}
}

class PresetExpectation {
	constructor(
		private readonly owner: BrowserPresets,
		private readonly family: PresetFamily,
		private readonly number: number,
	) {}

	async present() {
		expect(await this.owner.object(this.family, this.number)).not.toBeNull();
	}

	async absent() {
		expect(await this.owner.object(this.family, this.number)).toBeNull();
	}

	async metadata(properties: Partial<StoredPreset>) {
		const preset = await this.owner.requiredObject(this.family, this.number);
		expect(preset.body).toMatchObject(properties);
	}

	async button(properties: PresetProperties) {
		await this.owner.expectButton(this.family, this.number, properties);
	}
}

export class BrowserPresets {
	readonly routeReports: PresetRouteReport[] = [];
	readonly via = {
		pool: new PresetActionSurface(this, "pool"),
		keypad: new PresetActionSurface(this, "keypad"),
		api: new PresetActionSurface(this, "api"),
		osc: new PresetActionSurface(this, "osc"),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly commands: BrowserCommands,
		private readonly hardware: SimulatedHardware,
		private readonly activeShowId: () => string,
		private readonly seed: string,
	) {}
	private actionIndex = 0;

	store(family: PresetFamily, number: number, options: { mode: "merge" | "overwrite" }) {
		return this.unqualified(
			"store",
			family,
			this.withOsc(["api", "keypad", "pool"]),
			(route) => this.storeVia(route, family, number, options.mode),
		);
	}

	recall(family: PresetFamily, number: number) {
		return this.unqualified(
			"recall",
			family,
			this.withOsc(["api", "keypad", "pool"]),
			(route) => this.recallVia(route, family, number),
		);
	}

	edit(family: PresetFamily, number: number, properties: PresetProperties) {
		return this.unqualified("edit", family, ["pool"], (route) =>
			this.editVia(route, family, number, properties),
		);
	}

	delete(family: PresetFamily, number: number) {
		return this.unqualified(
			"delete",
			family,
			this.withOsc(["api", "keypad"]),
			(route) => this.deleteVia(route, family, number),
		);
	}

	expect(family: PresetFamily, number: number) {
		return new PresetExpectation(this, family, validPresetNumber(number));
	}

	async storeVia(
		route: PresetRoute,
		family: PresetFamily,
		number: number,
		mode: "merge" | "overwrite",
	) {
		number = validPresetNumber(number);
		const address = presetAddress(productPresetFamily(family), number);
		const before = await this.object(family, number);
		if (route === "api") {
			const session = this.session();
			await new HttpPresetRecordingTransport({
				baseUrl: this.api.baseUrl,
				sessionToken: session.token,
			}).record(this.showId(), {
				requestId: crypto.randomUUID(),
				address,
				name: before?.body.name ?? `Preset ${number}`,
				mode,
				expectedObjectRevision: before?.revision ?? 0,
			});
		} else if (route === "pool") {
			await this.activateFamily(family);
			await this.desk.click(this.page.locator(".global-store-button:visible").first());
			await this.desk.click(this.presetCard(number));
			const choice = this.page.getByRole("button", {
				name: mode === "merge" ? "Merge" : "Overwrite",
				exact: true,
			});
			if (await choice.count()) await this.desk.click(choice);
		} else if (route === "keypad") {
			await this.commands.via.ui.execute(
				`RECORD ${mode === "merge" ? "+ " : ""}${presetCommandAddress(family, number)}`,
			);
		} else {
			await this.sendOsc([
				"record",
				...(mode === "merge" ? ["plus"] : []),
				...presetAddressKeys(family, number),
				"enter",
			]);
		}
		await expect
			.poll(async () => (await this.object(family, number))?.revision ?? 0)
			.toBeGreaterThan(before?.revision ?? 0);
	}

	async recallVia(route: PresetRoute, family: PresetFamily, number: number) {
		number = validPresetNumber(number);
		const preset = await this.requiredObject(family, number);
		if (route === "api") {
			await recallPreset(this.api, {
				surface: "api",
				showId: this.showId(),
				preset: {
					objectId: preset.id,
					family: productPresetFamily(family),
					number,
				},
			});
		} else if (route === "pool") {
			await this.activateFamily(family);
			await this.desk.click(this.presetCard(number));
		} else if (route === "keypad")
			await this.commands.via.ui.execute(presetCommandAddress(family, number));
		else await this.sendOsc([...presetAddressKeys(family, number), "enter"]);
	}

	async editVia(
		route: PresetRoute,
		family: PresetFamily,
		number: number,
		properties: PresetProperties,
	) {
		if (route !== "pool")
			throw new Error(`Preset button customization has no truthful ${route} route`);
		await this.commands.clear();
		await this.activateFamily(family);
		const card = this.presetCard(validPresetNumber(number));
		if (!(await card.evaluate((element) => element.classList.contains("set-target"))))
			await this.desk.click(
				this.page.locator('[data-keypad-key="SET"]:visible').first(),
			);
		await expect(card).toHaveClass(/(?:^|\s)set-target(?:\s|$)/);
		await this.desk.click(card);
		const dialog = this.page.getByRole("dialog", {
			name: "Configure preset button",
		});
		if (properties.title !== undefined)
			await dialog.getByLabel("Title").fill(properties.title);
		if (properties.icon) {
			await dialog.getByRole("button", { name: /Choose icon/i }).click();
			await this.page.getByRole("button", { name: `Use ${properties.icon}` }).click();
		}
		if (properties.color) {
			await dialog.getByRole("button", { name: /#[0-9a-f]{6}/i }).click();
			await this.page
				.getByRole("option", {
					name: `Use color ${properties.color.toLowerCase()}`,
				})
				.click();
		}
		await this.desk.click(dialog.getByRole("button", { name: "Save button" }));
	}

	async deleteVia(route: PresetRoute, family: PresetFamily, number: number) {
		await this.requiredObject(family, number);
		const command = `DELETE ${presetCommandAddress(family, validPresetNumber(number))}`;
		if (route === "api") await this.commands.via.api.execute(command);
		else if (route === "keypad") await this.commands.via.ui.execute(command);
		else if (route === "osc")
			await this.sendOsc(["delete", ...presetAddressKeys(family, number), "enter"]);
		else throw new Error("Preset pool has no visible delete action");
		await expect.poll(async () => this.object(family, number)).toBeNull();
	}

	async object(family: PresetFamily, number: number) {
		const canonical = await this.api.showObject<StoredPreset>(
			this.showId(),
			"preset",
			presetStorageKey(
				presetAddress(productPresetFamily(family), validPresetNumber(number)),
			),
		);
		if (canonical || family !== PresetFamily.Mixed) return canonical;
		return this.api.showObject<StoredPreset>(
			this.showId(),
			"preset",
			String(number),
		);
	}

	async requiredObject(family: PresetFamily, number: number) {
		const preset = await this.object(family, number);
		if (!preset) throw new Error(`${family} Preset ${number} is absent`);
		return preset;
	}

	async expectButton(
		family: PresetFamily,
		number: number,
		properties: PresetProperties,
	) {
		await this.activateFamily(family);
		const card = this.presetCard(validPresetNumber(number));
		if (properties.title !== undefined)
			await expect(card.getByText(properties.title, { exact: true })).toBeVisible();
		if (properties.icon !== undefined)
			await expect(card.getByText(properties.icon, { exact: true })).toBeVisible();
		if (properties.color !== undefined)
			await expect
				.poll(() =>
					card.evaluate((element) =>
						getComputedStyle(element)
							.getPropertyValue("--preset-family")
							.trim()
							.toLowerCase(),
					),
				)
				.toBe(properties.color.toLowerCase());
	}

	private async unqualified(
		action: PresetAction,
		family: PresetFamily,
		candidates: readonly PresetRoute[],
		execute: (route: PresetRoute) => Promise<unknown>,
	) {
		const actionIndex = this.actionIndex++;
		const selected =
			candidates[
				stablePresetRouteIndex(`${this.seed}:${actionIndex}`, candidates.length)
			];
		this.routeReports.push({
			seed: this.seed,
			actionIndex,
			action,
			family,
			candidates,
			selected,
		});
		await execute(selected);
	}

	private withOsc(routes: PresetRoute[]): PresetRoute[] {
		return this.hardware.connected ? [...routes, "osc"] : routes;
	}

	private async activateFamily(family: PresetFamily) {
		const pane = this.page.locator('[data-pane-type="presets"]:visible');
		const direct = pane.getByRole("button", { name: family, exact: true });
		if (await direct.count()) {
			await this.desk.click(direct);
			return;
		}
		await this.desk.click(
			pane.getByRole("button", { name: "Settings", exact: true }),
		);
		const settings = this.page.getByRole("dialog", {
			name: "Pane Settings",
		});
		await this.desk.click(
			settings.getByRole("tab", { name: "Pool", exact: true }),
		);
		await this.desk.click(
			settings.getByRole("button", { name: family, exact: true }),
		);
		await this.desk.click(
			settings.getByRole("button", { name: "Close settings" }),
		);
	}

	private presetCard(number: number) {
		return this.page
			.locator('[data-pane-type="presets"] .preset-card:visible')
			.nth(number - 1);
	}

	private async sendOsc(keys: string[]) {
		if (!this.hardware.connected)
			throw new Error("Preset OSC route requires hardware.connect()");
		const alias = this.session().desk.osc_alias;
		for (const key of keys) {
			const mark = this.hardware.mark();
			await this.hardware.send(`/light/${alias}/programmer/${key}`, [true]);
			await this.hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/command-line`,
			);
			await this.hardware.send(`/light/${alias}/programmer/${key}`, [false]);
		}
	}

	private showId() {
		return this.activeShowId();
	}

	private session() {
		if (!this.api.session) throw new Error("Preset helper requires an API session");
		return this.api.session;
	}
}
