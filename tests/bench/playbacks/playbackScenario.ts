import { expect, type Locator, type Page } from "@playwright/test";
import { HttpPlaybackTopologyTransport } from "../../../apps/light-desktop/src/api/PlaybackTopologyTransport";
import type {
	PlaybackDefinition,
	PlaybackPage,
} from "../../../apps/light-desktop/src/api/types/playback";
import { decodePlaybackSnapshot } from "../../../apps/light-desktop/src/api/playbackWire";
import type {
	PlaybackAddressValue,
	PlaybackOverview,
} from "../../../apps/light-desktop/src/api/generated/light-wire";
import { commandKeys } from "../command-selection/commandScenario";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import { validInteger } from "./cuePlaybackScenario";

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
	| PlaybackAddressValue
	| { kind: "current_page"; position: number };

export function currentPagePlayback(position: number): PlaybackTarget {
	return {
		kind: "current_page",
		position: validInteger(position, "Playback position"),
	};
}

export function explicitPagePlayback(
	page: number,
	position: number,
): PlaybackAddressValue {
	if (page > 99 || position > 1000)
		throw new Error("Playback address is out of range");
	return {
		page: validInteger(page, "Playback Page"),
		position: validInteger(position, "Playback position"),
	};
}

class MomentaryPlayback {
	private heldTarget: PlaybackTarget | undefined;
	constructor(
		private readonly owner: BrowserPlaybacks,
		private readonly route: RuntimeRoute | "osc",
		private readonly target: PlaybackTarget,
		private readonly action: "flash" | "temp" | "swap",
	) {}

	async press() {
		this.heldTarget = this.target;
		if ("kind" in this.target && this.target.kind === "current_page")
			this.heldTarget = explicitPagePlayback(
				await this.owner.commandPage(),
				this.target.position,
			);
		return this.owner.actionVia(this.route, this.heldTarget, this.action, true);
	}

