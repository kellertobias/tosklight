import { expect, type Page } from "@playwright/test";
import { HttpPlaybackTopologyTransport } from "../../../src/api/PlaybackTopologyTransport";
import { HttpProgrammerCaptureModeTransport } from "../../../src/api/ProgrammerCaptureModeTransport";
import { HttpProgrammerPreloadValuesTransport } from "../../../src/api/ProgrammerPreloadValuesTransport";
import type { AttributeValue } from "../../../src/api/types/playback";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import {
	clearPendingProgrammerPreload,
	enterProgrammerPreload,
	goProgrammerPreload,
	releaseProgrammerPreload,
} from "../programmer/programmerPreloadLifecycle";
import { mapExistingPlaybackToSlot } from "./mapExistingPlaybackToSlot";

type PageRoute = "ui" | "api";
type PreloadRoute = "ui" | "api";

class PageSurface {
	constructor(
		private readonly owner: BrowserPages,
		private readonly route: PageRoute,
	) {}

	select(number: number) {
		return this.owner.selectVia(this.route, number);
	}

	next() {
		return this.owner.nextVia(this.route);
	}

	previous() {
		return this.owner.previousVia(this.route);
	}
}

class PageExpectation {
	constructor(
		private readonly owner: BrowserPages,
		private readonly number: number,
	) {}

	async present() {
		expect(await this.owner.object(this.number)).not.toBeNull();
	}

	async named(name: string) {
		expect((await this.owner.requiredObject(this.number)).body.name).toBe(name);
	}

	async selected() {
		await expect.poll(() => this.owner.current()).toBe(this.number);
	}
}

export class BrowserPages {
	readonly via = {
		ui: new PageSurface(this, "ui"),
		api: new PageSurface(this, "api"),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly showId: () => string,
	) {}

	select(number: number) {
		return this.selectVia("ui", number);
	}

	next() {
		return this.nextVia("ui");
	}

	previous() {
		return this.previousVia("ui");
	}

	expect(number: number) {
		return new PageExpectation(this, valid(number, "Playback Page"));
	}

	async selectVia(route: PageRoute, number: number) {
		number = valid(number, "Playback Page");
		if (route === "api") {
			const session = this.session();
			await this.api.request(
				"POST",
				`/api/v2/control-desks/${session.desk.id}/actions`,
				{
					request_id: crypto.randomUUID(),
					action: { type: "set_page", page: number, existing_only: true },
				},
				true,
				undefined,
				{ showId: this.showId(), deskId: session.desk.id },
			);
		} else {
			const softwareControl = this.page.locator(".playback-page-current");
			if (await softwareControl.count()) await this.desk.click(softwareControl);
			else
				await this.desk.click(
					this.page
						.locator(".hardware-control-summary")
						.getByRole("button", { name: /^Page \d+$/ }),
				);
			const dialog = this.page.locator(".playback-page-modal");
			await expect(dialog).toBeVisible();
			const object = await this.requiredObject(number);
			await this.desk.click(
				dialog.getByRole("button").filter({ hasText: object.body.name }),
			);
		}
		await this.expect(number).selected();
	}

	async nextVia(route: PageRoute) {
		return this.selectVia(route, (await this.current()) + 1);
	}

	async previousVia(route: PageRoute) {
		return this.selectVia(route, Math.max(1, (await this.current()) - 1));
	}

	async create(number: number) {
		number = valid(number, "Playback Page");
		await this.topology({
			type: "create_page",
			page: number,
			expectedPageRevision: 0,
			expectedPageObjectId: null,
		});
		await this.expect(number).present();
	}

	async rename(number: number, name: string) {
		const page = await this.requiredObject(number);
		await this.topology({
			type: "rename_page",
			page: number,
			name,
			expectedPageRevision: page.revision,
			expectedPageObjectId: page.id,
		});
		await this.expect(number).named(name);
	}

	async map(intent: { page: number; slot: number; playback: number }) {
		await mapExistingPlaybackToSlot(this.api, {
			surface: "api",
			showId: this.showId(),
			page: valid(intent.page, "Playback Page"),
			slot: valid(intent.slot, "Playback slot"),
			playbackNumber: valid(intent.playback, "Playback"),
		});
	}

	async current() {
		return (
			await this.api.request<{ active_page: number }>(
				"GET",
				"/api/v2/playback-overview",
			)
		).active_page;
	}

	object(number: number) {
		return this.api.showObject<any>(
			this.showId(),
			"playback_page",
			String(number),
		);
	}

