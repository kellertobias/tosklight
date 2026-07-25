import { expect, type Locator, type Page } from "@playwright/test";
import { HttpPlaybackTopologyTransport } from "../../../src/api/PlaybackTopologyTransport";
import type {
	PlaybackDefinition,
	PlaybackSnapshot,
} from "../../../src/api/types/playback";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import { playbackLocation, validInteger } from "./cuePlaybackScenario";

export enum PlaybackButton {
	Go = "go",
	GoBack = "go_minus",
	On = "on",
	Off = "off",
	Toggle = "toggle",
	Flash = "flash",
	Temp = "temp",
	Swap = "swap",
	Release = "release",
	Pause = "pause",
	Empty = "none",
}

export enum PlaybackFader {
	Master = "master",
	Temporary = "temp",
	Crossfade = "x_fade",
}

export interface PlaybackConfiguration {
	name?: string;
	color?: string;
	autoOff?: boolean;
	buttonCount?: 0 | 1 | 2 | 3;
	hasFader?: boolean;
	buttons?: [
		Exclude<PlaybackButton, PlaybackButton.Release>,
		Exclude<PlaybackButton, PlaybackButton.Release>,
		Exclude<PlaybackButton, PlaybackButton.Release>,
	];
	fader?: PlaybackFader;
}

type RuntimeRoute = "ui" | "api";

export type PlaybackTarget =
	| number
	| { kind: "current_page"; slot: number }
	| { kind: "explicit_page"; page: number; slot: number };

export function currentPagePlayback(slot: number): PlaybackTarget {
	return { kind: "current_page", slot: validInteger(slot, "Playback slot") };
}

export function explicitPagePlayback(
	page: number,
	slot: number,
): PlaybackTarget {
	return {
		kind: "explicit_page",
		page: validInteger(page, "Playback Page"),
		slot: validInteger(slot, "Playback slot"),
	};
}

class MomentaryPlayback {
	constructor(
		private readonly owner: BrowserPlaybacks,
		private readonly route: RuntimeRoute | "osc",
		private readonly target: PlaybackTarget,
		private readonly action: "flash" | "temp" | "swap",
	) {}

	press() {
		return this.owner.actionVia(this.route, this.target, this.action, true);
	}

	release() {
		return this.owner.actionVia(this.route, this.target, this.action, false);
	}

	async hold(callback: () => Promise<void>) {
		await this.press();
		try {
			await callback();
		} finally {
			await this.release();
		}
	}
}

class PlaybackSurface {
	constructor(
		private readonly owner: BrowserPlaybacks,
		private readonly route: RuntimeRoute,
	) {}

	go(target: PlaybackTarget) {
		return this.owner.actionVia(this.route, target, PlaybackButton.Go);
	}

	goBack(target: PlaybackTarget) {
		return this.owner.actionVia(this.route, target, PlaybackButton.GoBack);
	}

	on(number: number) {
		return this.owner.actionVia(this.route, number, PlaybackButton.On);
	}

	off(number: number) {
		return this.owner.actionVia(this.route, number, PlaybackButton.Off);
	}

	toggle(number: number) {
		return this.owner.actionVia(this.route, number, PlaybackButton.Toggle);
	}

	release(number: number) {
		return this.owner.actionVia(this.route, number, PlaybackButton.Release);
	}

	pause(number: number) {
		return this.owner.actionVia(this.route, number, PlaybackButton.Pause);
	}

	select(number: number) {
		return this.owner.selectVia(this.route, number);
	}

	fader(number: number, value: number) {
		return this.owner.faderVia(this.route, number, value);
	}

	flash(target: PlaybackTarget) {
		return new MomentaryPlayback(this.owner, this.route, target, "flash");
	}

	temp(target: PlaybackTarget) {
		return this.owner.actionVia(this.route, target, "temp");
	}

	swap(target: PlaybackTarget) {
		return new MomentaryPlayback(this.owner, this.route, target, "swap");
	}
}

class PlaybackExpectation {
	constructor(
		private readonly owner: BrowserPlaybacks,
		private readonly number: number,
	) {}

	async present() {
		expect(await this.owner.definition(this.number)).not.toBeNull();
	}

	async runtime(expected: Record<string, unknown>) {
		const { master, ...rest } = expected;
		await expect
			.poll(() => this.owner.runtime(this.number))
			.toMatchObject(rest);
		if (typeof master === "number")
			await expect
				.poll(async () => (await this.owner.runtime(this.number))?.master)
				.toBeCloseTo(master, 5);
	}