	release() {
		if (!this.heldTarget) throw new Error("Playback hold has not been pressed");
		const target = this.heldTarget;
		this.heldTarget = undefined;
		return this.owner.actionVia(this.route, target, this.action, false);
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

	on(address: PlaybackAddressValue) {
		return this.owner.actionVia(this.route, address, PlaybackButton.On);
	}

	off(address: PlaybackAddressValue) {
		return this.owner.actionVia(this.route, address, PlaybackButton.Off);
	}

	toggle(address: PlaybackAddressValue) {
		return this.owner.actionVia(this.route, address, PlaybackButton.Toggle);
	}

	release(address: PlaybackAddressValue) {
		return this.owner.actionVia(this.route, address, PlaybackButton.Release);
	}

	pause(address: PlaybackAddressValue) {
		return this.owner.actionVia(this.route, address, PlaybackButton.Pause);
	}

	select(address: PlaybackAddressValue) {
		return this.owner.selectVia(this.route, address);
	}

	fader(address: PlaybackAddressValue, value: number) {
		return this.owner.faderVia(this.route, address, value);
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
		private readonly address: PlaybackAddressValue,
	) {}
	async present() {
		expect(await this.owner.definition(this.address)).not.toBeNull();
	}
	async runtime(expected: Record<string, unknown>) {
		const { master, ...rest } = expected;
		await expect
			.poll(() => this.owner.runtime(this.address))
			.toMatchObject(rest);
		if (typeof master === "number")
			await expect
				.poll(async () => {
					const value = await this.owner.runtime(this.address);
					return value && "master" in value ? value.master : undefined;
				})
				.toBeCloseTo(master, 5);
	}
	async selected() {
		await expect
			.poll(async () => (await this.owner.snapshot()).selected_playback)
			.toEqual(this.address);
	}
	async configuration(expected: Partial<PlaybackDefinition>) {
		expect(
			(await this.owner.requiredDefinition(this.address)).body,
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

	go(address: PlaybackAddressValue) {
		return this.actionVia("ui", address, PlaybackButton.Go);
	}

	goBack(address: PlaybackAddressValue) {
		return this.actionVia("ui", address, PlaybackButton.GoBack);
	}

	on(address: PlaybackAddressValue) {
		return this.actionVia("ui", address, PlaybackButton.On);
	}

	off(address: PlaybackAddressValue) {
		return this.actionVia("ui", address, PlaybackButton.Off);
	}

	toggle(address: PlaybackAddressValue) {
		return this.actionVia("ui", address, PlaybackButton.Toggle);
	}

	release(address: PlaybackAddressValue) {
		return this.actionVia("api", address, PlaybackButton.Release);
	}

	pause(address: PlaybackAddressValue) {
		return this.actionVia("ui", address, PlaybackButton.Pause);
	}

	select(address: PlaybackAddressValue) {
		return this.selectVia("ui", address);
	}

	async nameTargetCuelist(address: PlaybackAddressValue, name: string) {
		const nextName = name.trim();
		if (!nextName) throw new Error("Cuelist name must not be empty");
		const definition = await this.requiredDefinition(address);
		if (definition.body.target.type !== "cue_list")
			throw new Error("Playback does not target a Cuelist");
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

	async selectFromHardwareCard(address: PlaybackAddressValue) {
		if (!this.hardware.connected)
			throw new Error(
				"Hardware-card selection requires hardware.connect() first",
			);
		const definition = await this.requiredDefinition(address);
		if (definition.body.target.type !== "cue_list")
			throw new Error("Playback does not target a Cuelist");
		const cueList = await this.api.showObject<any>(
			this.showId(),
			"cue_list",
			definition.body.target.cue_list_id,
		);
		if (!cueList) throw new Error("Assigned Cuelist is absent");
		const card = await this.visibleCard(address);
		await expect(
			card.getByRole("button", { name: /Playback representation/ }),
		).toHaveCount(0);
		await this.desk.click(card.locator("header b"));
		await expect(
			this.page.getByRole("main", { name: "cuelists built-in" }),
		).toContainText(cueList.body.name);
		await this.expect(address).selected();
	}

	fader(address: PlaybackAddressValue, value: number) {
		return this.faderVia("ui", address, value);
	}

	async open() {
		if (await this.page.locator(".playback-fader-bank").isVisible()) return;
		await this.desk.click(this.page.locator(".mode-toggle"));
		await expect(this.page.locator(".playback-fader-bank")).toBeVisible();
	}

	expect(address: PlaybackAddressValue) {
		return new PlaybackExpectation(this, address);
	}

	async expectActivatedTogether(...addresses: PlaybackAddressValue[]) {
		if (addresses.length < 2)
			throw new Error("Shared activation requires at least two Playbacks");
		await expect
			.poll(async () => {
				const timestamps = await Promise.all(
					addresses.map(async (address) => {
						const value = await this.runtime(address);
						return value && "activated_at" in value
							? value.activated_at
							: undefined;
					}),
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
		const address = await this.resolveTarget(target);
		if (route === "api") {
			await this.api.playbackAction(
				address,
				action === PlaybackButton.GoBack ? "go-minus" : action,
				{ pressed },
			);
			return;
		}
		if (route === "osc") {
			await this.oscAction(address, action, pressed);
			return;
		}
		if (action === PlaybackButton.Release)
			throw new Error("Playback release has no default visible button");
		if (["flash", "swap"].includes(action) && !pressed) {
			await this.page.mouse.up();
			return;
		}
		const button = (await this.visibleCard(address)).getByRole("button", {
			name: playbackButtonLabel(action),
			exact: true,
		});
		if (["flash", "swap"].includes(action)) {
			await button.hover();
			await this.page.mouse.down();
		} else await this.desk.click(button);
	}

	async selectVia(route: RuntimeRoute, address: PlaybackAddressValue) {
		if (route === "api") await this.api.playbackAction(address, "select");
		else {
			const keypad = (key: string) =>
				this.page.locator(`[data-keypad-key="${key}"]:visible`).first();
			if (!(await keypad("GRP").isVisible())) {
				await this.desk.click(this.page.locator(".mode-toggle"));
				await expect(keypad("GRP")).toBeVisible();
			}
			await this.desk.click(
				this.page.locator(".command-escape:visible").first(),
			);
			for (const key of commandKeys(
				`PBK ${address.page} . ${address.position}`,
				"FIXTURE",
			))
				await this.desk.click(keypad(key));
			await this.desk.click(keypad("ENT"));
		}
		await this.expect(address).selected();
	}

	async faderVia(
		route: RuntimeRoute,
		address: PlaybackAddressValue,
		value: number,
	) {
		if (!Number.isFinite(value) || value < 0 || value > 100)
			throw new Error("Playback fader value must be between 0 and 100");
		if (route === "api")
			await this.api.playbackAction(address, "master", { value: value / 100 });
		else
			await (await this.visibleCard(address))
				.getByRole("slider")
				.fill(String(value));
	}

	async configure(
		address: PlaybackAddressValue,
		definition: PlaybackConfiguration,
	) {
		const current = await this.requiredDefinition(address);
		const page = await this.api.showObject<PlaybackPage>(
			this.showId(),
			"playback_page",
			String(address.page),
		);
		if (!page) throw new Error(`Playback Page ${address.page} is absent`);
		const session = this.session();
		const runtime = decodePlaybackSnapshot(
			await this.api.request<unknown>(
				"POST",
				"/api/v2/playback-runtime/snapshot",
				{ identities: [] },
				true,
				undefined,
				{ showId: this.showId(), deskId: session.desk.id },
			),
		);
		await new HttpPlaybackTopologyTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: session.token,
		}).apply(this.showId(), runtime.desk.scope.show_revision, {
			requestId: crypto.randomUUID(),
			action: {
				type: "update_playback",
				address,
				expectedPageRevision: page.revision,
				expectedPageObjectId: page.id,
				definition: {
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

	async definition(address: PlaybackAddressValue) {
		const page = await this.api.showObject<PlaybackPage>(
			this.showId(),
			"playback_page",
			String(address.page),
		);
		const body = page?.body.assignments[String(address.position)];
		return body && page ? { id: page.id, revision: page.revision, body } : null;
	}
	async requiredDefinition(address: PlaybackAddressValue) {
		const definition = await this.definition(address);
		if (!definition)
			throw new Error(`Playback ${address.page}.${address.position} is absent`);
		return definition;
	}
	snapshot() {
		return this.api.request<PlaybackOverview>(
			"GET",
			"/api/v2/playback-overview",
		);
	}

	async runtime(address: PlaybackAddressValue) {
		const session = this.session();
		const snapshot = decodePlaybackSnapshot(
			await this.api.request<unknown>(
				"POST",
				"/api/v2/playback-runtime/snapshot",
				{
					identities: [{ kind: "playback", ...address }],
				},
				true,
				undefined,
				{ showId: this.showId(), deskId: session.desk.id },
			),
		);
		const projection = snapshot.projections.find(
			(item) =>
				item.identity.kind === "playback" &&
				item.identity.page === address.page &&
				item.identity.position === address.position,
		);
		if (projection?.target === "cue_list") {
			const runtime = projection.runtime;
			return runtime
				? {
						...runtime,
						address,
						current_cue_number: runtime.current?.number ?? null,
						loaded_cue_number: runtime.loaded?.number ?? null,
						normal_next_cue_number: runtime.normal_next?.number ?? null,
						effective_next_cue_number: runtime.effective_next?.number ?? null,
					}
				: {
						enabled: false,
						flash: false,
						temporary_active: false,
						swap_active: false,
						current_cue_number: null,
						loaded_cue_number: null,
					};
		}
		return projection?.target === "dynamic" ? projection.runtime : projection;
	}

	private async visibleCard(address: PlaybackAddressValue): Promise<Locator> {
		await this.open();
		const card = this.page.locator(
			`.playback-fader-bank [data-page="${address.page}"][data-playback-slot="${address.position}"]:visible`,
		);
		await expect(card).toBeVisible();
		return card;
	}
	commandPage() {
		return this.api.playbackCommandPage();
	}
	private async resolveTarget(
		target: PlaybackTarget,
	): Promise<PlaybackAddressValue> {
		return "kind" in target
			? explicitPagePlayback(await this.commandPage(), target.position)
			: explicitPagePlayback(target.page, target.position);
	}
	private async oscAction(
		address: PlaybackAddressValue,
		action: PlaybackButton | "flash" | "temp" | "swap",
		pressed: boolean,
	) {
		if (!this.hardware.connected)
			throw new Error("Playback OSC route requires hardware.connect()");
		const definition = await this.requiredDefinition(address);
		const button = definition.body.buttons.indexOf(
			action === PlaybackButton.GoBack
				? "go_minus"
				: (action as (typeof definition.body.buttons)[number]),
		);
		if (button < 0)
			throw new Error(
				`Playback ${address.page}.${address.position} has no configured ${action} button`,
			);
		await this.hardware.send(
			`/light/playback/${address.page}/${address.position}/button/${button + 1}`,
			[pressed],
		);
	}

	private session() {
		if (!this.api.session)
			throw new Error("Playback helper requires an API session");
		return this.api.session;
	}
}

function playbackButtonLabel(
	action: PlaybackButton | "flash" | "swap" | "temp",
) {
	const labels: Partial<Record<string, string | RegExp>> = {
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
