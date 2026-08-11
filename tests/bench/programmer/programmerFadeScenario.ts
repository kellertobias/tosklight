import { expect, type Locator, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import { type ClockDuration, parseClockDuration } from "../core/clockScenario";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import { BrowserCueFade } from "../playbacks/cueFadeScenario";

type ProgrammerFadeRoute = "api" | "fader" | "valueEntry" | "osc";
type ProgrammerFadeOperation = "set" | "double" | "half" | "off";

export interface ProgrammerFadeRouteReport {
	seed: string;
	actionIndex: number;
	operation: ProgrammerFadeOperation;
	duration: ClockDuration | null;
	candidates: readonly ProgrammerFadeRoute[];
	selected: ProgrammerFadeRoute;
}

export interface ProgrammerFadeSetPort {
	set(duration: ClockDuration): Promise<void>;
}

class ExplicitProgrammerFadePort implements ProgrammerFadeSetPort {
	constructor(
		private readonly fade: BrowserProgrammerFade,
		private readonly route: ProgrammerFadeRoute,
	) {}

	set(duration: ClockDuration): Promise<void> {
		return this.fade.executeSet(duration, this.route);
	}
}

export class BrowserProgrammerFade implements ProgrammerFadeSetPort {
	readonly via = {
		api: new ExplicitProgrammerFadePort(this, "api"),
		fader: new ExplicitProgrammerFadePort(this, "fader"),
		valueEntry: new ExplicitProgrammerFadePort(this, "valueEntry"),
		osc: new ExplicitProgrammerFadePort(this, "osc"),
	};
	readonly routeReports: ProgrammerFadeRouteReport[] = [];
	private actionIndex = 0;

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly hardware: SimulatedHardware,
		private readonly seed: string,
	) {}

	async set(duration: ClockDuration): Promise<void> {
		const candidates: ProgrammerFadeRoute[] = [
			"api",
			"fader",
			"valueEntry",
			...(this.hardware.connected ? (["osc"] as const) : []),
		];
		const actionIndex = this.actionIndex++;
		const selected =
			candidates[stableIndex(`${this.seed}:${actionIndex}`, candidates.length)];
		this.routeReports.push({
			seed: this.seed,
			actionIndex,
			operation: "set",
			duration,
			candidates,
			selected,
		});
		await this.executeSet(duration, selected);
	}

	async double(): Promise<void> {
		await this.scale("double", (current) => Math.min(20_000, current * 2));
	}

	async half(): Promise<void> {
		await this.scale("half", (current) => Math.floor(current / 2));
	}

	async off(): Promise<void> {
		await this.scale("off", () => 0);
	}

	async currentMillis(): Promise<number> {
		const response = await this.api.request<{
			configuration?: { programmer_fade_millis: number };
			programmer_fade_millis?: number;
		}>("GET", "/api/v2/configuration");
		return (
			response.configuration?.programmer_fade_millis ??
			response.programmer_fade_millis ??
			0
		);
	}

	async setCommandLineAtEnabled(enabled: boolean): Promise<void> {
		await this.api.request("PUT", "/api/v2/configuration", {
			command_line_at_uses_programmer_fade: enabled,
		});
	}

	async executeSet(
		duration: ClockDuration,
		route: ProgrammerFadeRoute,
	): Promise<void> {
		const millis = fadeMillis(duration);
		if (route === "osc" && !this.hardware.connected)
			throw new Error(
				"Programmer Fade OSC route requires connected simulated hardware",
			);
		await this.desk.recordStep(
			"PROGRAMMER FADE",
			`Set Programmer Fade to ${duration} through the ${route} route.`,
		);
		if (route === "api") await this.apiSet(millis);
		if (route === "valueEntry") await this.valueEntrySet(millis);
		if (route === "fader") await this.faderSet(millis);
		if (route === "osc") await this.oscSet(millis);
		await this.waitForMillis(millis);
	}

	private async scale(
		operation: Exclude<ProgrammerFadeOperation, "set">,
		resolve: (current: number) => number,
	): Promise<void> {
		const current = await this.currentMillis();
		const next = resolve(current);
		await this.desk.recordStep(
			`PROGRAMMER FADE ${operation.toUpperCase()}`,
			`${operation} Programmer Fade from ${current} ms to ${next} ms through the API authority.`,
		);
		await this.apiSet(next);
		await this.waitForMillis(next);
	}

	private async apiSet(millis: number): Promise<void> {
		await this.api.request("PUT", "/api/v2/configuration", {
			programmer_fade_millis: millis,
		});
	}

	private async valueEntrySet(millis: number): Promise<void> {
		const surface = await this.visibleSurface();
		await this.desk.click(
			surface.getByRole("button", { name: "Set value", exact: true }),
		);
		const dialog = this.page.getByRole("dialog", {
			name: "Prog. Fade value",
			exact: true,
		});
		await expect(dialog).toBeVisible();
		for (const token of secondsTokens(millis))
			await this.desk.click(
				dialog.getByRole("button", { name: token, exact: true }),
			);
		await expect(dialog).toBeHidden();
	}

	private async faderSet(millis: number): Promise<void> {
		const surface = await this.visibleSurface();
		const input = surface.getByRole("slider", {
			name: "Prog. Fade",
			exact: true,
		});
		const box = await input.boundingBox();
		if (!box)
			throw new Error("Visible Programmer Fade fader has no pointer box");
		const current = await this.currentMillis();
		const endpointZone = Math.min(
			box.height / 3,
			Math.max(18, Math.min(36, box.height * 0.1)),
		);
		const travelHeight = Math.max(1, box.height - endpointZone * 2);
		const y = (value: number) =>
			box.y + endpointZone + (1 - value / 20_000) * travelHeight;
		const x = box.x + box.width / 2;
		let observed = current;
		let targetY = y(millis);
		for (let attempt = 0; attempt < 4; attempt += 1) {
			await this.page.mouse.move(x, y(observed));
			await this.page.mouse.down();
			await this.page.mouse.move(x, targetY, { steps: 8 });
			await this.page.mouse.up();
			await this.page.waitForTimeout(50);
			observed = await this.currentMillis();
			if (observed === millis) return;
			targetY -= ((millis - observed) / 20_000) * travelHeight;
		}
	}

	private async oscSet(millis: number): Promise<void> {
		const session = this.api.session;
		if (!session)
			throw new Error("Programmer Fade OSC route requires a session");
		await this.hardware.send(
			`/light/${session.desk.osc_alias}/programmer/prog-fade`,
			[millis / 20_000],
		);
	}

	private async visibleSurface(): Promise<Locator> {
		const surface = this.page.locator(
			".programmer-fade-fader.full .vertical-touch-fader-stack",
		);
		if (!(await surface.isVisible()))
			await this.desk.click(
				this.page.getByRole("button", { name: /PROG\.\s*PLAYBK/ }),
			);
		await expect(surface).toBeVisible();
		return surface;
	}

	private async waitForMillis(expected: number): Promise<void> {
		const deadline = Date.now() + 5_000;
		do {
			if ((await this.currentMillis()) === expected) return;
			await this.page.waitForTimeout(10);
		} while (Date.now() < deadline);
		throw new Error(
			`Timed out waiting for authoritative Programmer Fade ${expected} ms`,
		);
	}
}

export class BrowserTiming {
	readonly programmerFade: BrowserProgrammerFade;
	readonly cueFade: BrowserCueFade;

	constructor(
		api: ApiDriver,
		page: Page,
		desk: DeskDriver,
		hardware: SimulatedHardware,
		seed: string,
	) {
		this.programmerFade = new BrowserProgrammerFade(
			api,
			page,
			desk,
			hardware,
			seed,
		);
		this.cueFade = new BrowserCueFade(api, desk);
	}
}

function fadeMillis(duration: ClockDuration): number {
	const millis = parseClockDuration(duration);
	if (millis > 20_000)
		throw new Error("Programmer Fade cannot exceed the desk's 20 second range");
	return millis;
}

function secondsTokens(millis: number): string[] {
	return [...String(millis / 1_000), "ENTER"];
}

function stableIndex(value: string, length: number): number {
	let hash = 2166136261;
	for (const character of value) {
		hash ^= character.charCodeAt(0);
		hash = Math.imul(hash, 16777619);
	}
	return (hash >>> 0) % length;
}