	async selected() {
		await expect
			.poll(async () => (await this.owner.snapshot()).selected_playback)
			.toBe(this.number);
	}

	async configuration(expected: Partial<PlaybackDefinition>) {
		expect(
			(await this.owner.requiredDefinition(this.number)).body,
		).toMatchObject(expected);
	}
}

export class BrowserPlaybacks {
	readonly via = {
		ui: new PlaybackSurface(this, "ui"),
		api: new PlaybackSurface(this, "api"),
		osc: {
			flash: (target: PlaybackTarget) =>
				new MomentaryPlayback(this, "osc", target, "flash"),
			temp: (target: PlaybackTarget) =>
				new MomentaryPlayback(this, "osc", target, "temp"),
			swap: (target: PlaybackTarget) =>
				new MomentaryPlayback(this, "osc", target, "swap"),
			go: (target: PlaybackTarget) =>
				this.actionVia("osc", target, PlaybackButton.Go),
		},
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly hardware: SimulatedHardware,
		private readonly showId: () => string,
	) {}

	go(number: number) {
		return this.actionVia("ui", number, PlaybackButton.Go);
	}

	goBack(number: number) {
		return this.actionVia("ui", number, PlaybackButton.GoBack);
	}

	on(number: number) {
		return this.actionVia("ui", number, PlaybackButton.On);
	}

	off(number: number) {
		return this.actionVia("ui", number, PlaybackButton.Off);
	}

	toggle(number: number) {
		return this.actionVia("ui", number, PlaybackButton.Toggle);
	}

	release(number: number) {
		return this.actionVia("api", number, PlaybackButton.Release);
	}

	pause(number: number) {
		return this.actionVia("ui", number, PlaybackButton.Pause);
	}

	select(number: number) {
		return this.selectVia("ui", number);
	}

	async nameTargetCuelist(number: number, name: string) {
		number = validInteger(number, "Playback");
		const nextName = name.trim();
		if (!nextName) throw new Error("Cuelist name must not be empty");
		const definition = await this.requiredDefinition(number);
		if (definition.body.target.type !== "cue_list")
			throw new Error(`Playback ${number} does not target a Cuelist`);
		const cueListId = definition.body.target.cue_list_id;
		const cueList = await this.api.showObject<any>(
			this.showId(),
			"cue_list",
			cueListId,
		);
		if (!cueList) throw new Error(`Cuelist ${cueListId} is absent`);
		await this.api.seedShowObject(
			this.showId(),
			"cue_list",
			cueListId,
			{ ...cueList.body, name: nextName },
			cueList.revision,
		);
	}

	async selectFromHardwareCard(number: number) {
		number = validInteger(number, "Playback");
		if (!this.hardware.connected)
			throw new Error(
				"Hardware-card selection requires hardware.connect() first",
			);
		await this.desk.recordStep(
			"HARDWARE PLAYBACK SELECT",
			`Select concrete Playback ${number} from its attached-hardware card header.`,
		);
		const definition = await this.requiredDefinition(number);
		if (definition.body.target.type !== "cue_list")
			throw new Error(`Playback ${number} does not target a Cuelist`);
		const cueList = await this.api.showObject<any>(
			this.showId(),
			"cue_list",
			definition.body.target.cue_list_id,
		);
		if (!cueList)
			throw new Error(
				`Cuelist ${definition.body.target.cue_list_id} is absent`,
			);
		const card = await this.visibleCard(number);
		await expect(
			card.getByRole("button", { name: /Playback representation/ }),
		).toHaveCount(0);
		await this.desk.click(card.locator("header b"));
		await expect(this.page.locator(".ui-window-header")).toContainText(
			`Cuelist ${number} · ${cueList.body.name}`,
		);
		await this.expect(number).selected();
	}

	fader(number: number, value: number) {
		return this.faderVia("ui", number, value);
	}

	async open() {
		if (await this.page.locator(".playback-fader-bank").isVisible()) return;
		await this.desk.click(this.page.locator(".mode-toggle"));
		await expect(this.page.locator(".playback-fader-bank")).toBeVisible();
	}

	expect(number: number) {
		return new PlaybackExpectation(this, validInteger(number, "Playback"));
	}

