import { expect, type Locator, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { DynamicHandle } from "./dynamicScenario";

/** A lane as the lane chooser offers it: its attribute group, then the attribute inside. */
export interface DynamicLaneChoice {
	group: string;
	attribute: string;
}

/** A curve function as the Curve function chooser names it. */
export type DynamicCurveFunction =
	| "Sinus"
	| "Cosinus"
	| "Linear +"
	| "Linear −"
	| "PWM";

/** The typed address one stored lane programs, or the scalar attribute it drives. */
export type StoredDynamicLane =
	| { family: "position"; component: "pan" | "tilt" }
	| { family: "zoom" }
	| { family: "color"; component: string }
	| { scalar: string };

/**
 * The Dynamics window's editor driven by touch, as an operator builds a Dynamic: an empty pool
 * tile opens the lane chooser, the editor adds lanes, picks curve functions and reads the lane
 * encoders, and a tile tap toggles the Dynamic on the current selection.
 */
export class BrowserDynamicEditor {
	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly activeShowId: () => string,
	) {}

	private get window() {
		return this.page.locator(".dynamics-window");
	}

	private get editor() {
		return this.page.locator(".dynamics-editor");
	}

	/** Taps empty pool tile `pool` and chooses its first lane; the editor opens on it. */
	async create(pool: number, lane: DynamicLaneChoice): Promise<DynamicHandle> {
		await this.desk.click(
			this.window.locator(".dynamic-pool-card").nth(pool - 1),
		);
		await this.chooseLane("Select lane attribute", lane);
		await expect(this.editor).toBeVisible();
		const stored = await expect
			.poll(async () => (await this.stored(pool))?.id, { timeout: 10_000 })
			.toBeTruthy()
			.then(() => this.stored(pool));
		if (!stored) throw new Error(`Dynamic ${pool} was not created`);
		return { id: stored.id, pool, name: stored.body.name };
	}

	/** + Add Lane, then the lane chooser; waits until the editor shows the stored lanes. */
	async addLane(dynamic: DynamicHandle, lane: DynamicLaneChoice): Promise<void> {
		const before = (await this.stored(dynamic.pool))?.revision ?? 0;
		await this.desk.click(
			this.editor.getByRole("button", { name: "+ Add Lane", exact: true }),
		);
		await this.chooseLane("Select lane attribute", lane);
		await expect
			.poll(async () => (await this.stored(dynamic.pool))?.revision ?? 0)
			.toBeGreaterThan(before);
		const stored = await this.stored(dynamic.pool);
		const count = stored?.body.lanes.length ?? 0;
		await expect(
			this.editor.getByRole("list", { name: "Dynamic lanes" }).getByRole("listitem"),
		).toHaveCount(count);
	}

	/** Taps lane `index` (1-based) in the lane list; its label must read `label`. */
	async selectLane(index: number, label: string): Promise<void> {
		const lane = this.editor.getByRole("button", {
			name: `Select lane ${index}, ${label}`,
			exact: true,
		});
		await this.desk.click(lane);
		await expect(lane).toHaveAttribute("aria-pressed", "true");
	}

	/** Chooses the selected lane's curve function in the Curve Composer. */
	async chooseCurve(curve: DynamicCurveFunction): Promise<void> {
		await this.desk.click(
			this.editor.getByRole("button", { name: /^Curve function:/ }),
		);
		const chooser = this.page.getByRole("dialog", {
			name: "Choose curve function",
		});
		await expect(chooser).toBeVisible();
		await this.desk.click(
			chooser.getByRole("button", { name: new RegExp(`^${escape(curve)}\\b`) }),
		);
		await expect(chooser).toBeHidden();
		await expect(
			this.editor.getByRole("button", { name: `Curve function: ${curve}` }),
		).toBeVisible();
	}

	/** Enters a lane encoder's value on its value modal keypad, in the units the encoder shows. */
	async setEncoder(label: string, value: string): Promise<void> {
		await this.desk.click(this.encoder(label));
		const dialog = this.page.getByRole("dialog", {
			name: new RegExp(`^Enc \\d+ · ${label} value$`),
		});
		await expect(dialog).toBeVisible();
		const keypad = dialog.getByLabel("Number input keypad");
		const entered = (await dialog.getByRole("textbox").textContent()) ?? "";
		for (const _ of entered.trim())
			await this.desk.click(keypad.getByRole("button", { name: "⌫" }));
		for (const key of value)
			await this.desk.click(keypad.getByRole("button", { name: key, exact: true }));
		await this.desk.click(keypad.getByRole("button", { name: "ENTER" }));
		await expect(dialog).toBeHidden();
	}

	/** Settings → Targets → Take Selection, then back to the pool. */
	async takeSelectionAndClose(): Promise<void> {
		await this.desk.click(
			this.editor.getByRole("button", { name: "Settings", exact: true }),
		);
		const settings = this.page.getByRole("dialog", { name: "Dynamic Settings" });
		await this.desk.click(
			settings.getByRole("tab", { name: "Targets", exact: true }),
		);
		await this.desk.click(
			settings.getByRole("button", { name: "Take Selection", exact: true }),
		);
		await this.desk.click(
			settings.getByRole("button", { name: "Close settings", exact: true }),
		);
		await this.desk.click(
			this.editor.getByRole("button", { name: "← Dynamics", exact: true }),
		);
		await expect(this.editor).toBeHidden();
	}

	/** Taps the Dynamic's pool tile, toggling it as the pool does. */
	async toggle(dynamic: DynamicHandle): Promise<void> {
		await this.desk.click(
			this.window.locator(".dynamic-pool-card").nth(dynamic.pool - 1),
		);
	}

	readonly expect = {
		/** A lane encoder reads `display`, in the lane's own units. */
		encoder: async (label: string, display: string) =>
			expect(this.encoder(label)).toHaveText(display),
		/** No error banner: the desk accepted every edit. */
		noError: async () =>
			expect(this.editor.getByRole("alert")).toHaveCount(0),
		/** The stored lanes, in order, by their typed address or scalar attribute. */
		lanes: async (dynamic: DynamicHandle, lanes: StoredDynamicLane[]) =>
			expect
				.poll(async () => {
					const stored = await this.stored(dynamic.pool);
					return (stored?.body.lanes ?? []).map(storedLane);
				})
				.toEqual(lanes),
		/** One stored lane's typed configuration, matched partially. */
		laneConfiguration: async (
			dynamic: DynamicHandle,
			index: number,
			configuration: Record<string, unknown>,
		) =>
			expect
				.poll(
					async () =>
						(await this.stored(dynamic.pool))?.body.lanes[index - 1]
							?.programming?.configuration,
				)
				.toMatchObject(configuration),
	};

	/** A lane encoder; the Dynamics editor takes over the desk's encoder section. */
	private encoder(label: string): Locator {
		return this.page
			.getByRole("region", { name: "Lanes encoders" })
			.getByRole("button", {
				name: new RegExp(`^Set Enc \\d+ · ${label} value$`),
			});
	}

	private async chooseLane(title: string, lane: DynamicLaneChoice) {
		const chooser = this.page.getByRole("dialog", { name: title });
		await expect(chooser).toBeVisible();
		await this.desk.click(
			chooser.getByRole("button", { name: lane.group, exact: true }),
		);
		await this.desk.click(
			chooser.getByRole("button", { name: lane.attribute, exact: true }),
		);
		await expect(chooser).toBeHidden();
	}

	private async stored(pool: number) {
		const dynamics = await this.api.showObjects<{
			lanes: Array<{
				attribute?: string;
				programming?: {
					address: {
						representation: { kind: string };
						component: { kind: string; component?: string } | null;
					};
					configuration: unknown;
				};
			}>;
			name: string;
			pool_number: number;
		}>(this.activeShowId(), "dynamic");
		return dynamics.find((dynamic) => dynamic.body.pool_number === pool);
	}
}

function storedLane(lane: {
	attribute?: string;
	programming?: {
		address: {
			representation: { kind: string };
			component: { kind: string; component?: string } | null;
		};
	};
}): StoredDynamicLane {
	if (lane.attribute !== undefined) return { scalar: lane.attribute };
	const address = lane.programming?.address;
	const component = address?.component;
	if (address?.representation.kind === "angles")
		return {
			family: "position",
			component: component?.kind as "pan" | "tilt",
		};
	if (address?.representation.kind === "zoom") return { family: "zoom" };
	return { family: "color", component: component?.component ?? "whole" };
}

function escape(value: string) {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
