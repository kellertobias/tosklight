import { expect, type Page } from "@playwright/test";
import { HttpPlaybackTopologyTransport } from "../../../apps/light-desktop/src/api/PlaybackTopologyTransport";
import { HttpProgrammerCaptureModeTransport } from "../../../apps/light-desktop/src/api/ProgrammerCaptureModeTransport";
import { HttpProgrammerPreloadValuesTransport } from "../../../apps/light-desktop/src/api/ProgrammerPreloadValuesTransport";
import type { PlaybackAddress } from "../../../apps/light-desktop/src/features/playbackTopology/contracts";
import type { AttributeValue } from "../../../apps/light-desktop/src/api/types/playback";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import {
	clearPendingProgrammerPreload,
	enterProgrammerPreload,
	goProgrammerPreload,
	releaseProgrammerPreload,
} from "../programmer/programmerPreloadLifecycle";
import { copyPlaybackAssignment } from "./copyPlaybackAssignment";
import type { BrowserPlaybacks } from "./playbackScenario";
import { HttpPreloadScopeTransport } from "../../../apps/light-desktop/src/features/playbackPreloadScope/transport";
import { preloadScopeText } from "../../../apps/light-desktop/src/features/playbackPreloadScope/scopeText";
import type {
	PlaybackPreloadScope,
	PlaybackControlSource,
} from "../../../apps/light-desktop/src/api/generated/light-wire";

type PageRoute = "ui" | "api";
type PreloadRoute = "ui" | "api";

export interface PreloadCaptureConfiguration {
	programmer: boolean;
	playbackScope: PlaybackPreloadScope;
	programmerFade: number;
	cueFade: number;
}

export type PreloadCaptureMask = Pick<
	PreloadCaptureConfiguration,
	"programmer" | "playbackScope"
>;

export interface PendingPreloadExpectation {
	groupIds: string[];
	playbackActions: Array<[PlaybackAddress, string, PlaybackControlSource]>;
}