	async expectActivatedTogether(...numbers: number[]) {
		if (numbers.length < 2)
			throw new Error("Shared activation requires at least two Playbacks");
		await expect
			.poll(async () => {
				const timestamps = await Promise.all(
					numbers.map(
						async (number) => (await this.runtime(number))?.activated_at,
					),
				);
				return timestamps.every(Boolean) ? new Set(timestamps).size : 0;
			})
			.toBe(1);
	}

	async actionVia(
		route: RuntimeRoute | "osc",
		target: PlaybackTarget,
		action: PlaybackButton | "flash" | "temp" | "swap",
		pressed = true,
	) {
		const number =
			typeof target === "number"
				? validInteger(target, "Playback")
				: await this.playbackNumber(target);
		if (route === "api") {
			if (action === PlaybackButton.Release) {
				const definition = await this.requiredDefinition(number);
				if (definition.body.target.type !== "cue_list")
					throw new Error("Playback release requires a Cuelist target");
				await this.api.cueListPlaybackAction(
					definition.body.target.cue_list_id,
					"release",
					{},
				);
			} else {
				const wireAction =
					action === PlaybackButton.GoBack ? "go-minus" : action;
				await this.apiAction(target, wireAction, pressed);
			}
		} else if (route === "ui") {
			if (action === PlaybackButton.Release)
				throw new Error("Playback release has no default visible button");
			const button = (await this.visibleCardFor(target, number)).getByRole(
				"button",
				{
					name: playbackButtonLabel(action),
					exact: true,
				},
			);
			if (["flash", "swap"].includes(action)) {
				if (pressed) {
					await button.hover();
					await this.page.mouse.down();
				} else await this.page.mouse.up();
			} else await this.desk.click(button);
		} else await this.oscAction(target, number, action, pressed);
	}

	async selectVia(route: RuntimeRoute, number: number) {
		if (route === "api") await this.api.playbackNumberAction(number, "select");
		else {
			await this.desk.recordStep(
				"PLAYBACK SELECT",
				`Select Playback ${number} with the documented Shift+Z shortcut and visible representation.`,
			);
			await this.open();
			await this.page.keyboard.press("Shift+KeyZ");
			await expect(this.page.getByLabel("Command line")).toHaveValue("SELECT");
			await this.desk.click(
				(await this.visibleCard(number)).getByRole("button", {
					name: /Playback representation/,
				}),
			);
		}
		await this.expect(number).selected();
	}

	async faderVia(route: RuntimeRoute, number: number, value: number) {
		if (!Number.isFinite(value) || value < 0 || value > 100)
			throw new Error("Playback fader value must be between 0 and 100");
		if (route === "api")
			await this.api.playbackNumberAction(number, "master", {
				value: value / 100,
			});
		else
			await (await this.visibleCard(number))
				.getByRole("slider")
				.fill(String(value));
	}