	async requiredObject(number: number) {
		const page = await this.object(number);
		if (!page) throw new Error(`Playback Page ${number} is absent`);
		return page;
	}

	private async topology(action: any) {
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
			action,
		});
	}

	private session() {
		if (!this.api.session)
			throw new Error("Page helper requires an API session");
		return this.api.session;
	}
}

class PreloadSurface {
	constructor(
		private readonly owner: BrowserPreload,
		private readonly route: PreloadRoute,
	) {}

	start() {
		return this.owner.startVia(this.route);
	}

	commit() {
		return this.owner.commitVia(this.route);
	}

	clear() {
		return this.owner.clearVia(this.route);
	}

	release() {
		return this.owner.releaseVia(this.route);
	}
}

export class BrowserPreload {
	readonly via = {
		ui: new PreloadSurface(this, "ui"),
		api: new PreloadSurface(this, "api"),
	};
	readonly expect = {
		active: async () => expect(await this.active()).toBe(true),
		inactive: async () => expect(await this.active()).toBe(false),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly showId: () => string,
	) {}

	start() {
		return this.startVia("ui");
	}

	commit() {
		return this.commitVia("ui");
	}

	clear() {
		return this.clearVia("api");
	}

	async startVia(route: PreloadRoute) {
		if (route === "api") await enterProgrammerPreload(this.api, this.intent());
		else
			await this.desk.click(
				this.page.locator(".preload-button:visible").first(),
			);
		await this.expect.active();
	}

	async commitVia(route: PreloadRoute) {
		if (route === "api") await goProgrammerPreload(this.api, this.intent());
		else
			await this.desk.click(
				this.page.getByRole("button", { name: /^PRELOAD GO\b/ }),
			);
		if (route === "api") await this.expect.inactive();
		else await this.expect.active();
	}

	async clearVia(route: PreloadRoute) {
		if (route === "ui")
			throw new Error(
				"Visible Preload clear is a hold gesture; use release() or API clear",
			);
		await clearPendingProgrammerPreload(this.api, this.intent());
	}

	async release() {
		await this.releaseVia("api");
	}

	async releaseVia(route: PreloadRoute) {
		if (route === "api")
			await releaseProgrammerPreload(this.api, this.intent());
		else {
			const preload = this.page.locator(".preload-button:visible").first();
			await preload.hover();
			await this.page.mouse.down();
			await this.page.waitForTimeout(700);
			await this.page.mouse.up();
		}
		await this.expect.inactive();
	}

	async setFixtureValue(input: {
		fixture: number;
		attribute: string;
		value: AttributeValue;
	}) {
		const session = this.session();
		const scope = { showId: this.showId(), userId: session.user.id };
		const transport = new HttpProgrammerPreloadValuesTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: session.token,
			authenticatedUserId: session.user.id,
		});
		const [patch, values, captureMode] = await Promise.all([
			this.api.patch(),
			transport.loadSnapshot(scope),
			new HttpProgrammerCaptureModeTransport({
				baseUrl: this.api.baseUrl,
				sessionToken: session.token,
			}).loadSnapshot(scope),
		]);
		const fixtures = patch.fixtures.filter(
			(fixture) => fixture.fixture_number === valid(input.fixture, "Fixture"),
		);
		if (fixtures.length !== 1)
			throw new Error(
				`Fixture ${input.fixture} ${fixtures.length ? "is ambiguous" : "is not present in the active patch"}`,
			);
		await transport.applyAction(scope, {
			requestId: crypto.randomUUID(),
			expectedPreloadRevision: values.projection.revision,
			expectedCaptureModeRevision: captureMode.projection.revision,
			action: {
				action: "set_fixture",
				fixtureId: fixtures[0].fixture_id,
				attribute: input.attribute,
				value: input.value,
				timing: { fade: false, fadeMillis: null, delayMillis: null },
			},
		});
	}

	private async active() {
		const session = this.session();
		const snapshot = await new HttpProgrammerCaptureModeTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: session.token,
		}).loadSnapshot({ showId: this.showId(), userId: session.user.id });
		return (
			snapshot.projection.blind && snapshot.projection.preloadCaptureProgrammer
		);
	}

	private intent() {
		return { surface: "api" as const, showId: this.showId() };
	}

	private session() {
		if (!this.api.session)
			throw new Error("Preload helper requires an API session");
		return this.api.session;
	}
}

function valid(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 1)
		throw new Error(`${label} numbers start at 1`);
	return value;
}