interface ProgrammerPreloadProjection {
	session_id: string;
	preload_group_pending: Record<string, unknown>;
	preload_group_active: Record<string, Record<string, { changed_at?: string }>>;
	preload_playback_pending: Array<{
		address: PlaybackAddress;
		action: string;
		control_source: PlaybackControlSource;
	}>;
}

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
		return new PageExpectation(this, validPage(number));
	}

	async selectVia(route: PageRoute, number: number) {
		number = validPage(number);
		if (route === "api") {
			const session = this.session();
			await this.api.request(
				"POST",
				`/api/v2/playback-surfaces/${encodeURIComponent(await this.currentSurface())}/page`,
				{ page: number },
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
		number = validPage(number);
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

	async map(intent: {
		sourceAddress: PlaybackAddress;
		targetAddress: PlaybackAddress;
	}) {
		await copyPlaybackAssignment(this.api, {
			surface: "api",
			showId: this.showId(),
			sourceAddress: intent.sourceAddress,
			targetAddress: intent.targetAddress,
		});
	}

	async current() {
		return (await this.runtimeDesk()).command_page;
	}
	private async currentSurface() {
		return (await this.runtimeDesk()).selected_surface_id;
	}
	private async runtimeDesk() {
		return (
			await this.api.request<{
				desk: { command_page: number; selected_surface_id: string };
			}>("GET", "/api/v2/playback-overview")
		).desk;
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

export class PreloadSettings {
	readonly expect = {
		mask: async (mask: PreloadCaptureMask) => {
			await expect
				.poll(async () => ({
					programmer: (await this.configuration()).preload_programmer_changes,
					playbackScope: (
						await this.scopeTransport().loadSnapshot({ showId: this.showId() })
					).scope,
				}))
				.toEqual(mask);
			await expect(
				this.page.getByRole("switch", { name: "Preload programmer changes" }),
			).toBeChecked({ checked: mask.programmer });
			const text = preloadScopeText(mask.playbackScope);
			await expect(
				this.page.getByLabel("Playback position ranges"),
			).toHaveValue(text.positions);
			await expect(
				this.page.getByRole("switch", { name: "All pages" }),
			).toBeChecked({ checked: text.allPages });
		},
	};
	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly showId: () => string,
	) {}
	async configure(mask: PreloadCaptureMask): Promise<void> {
		const control = this.page.getByRole("switch", {
			name: "Preload programmer changes",
		});
		if ((await control.isChecked()) !== mask.programmer)
			await this.desk.click(control.locator("..").locator(".ui-switch-track"));
		const text = preloadScopeText(mask.playbackScope);
		await this.page.getByLabel("Playback position ranges").fill(text.positions);
		const pages = this.page.getByRole("switch", { name: "All pages" });
		if ((await pages.isChecked()) !== text.allPages)
			await this.desk.click(pages.locator("..").locator(".ui-switch-track"));
		if (!text.allPages)
			await this.page.getByLabel("Eligible pages").fill(text.pages);
		await this.desk.click(
			this.page.getByRole("button", {
				name: "Save playback eligibility",
				exact: true,
			}),
		);
		await this.expect.mask(mask);
		await this.desk.click(
			this.page.getByRole("button", { name: "Outputs", exact: true }),
		);
		await this.desk.click(
			this.page.getByRole("button", { name: "Others", exact: true }),
		);
	}
	private configuration() {
		return this.api
			.request<{ configuration: { preload_programmer_changes: boolean } }>(
				"GET",
				"/api/v2/configuration",
			)
			.then((r) => r.configuration);
	}
	private scopeTransport() {
		if (!this.api.session)
			throw new Error("Preload settings require an API session");
		return new HttpPreloadScopeTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: this.api.session.token,
		});
	}
}

export class BrowserPreload {
	readonly via = {
		ui: new PreloadSurface(this, "ui"),
		api: new PreloadSurface(this, "api"),
	};
	readonly expect = {
		active: async () => expect.poll(() => this.active()).toBe(true),
		inactive: async () => expect.poll(() => this.active()).toBe(false),
		pendingPlaybackActions: async (actions: readonly string[]) =>
			expect(await this.pendingPlaybackActions()).toEqual(actions),
		pending: async (expected: PendingPreloadExpectation) =>
			expect.poll(() => this.pending()).toEqual(expected),
		pendingMatching: async (expected: unknown) =>
			expect.poll(() => this.pending()).toMatchObject(expected),
		atomicCommit: async (groupId: string, playbacks: PlaybackAddress[]) => {
			if (playbacks.length < 1)
				throw new Error("Atomic Preload commit requires a Playback");
			await expect
				.poll(async () => {
					const programmer = await this.programmer();
					const groupTimestamp =
						programmer.preload_group_active[groupId]?.intensity?.changed_at;
					const playbackTimestamps = await Promise.all(
						playbacks.map(async (identity) => {
							const runtime = await this.playbacks.runtime(identity);
							return runtimeActivationTimestamp(runtime);
						}),
					);
					const timestamps = [groupTimestamp, ...playbackTimestamps].map(
						normalizeCommitTimestamp,
					);
					return timestamps.every(Boolean) ? new Set(timestamps).size : 0;
				})
				.toBe(1);
		},
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly showId: () => string,
		private readonly playbacks: BrowserPlaybacks,
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

	async configure(configuration: PreloadCaptureConfiguration) {
		await this.api.request("POST", "/api/v2/configuration/update", {
			request_id: crypto.randomUUID(),
			patch: {
				programmer_fade_millis: configuration.programmerFade,
				sequence_master_fade_millis: configuration.cueFade,
				preload_programmer_changes: configuration.programmer,
			},
		});
		const transport = new HttpPreloadScopeTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: this.session().token,
		});
		const scope = { showId: this.showId() };
		const snapshot = await transport.loadSnapshot(scope);
		await transport.save(
			scope,
			snapshot.revision,
			configuration.playbackScope,
			crypto.randomUUID(),
		);
	}

	async openSettings(): Promise<PreloadSettings> {
		await this.desk.click(
			this.page.getByRole("button", { name: /Open show menu/ }),
		);
		await this.desk.click(
			this.page.getByRole("button", { name: "Enter Setup", exact: true }),
		);
		await this.desk.click(
			this.page.getByRole("button", { name: "Others", exact: true }),
		);
		await expect(
			this.page.getByRole("switch", {
				name: "Preload programmer changes",
			}),
		).toBeVisible();
		return new PreloadSettings(this.api, this.page, this.desk, this.showId);
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
		// Releasing the programmer's Preload data has no visible desk gesture: holding the Preload
		// button opens the pending inspector, and Shift with it clears the pending actions instead.
		if (route !== "api")
			throw new Error(
				"Releasing Preload programmer data has no visible desk gesture; use the api route",
			);
		await releaseProgrammerPreload(this.api, this.intent());
		await this.expect.inactive();
	}

	async setFixtureValue(input: {
		fixture: number;
		attribute: string;
		value: AttributeValue;
	}) {
		const patch = await this.api.patch();
		const fixtures = patch.fixtures.filter(
			(fixture) => fixture.fixture_number === valid(input.fixture, "Fixture"),
		);
		if (fixtures.length !== 1)
			throw new Error(
				`Fixture ${input.fixture} ${fixtures.length ? "is ambiguous" : "is not present in the active patch"}`,
			);
		await this.setFixtureValueById({
			fixtureId: fixtures[0].fixture_id,
			attribute: input.attribute,
			value: input.value,
		});
	}

	async setFixtureValueById(input: {
		fixtureId: string;
		attribute: string;
		value: AttributeValue;
	}) {
		const session = this.session();
		const scope = { showId: this.showId(), sessionId: session.session_id };
		const transport = new HttpProgrammerPreloadValuesTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: session.token,
			authenticatedSessionId: session.session_id,
		});
		const [values, captureMode] = await Promise.all([
			transport.loadSnapshot(scope),
			new HttpProgrammerCaptureModeTransport({
				baseUrl: this.api.baseUrl,
				sessionToken: session.token,
			}).loadSnapshot(scope),
		]);
		await transport.applyAction(scope, {
			requestId: crypto.randomUUID(),
			expectedPreloadRevision: values.projection.revision,
			expectedCaptureModeRevision: captureMode.projection.revision,
			action: {
				action: "set_fixture",
				fixtureId: input.fixtureId,
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
		}).loadSnapshot({ showId: this.showId() });
		return snapshot.projection.blind;
	}

	private async pendingPlaybackActions() {
		return (await this.programmer()).preload_playback_pending.map(
			(entry) => entry.action,
		);
	}

	private async pending(): Promise<PendingPreloadExpectation> {
		const programmer = await this.programmer();
		return {
			groupIds: Object.keys(programmer.preload_group_pending).sort(),
			playbackActions: pendingPlaybackTuples(
				programmer.preload_playback_pending,
			),
		};
	}

	private async programmer(): Promise<ProgrammerPreloadProjection> {
		const sessionId = this.session().session_id;
		const programmers = await this.api.request<ProgrammerPreloadProjection[]>(
			"GET",
			"/api/v2/programmers",
		);
		const programmer = programmers.find(
			(candidate) => candidate.session_id === sessionId,
		);
		if (!programmer) throw new Error("No Programmer projection is available");
		return programmer;
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

export function pendingPlaybackTuples(
	entries: ProgrammerPreloadProjection["preload_playback_pending"],
): PendingPreloadExpectation["playbackActions"] {
	return entries.map((entry) => [
		{ page: entry.address.page, position: entry.address.position },
		entry.action,
		{ ...entry.control_source },
	]);
}

function valid(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 1)
		throw new Error(`${label} numbers start at 1`);
	return value;
}

function normalizeCommitTimestamp(value: unknown): number | null {
	if (typeof value !== "string" && typeof value !== "number") return null;
	const millis = typeof value === "number" ? value : Date.parse(value);
	return Number.isFinite(millis) ? millis : null;
}

function validPage(number: number) {
	if (!Number.isSafeInteger(number) || number < 1 || number > 99)
		throw new Error("Playback Pages are 1–99");
	return number;
}

function runtimeActivationTimestamp(value: unknown): unknown {
	if (!value || typeof value !== "object") return null;
	if ("activated_at" in value) return value.activated_at;
	return "runtime" in value ? runtimeActivationTimestamp(value.runtime) : null;
}