	async configure(number: number, definition: PlaybackConfiguration) {
		number = validInteger(number, "Playback");
		const current = await this.requiredDefinition(number);
		const location = await playbackLocation(this.api, this.showId(), number);
		const page = await this.api.showObject<any>(
			this.showId(),
			"playback_page",
			String(location.page),
		);
		if (!page) throw new Error(`Playback Page ${location.page} is absent`);
		const session = this.session();
		const runtime = await this.api.request<any>(
			"POST",
			"/api/v2/playback-runtime/snapshot",
			{ identities: [] },
			true,
			undefined,
			{ showId: this.showId(), deskId: session.desk.id },
		);
		await new HttpPlaybackTopologyTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: session.token,
		}).apply(this.showId(), runtime.desk.scope.show_revision, {
			requestId: crypto.randomUUID(),
			action: {
				type: "configure_slot",
				page: location.page,
				slot: location.slot,
				expectedPageRevision: page.revision,
				expectedPageObjectId: page.id,
				expectedPlaybackRevision: current.revision,
				expectedPlaybackObjectId: current.id,
				playback: {
					...current.body,
					...(definition.name == null ? {} : { name: definition.name }),
					...(definition.color == null ? {} : { color: definition.color }),
					...(definition.autoOff == null
						? {}
						: { auto_off: definition.autoOff }),
					...(definition.buttonCount == null
						? {}
						: { button_count: definition.buttonCount }),
					...(definition.hasFader == null
						? {}
						: { has_fader: definition.hasFader }),
					...(definition.buttons == null
						? {}
						: { buttons: definition.buttons }),
					...(definition.fader == null ? {} : { fader: definition.fader }),
				},
			},
		});
	}

	definition(number: number) {
		return this.api.showObject<PlaybackDefinition>(
			this.showId(),
			"playback",
			String(number),
		);
	}

	async requiredDefinition(number: number) {
		const definition = await this.definition(number);
		if (!definition) throw new Error(`Playback ${number} is absent`);
		return definition;
	}

	snapshot() {
		return this.api.request<PlaybackSnapshot>(
			"GET",
			"/api/v2/playback-overview",
		);
	}

	async runtime(number: number) {
		return (await this.snapshot()).active.find(
			(item) => item.playback_number === number,
		);
	}

	private async visibleCard(number: number): Promise<Locator> {
		await this.open();
		const location = await playbackLocation(this.api, this.showId(), number);
		const card = this.page.locator(
			`.playback-fader-bank [data-playback-slot="${location.slot}"]:visible`,
		);
		await expect(card).toBeVisible();
		return card;
	}

	private async visibleCardFor(
		target: PlaybackTarget,
		number: number,
	): Promise<Locator> {
		if (typeof target === "number") return this.visibleCard(number);
		await this.open();
		const card = this.page.locator(
			`.playback-fader-bank [data-playback-slot="${target.slot}"]:visible`,
		);
		await expect(card).toBeVisible();
		return card;
	}

	private apiAction(target: PlaybackTarget, action: string, pressed: boolean) {
		const input = { pressed };
		if (typeof target === "number")
			return this.api.playbackNumberAction(target, action, input);
		if (target.kind === "current_page")
			return this.api.currentPagePlaybackAction(target.slot, action, input);
		return this.api.explicitPagePlaybackAction(
			target.page,
			target.slot,
			action,
			input,
		);
	}

	private async playbackNumber(target: Exclude<PlaybackTarget, number>) {
		const page =
			target.kind === "explicit_page"
				? target.page
				: (await this.snapshot()).active_page;
		const object = await this.api.showObject<any>(
			this.showId(),
			"playback_page",
			String(page),
		);
		const number = object?.body.slots[String(target.slot)];
		if (typeof number !== "number")
			throw new Error(`Playback Page ${page} slot ${target.slot} is empty`);
		return number;
	}

	private async oscAction(
		target: PlaybackTarget,
		number: number,
		action: PlaybackButton | "flash" | "temp" | "swap",
		pressed: boolean,
	) {
		if (!this.hardware.connected)
			throw new Error("Playback OSC route requires hardware.connect()");
		const definition = await this.requiredDefinition(number);
		const button = definition.body.buttons.indexOf(
			action === PlaybackButton.GoBack ? "go_minus" : (action as any),
		);
		if (button < 0)
			throw new Error(`Playback ${number} has no configured ${action} button`);
		const alias = this.session().desk.osc_alias;
		const address =
			typeof target !== "number" && target.kind === "current_page"
				? `/light/${alias}/page-playback/${target.slot}/button/${button + 1}`
				: await this.explicitOscAddress(target, number, button + 1);
		await this.hardware.send(address, [pressed]);
	}

	private async explicitOscAddress(
		target: PlaybackTarget,
		number: number,
		button: number,
	) {
		const location =
			typeof target === "number"
				? await playbackLocation(this.api, this.showId(), number)
				: target;
		return `/light/playback/${location.page}/${location.slot}/button/${button}`;
	}

	private session() {
		if (!this.api.session)
			throw new Error("Playback helper requires an API session");
		return this.api.session;
	}
}

function playbackButtonLabel(action: PlaybackButton) {
	const labels: Partial<Record<PlaybackButton, string>> = {
		[PlaybackButton.Go]: "GO +",
		[PlaybackButton.GoBack]: "GO −",
		[PlaybackButton.On]: "ON",
		[PlaybackButton.Off]: "OFF",
		[PlaybackButton.Toggle]: "TOGGLE",
		[PlaybackButton.Flash]: "FLASH",
		[PlaybackButton.Temp]: "TEMP",
		[PlaybackButton.Swap]: "SWAP",
		[PlaybackButton.Pause]: /^(?:PAUSE|RESUME)$/,
	};
	const label = labels[action];
	if (!label) throw new Error(`Playback ${action} has no visible button label`);
	return label;
}
