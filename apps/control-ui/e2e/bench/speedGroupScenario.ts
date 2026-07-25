import { expect, type Locator, type Page } from "@playwright/test";
import { HttpSpeedGroupRuntimeTransport } from "../../src/api/SpeedGroupRuntimeTransport";
import type {
	SpeedGroupId,
	SpeedGroupProjection,
} from "../../src/features/speedGroupRuntime/contracts";
import type { ApiDriver } from "./api";
import type { DeskDriver } from "./desk";
import type { LightBench } from "./lightBench";
import { applySpeedGroupRuntimeAction } from "./speedGroupRuntime";

export enum SpeedGroup {
	A = "A",
	B = "B",
	C = "C",
	D = "D",
	E = "E",
}

export interface TapTempoReport {
	seed: string;
	group: SpeedGroup;
	targetBpm: number;
	intervalsMillis: number[];
}

class SpeedGroupClickSurface {
	constructor(private readonly owner: BrowserSpeedGroup) {}

	tapTempo(targetBpm: number, taps = 5) {
		return this.owner.tapTempo(targetBpm, taps);
	}
}

class SpeedGroupModifiedSurface {
	constructor(
		private readonly owner: BrowserSpeedGroup,
		private readonly route: "shiftClick" | "hold",
	) {}

	openSettings() {
		return this.owner.openSettings(this.route);
	}
}

class SpeedGroupExpectation {
	constructor(private readonly owner: BrowserSpeedGroup) {}

	async bpm(expected: number, precision = 1) {
		await expect
			.poll(async () => (await this.owner.projection()).manualBpm)
			.toBeCloseTo(expected, precision);
	}

	async bpmWithin(expected: number, tolerance: number) {
		await expect
			.poll(async () =>
				Math.abs((await this.owner.projection()).manualBpm - expected),
			)
			.toBeLessThanOrEqual(tolerance);
	}

	async synchronizedFrom(source: SpeedGroup | null) {
		await expect
			.poll(async () => (await this.owner.projection()).synchronizedWith)
			.toBe(source);
	}

	async settingsOpen() {
		await expect(this.owner.settingsDialog()).toBeVisible();
	}
}

