import { expect, type Locator, type Page } from "@playwright/test";
import { HttpPlaybackTopologyTransport } from "../../src/api/PlaybackTopologyTransport";
import type {
	PlaybackDefinition,
	PlaybackSnapshot,
} from "../../src/api/types/playback";
import type { ApiDriver } from "./api";
import {
	playbackLocation,
	validInteger,
} from "./cuePlaybackScenario";
import type { DeskDriver } from "./desk";

export enum PlaybackButton {
	Go = "go",
	GoBack = "go_minus",
	On = "on",
	Off = "off",
	Toggle = "toggle",
	Release = "release",
}

export enum PlaybackFader {
	Master = "master",
	Temporary = "temp",
	Crossfade = "x_fade",
}

export interface PlaybackConfiguration {
	name?: string;
	color?: string;
	buttons?: [
		Exclude<PlaybackButton, PlaybackButton.Release>,
		Exclude<PlaybackButton, PlaybackButton.Release>,
		Exclude<PlaybackButton, PlaybackButton.Release>,
	];
	fader?: PlaybackFader;
}

type RuntimeRoute = "ui" | "api";

class PlaybackSurface {
	constructor(
		private readonly owner: BrowserPlaybacks,
		private readonly route: RuntimeRoute,
	) {}

	go(number: number) {
		return this.owner.actionVia(this.route, number, PlaybackButton.Go);
	}

	goBack(number: number) {
		return this.owner.actionVia(this.route, number, PlaybackButton.GoBack);
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

	select(number: number) {
		return this.owner.selectVia(this.route, number);
	}

	fader(number: number, value: number) {
		return this.owner.faderVia(this.route, number, value);
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
		await expect.poll(() => this.owner.runtime(this.number)).toMatchObject(rest);
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
		expect((await this.owner.requiredDefinition(this.number)).body).toMatchObject(
			expected,
		);
	}
}

export class BrowserPlaybacks {
	readonly via = {
		ui: new PlaybackSurface(this, "ui"),
		api: new PlaybackSurface(this, "api"),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
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

	select(number: number) {
		return this.selectVia("ui", number);
	}

	fader(number: number, value: number) {
		return this.faderVia("ui", number, value);
	}

	expect(number: number) {
		return new PlaybackExpectation(this, validInteger(number, "Playback"));
	}

	async actionVia(route: RuntimeRoute, number: number, action: PlaybackButton) {
		number = validInteger(number, "Playback");
		if (route === "api") {
			const wireAction = action === PlaybackButton.GoBack ? "go-minus" : action;
			await this.api.playbackNumberAction(number, wireAction);
		} else {
			if (action === PlaybackButton.Release)
				throw new Error("Playback release has no default visible button");
			await this.desk.click(
				(await this.visibleCard(number)).getByRole("button", {
					name: playbackButtonLabel(action),
					exact: true,
				}),
			);
		}
	}

	async selectVia(route: RuntimeRoute, number: number) {
		if (route === "api") await this.api.playbackNumberAction(number, "select");
		else
			await this.desk.click(
				(await this.visibleCard(number)).getByRole("button", {
					name: /Playback representation/,
				}),
			);
		await this.expect(number).selected();
	}

	async faderVia(route: RuntimeRoute, number: number, value: number) {
		if (!Number.isFinite(value) || value < 0 || value > 100)
			throw new Error("Playback fader value must be between 0 and 100");
		if (route === "api")
			await this.api.playbackNumberAction(number, "master", {
				value: value / 100,
			});
		else await (await this.visibleCard(number)).getByRole("slider").fill(String(value));
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
					...(definition.buttons == null ? {} : { buttons: definition.buttons }),
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
		return this.api.request<PlaybackSnapshot>("GET", "/api/v2/playback-overview");
	}

	async runtime(number: number) {
		return (await this.snapshot()).active.find(
			(item) => item.playback_number === number,
		);
	}

	private async visibleCard(number: number): Promise<Locator> {
		const location = await playbackLocation(this.api, this.showId(), number);
		const card = this.page.locator(
			`.playback-fader-bank [data-playback-slot="${location.slot}"]:visible`,
		);
		await expect(card).toBeVisible();
		return card;
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
	};
	const label = labels[action];
	if (!label) throw new Error(`Playback ${action} has no visible button label`);
	return label;
}
