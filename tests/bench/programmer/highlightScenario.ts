import { expect, type Page } from "@playwright/test";
import type {
	HighlightAction,
	HighlightState,
} from "../../../apps/light-desktop/src/api/types/desk";
import type { ApiDriver } from "../core/api";
import type { OscHardware } from "../core/protocols";

export type HighlightSurface = "ui" | "api" | "osc";
export type HighlightPowerAction = Extract<
	HighlightAction,
	"on" | "off" | "toggle"
>;

export interface HighlightActionPort {
	read(): Promise<HighlightState>;
	act(action: HighlightPowerAction): Promise<HighlightState | undefined>;
}

/**
 * Intent-level Highlight power controls. Explicit on/off are idempotent while toggle
 * always crosses the current state. Selection stepping remains a separate concern.
 */
export class HighlightControl {
	constructor(
		private readonly port: HighlightActionPort,
		private readonly timeoutMillis = 2_000,
	) {}

	async on(): Promise<HighlightState> {
		const current = await this.port.read();
		if (current.active) return current;
		const applied = await this.port.act("on");
		if (applied?.active === true) return applied;
		return this.waitFor(true);
	}

	async off(): Promise<HighlightState> {
		const current = await this.port.read();
		if (!current.active) return current;
		const applied = await this.port.act("off");
		if (applied?.active === false) return applied;
		return this.waitFor(false);
	}

	async toggle(): Promise<HighlightState> {
		const expected = !(await this.port.read()).active;
		const applied = await this.port.act("toggle");
		if (applied?.active === expected) return applied;
		return this.waitFor(expected);
	}

	private async waitFor(active: boolean): Promise<HighlightState> {
		const deadline = Date.now() + this.timeoutMillis;
		let latest: HighlightState | undefined;
		do {
			latest = await this.port.read();
			if (latest.active === active) return latest;
			await new Promise<void>((resolve) => setTimeout(resolve, 5));
		} while (Date.now() < deadline);
		throw new Error(
			`Timed out waiting for Highlight ${active ? "on" : "off"}; latest state: ${JSON.stringify(latest)}`,
		);
	}
}

export function highlightApiPort(api: ApiDriver): HighlightActionPort {
	return {
		read: () => readHighlight(api),
		act: (action) =>
			api.request<HighlightState>(
				"POST",
				"/api/v2/output/highlight/actions",
				{ request_id: crypto.randomUUID(), action },
				true,
				undefined,
				{ deskId: deskId(api) },
			),
	};
}

export function highlightUiPort(
	page: Page,
	api: ApiDriver,
): HighlightActionPort {
	return {
		read: () => readHighlight(api),
		act: async (action) => {
			const button =
				action === "toggle"
					? page.locator('[data-keypad-key="HIGH"]')
					: page.getByRole("button", {
							name:
								action === "on" ? "Turn Highlight on" : "Turn Highlight off",
							exact: true,
						});
			await button.click();
			return undefined;
		},
	};
}

export function highlightOscPort(
	hardware: Pick<OscHardware, "send">,
	deskAlias: string,
	api: ApiDriver,
	options: { deliveryTimeoutMillis?: number; debounceMillis?: number } = {},
): HighlightActionPort {
	const deliveryTimeoutMillis = options.deliveryTimeoutMillis ?? 2_000;
	const debounceMillis = options.debounceMillis ?? 155;
	let previous: { action: HighlightPowerAction; sentAt: number } | undefined;
	return {
		read: () => readHighlight(api),
		act: async (action) => {
			const current = await readHighlight(api);
			const expected = action === "toggle" ? !current.active : action === "on";
			for (let attempt = 0; attempt < 2; attempt++) {
				const now = Date.now();
				if (previous?.action === action) {
					const remaining = debounceMillis - (now - previous.sentAt);
					if (remaining > 0)
						await new Promise<void>((resolve) => setTimeout(resolve, remaining));
				}
				await hardware.send(`/light/${deskAlias}/highlight/${action}`, [true]);
				await hardware.send(`/light/${deskAlias}/highlight/${action}`, [false]);
				previous = { action, sentAt: Date.now() };
				const applied = await waitForHighlightState(
					api,
					expected,
					deliveryTimeoutMillis,
				);
				if (applied) return applied;
				// OSC uses UDP. Only repeat after a fresh authoritative read proves
				// that the first datagram did not cross the requested state boundary.
				const latest = await readHighlight(api);
				if (latest.active === expected) return latest;
			}
			return undefined;
		},
	};
}

export class BrowserHighlight {
	readonly via: {
		ui: HighlightControl;
		api: HighlightControl;
		osc: HighlightControl;
	};

	constructor(
		page: Page,
		private readonly api: ApiDriver,
		hardware: Pick<OscHardware, "send">,
	) {
		this.via = {
			ui: new HighlightControl(highlightUiPort(page, api)),
			api: new HighlightControl(highlightApiPort(api)),
			osc: new HighlightControl(
				highlightOscPort(hardware, deskAlias(api), api),
			),
		};
	}

	on() {
		return this.via.ui.on();
	}

	off() {
		return this.via.ui.off();
	}

	toggle() {
		return this.via.ui.toggle();
	}

	async expectSelection(...numbers: number[]): Promise<void> {
		const fixtures = new Map(
			(await this.api.patch()).fixtures.map((fixture) => [
				fixture.fixture_id,
				fixture.fixture_number,
			]),
		);
		await expect
			.poll(async () => {
				const states = await this.api.request<
					Array<{ session_id: string; selected: string[] }>
				>("GET", "/api/v2/programmers");
				const state = states.find(
					(candidate) => candidate.session_id === this.api.session?.session_id,
				);
				return state?.selected.map((id) => fixtures.get(id)) ?? [];
			})
			.toEqual(numbers);
	}

	waitForControlDebounce(): Promise<void> {
		return new Promise((resolve) => setTimeout(resolve, 175));
	}
}

function readHighlight(api: ApiDriver): Promise<HighlightState> {
	return api.request(
		"GET",
		"/api/v2/output/highlight",
		undefined,
		true,
		undefined,
		{ deskId: deskId(api) },
	);
}

async function waitForHighlightState(
	api: ApiDriver,
	active: boolean,
	timeoutMillis: number,
): Promise<HighlightState | undefined> {
	const deadline = Date.now() + timeoutMillis;
	do {
		const state = await readHighlight(api);
		if (state.active === active) return state;
		if (Date.now() >= deadline) return undefined;
		await new Promise<void>((resolve) => setTimeout(resolve, 5));
	} while (true);
}

function deskId(api: ApiDriver): string {
	if (!api.session) throw new Error("API session is not initialized");
	return api.session.desk.id;
}

function deskAlias(api: ApiDriver): string {
	if (!api.session) throw new Error("API session is not initialized");
	return api.session.desk.osc_alias;
}