export class BrowserSpeedGroup {
	readonly via = {
		click: new SpeedGroupClickSurface(this),
		shiftClick: new SpeedGroupModifiedSurface(this, "shiftClick"),
		hold: new SpeedGroupModifiedSurface(this, "hold"),
	};
	readonly expect = new SpeedGroupExpectation(this);

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly bench: LightBench,
		readonly group: SpeedGroup,
		private readonly seed: string,
		private readonly reports: TapTempoReport[],
	) {}

	async setBpm(bpm: number) {
		await this.action({ type: "set_bpm", group: this.group, bpm: validBpm(bpm) });
	}

	async addBpm(bpm: number) {
		await this.adjust(Math.abs(validDelta(bpm)));
	}

	async subtractBpm(bpm: number) {
		await this.adjust(-Math.abs(validDelta(bpm)));
	}

	async synchronizeFrom(source: SpeedGroup) {
		await this.action({
			type: "synchronize",
			source,
			target: this.group,
		});
	}

	async tapTempo(targetBpm: number, taps = 5): Promise<TapTempoReport> {
		targetBpm = validBpm(targetBpm);
		if (!Number.isSafeInteger(taps) || taps < 2 || taps > 8)
			throw new Error("Tap tempo requires 2 to 8 taps");
		await this.openPlaybackTools();
		const intervalsMillis = tapIntervals(this.seed, this.group, targetBpm, taps);
		const button = this.button();
		for (let index = 0; index < taps; index += 1) {
			if (index) {
				await this.page.waitForTimeout(intervalsMillis[index - 1]);
				await this.bench.tick(intervalsMillis[index - 1]);
			}
			await this.desk.click(button);
		}
		await expect(this.settingsDialog()).toHaveCount(0);
		const report = {
			seed: this.seed,
			group: this.group,
			targetBpm,
			intervalsMillis,
		};
		this.reports.push(report);
		return report;
	}

	replayIntervals(report: TapTempoReport) {
		return tapIntervals(
			report.seed,
			report.group,
			report.targetBpm,
			report.intervalsMillis.length + 1,
		);
	}

	async openSettings(route: "shiftClick" | "hold") {
		await this.openPlaybackTools();
		const button = this.button();
		if (route === "shiftClick") await button.click({ modifiers: ["Shift"] });
		else {
			await button.hover();
			await this.page.mouse.down();
			await this.page.waitForTimeout(700);
			await this.page.mouse.up();
		}
		await this.expect.settingsOpen();
	}

	settingsDialog(): Locator {
		return this.page.getByRole("dialog", {
			name: `Speed Group ${this.group} Sound to Light`,
		});
	}

	async closeSettings() {
		const dialog = this.settingsDialog();
		const close = dialog.getByRole("button", { name: /Close/ });
		if (await close.count()) await close.click();
		else await this.page.keyboard.press("Escape");
		await expect(dialog).toBeHidden();
	}

	async projection(): Promise<SpeedGroupProjection> {
		const session = this.session();
		const snapshot = await new HttpSpeedGroupRuntimeTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: session.token,
			authenticatedDeskId: session.desk.id,
		}).loadSnapshot({ deskId: session.desk.id });
		const projection = snapshot.projection.groups.find(
			(candidate) => candidate.group === this.group,
		);
		if (!projection) throw new Error(`Speed Group ${this.group} is absent`);
		return projection;
	}

	private adjust(deltaBpm: number) {
		return this.action({
			type: "adjust_bpm",
			group: this.group,
			deltaBpm,
		});
	}

	private action(action: Parameters<typeof applySpeedGroupRuntimeAction>[1]["action"]) {
		return applySpeedGroupRuntimeAction(this.api, { surface: "api", action });
	}

	private async openPlaybackTools() {
		if (await this.page.locator(".playback-tools").isVisible()) return;
		await this.desk.click(this.page.locator(".mode-toggle"));
		await expect(this.page.locator(".playback-tools")).toBeVisible();
	}

	private button() {
		return this.page.getByRole("button", {
			name: new RegExp(`^Speed group ${this.group}, .* BPM$`),
		});
	}

	private session() {
		if (!this.api.session)
			throw new Error("Speed Group helper requires an API session");
		return this.api.session;
	}
}

export class BrowserSpeedGroups {
	readonly reports: TapTempoReport[] = [];
	readonly A: BrowserSpeedGroup;
	readonly B: BrowserSpeedGroup;
	readonly C: BrowserSpeedGroup;
	readonly D: BrowserSpeedGroup;
	readonly E: BrowserSpeedGroup;

	constructor(
		api: ApiDriver,
		page: Page,
		desk: DeskDriver,
		bench: LightBench,
		seed: string,
	) {
		for (const group of Object.values(SpeedGroup))
			this[group] = new BrowserSpeedGroup(
				api,
				page,
				desk,
				bench,
				group,
				`${seed}:speed-group`,
				this.reports,
			);
	}
}

function tapIntervals(
	seed: string,
	group: SpeedGroup,
	targetBpm: number,
	taps: number,
) {
	const base = 60_000 / targetBpm;
	return Array.from({ length: taps - 1 }, (_, index) => {
		const jitter = stableUnit(`${seed}:${group}:${targetBpm}:${index}`) * 16 - 8;
		return Math.max(80, Math.round(base + jitter));
	});
}

function stableUnit(value: string) {
	let hash = 2166136261;
	for (const character of value) {
		hash ^= character.charCodeAt(0);
		hash = Math.imul(hash, 16777619);
	}
	return (hash >>> 0) / 0xffffffff;
}

function validBpm(value: number) {
	if (!Number.isFinite(value) || value < 1 || value > 999)
		throw new Error("Speed Group BPM must be between 1 and 999");
	return value;
}

function validDelta(value: number) {
	if (!Number.isFinite(value) || value === 0)
		throw new Error("Speed Group adjustment must be non-zero");
	return value;
}
