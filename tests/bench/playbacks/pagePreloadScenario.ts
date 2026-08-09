import { expect, type Page } from "@playwright/test";
import { HttpPlaybackTopologyTransport } from "../../../apps/light-desktop/src/api/PlaybackTopologyTransport";
import { HttpProgrammerCaptureModeTransport } from "../../../apps/light-desktop/src/api/ProgrammerCaptureModeTransport";
import { HttpProgrammerPreloadValuesTransport } from "../../../apps/light-desktop/src/api/ProgrammerPreloadValuesTransport";
import type { AttributeValue } from "../../../apps/light-desktop/src/api/types/playback";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import {
	clearPendingProgrammerPreload,
	enterProgrammerPreload,
	goProgrammerPreload,
	releaseProgrammerPreload,
} from "../programmer/programmerPreloadLifecycle";
import { mapExistingPlaybackToSlot } from "./mapExistingPlaybackToSlot";
import type { BrowserPlaybacks } from "./playbackScenario";
import type { VirtualPlaybackIdentity } from "./virtualPlaybackScenario";

type PageRoute = "ui" | "api";
type PreloadRoute = "ui" | "api";

export interface PreloadCaptureConfiguration {
	programmer: boolean;
	physicalPlaybacks: boolean;
	virtualPlaybacks: boolean;
	programmerFade: number;
	cueFade: number;
}

export type PreloadCaptureMask = Pick<
	PreloadCaptureConfiguration,
	"programmer" | "physicalPlaybacks" | "virtualPlaybacks"
>;

export interface PendingPreloadExpectation {
	groupIds: string[];
	playbackActions: Array<[number | VirtualPlaybackIdentity, string, string]>;
}

interface ProgrammerPreloadProjection {
	session_id: string;
	preload_group_pending: Record<string, unknown>;
	preload_group_active: Record<string, Record<string, { changed_at?: string }>>;
	preload_playback_pending: Array<{
		playback_number: number;
		page?: number;
		action: string;
		surface: string;
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

export class PreloadSettings {
	readonly expect = {
		mask: async (mask: PreloadCaptureMask) => {
			const expected = [
				mask.programmer,
				mask.physicalPlaybacks,
				mask.virtualPlaybacks,
			];
			await expect
				.poll(async () => {
					const configuration = await this.configuration();
					return [
						configuration.preload_programmer_changes,
						configuration.preload_physical_playback_actions,
						configuration.preload_virtual_playback_actions,
					];
				})
				.toEqual(expected);
			for (const [index, label] of PRELOAD_MASK_LABELS.entries())
				await expect(
					this.page.getByRole("switch", { name: label }),
				).toBeChecked({
					checked: expected[index],
				});
		},
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
	) {}

	async configure(mask: PreloadCaptureMask): Promise<void> {
		const desired = [
			mask.programmer,
			mask.physicalPlaybacks,
			mask.virtualPlaybacks,
		];
		for (const [index, label] of PRELOAD_MASK_LABELS.entries()) {
			const control = this.page.getByRole("switch", { name: label });
			if ((await control.isChecked()) !== desired[index])
				await this.desk.click(
					control.locator("..").locator(".ui-switch-track"),
				);
		}
		await this.desk.click(
			this.page.getByRole("button", { name: "Save changes", exact: true }),
		);
		await expect
			.poll(async () => {
				const configuration = await this.configuration();
				return [
					configuration.preload_programmer_changes,
					configuration.preload_physical_playback_actions,
					configuration.preload_virtual_playback_actions,
				];
			})
			.toEqual(desired);
		await this.desk.click(
			this.page.getByRole("button", { name: "Outputs", exact: true }),
		);
		await this.desk.click(
			this.page.getByRole("button", { name: "Others", exact: true }),
		);
	}

	private configuration() {
		return this.api
			.request<{
				configuration: {
					preload_programmer_changes: boolean;
					preload_physical_playback_actions: boolean;
					preload_virtual_playback_actions: boolean;
				};
			}>("GET", "/api/v2/configuration")
			.then((response) => response.configuration);
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
		atomicCommit: async (
			groupId: string,
			playbacks: Array<number | VirtualPlaybackIdentity>,
		) => {
			if (playbacks.length < 1)
				throw new Error("Atomic Preload commit requires a Playback");
			await expect
				.poll(async () => {
					const programmer = await this.programmer();
					const groupTimestamp =
						programmer.preload_group_active[groupId]?.intensity?.changed_at;
					const playbackTimestamps = await Promise.all(
						playbacks.map(async (identity) => {
							const runtime =
								typeof identity === "number"
									? await this.playbacks.runtime(identity)
									: await this.virtualRuntime(identity);
							return runtime?.activated_at ?? runtime?.runtime?.activated_at;
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
		const current = await this.api.request<{
			configuration: Record<string, unknown>;
		}>("GET", "/api/v2/configuration");
		await this.api.request("PUT", "/api/v2/configuration", {
			...current.configuration,
			programmer_fade_millis: configuration.programmerFade,
			sequence_master_fade_millis: configuration.cueFade,
			preload_programmer_changes: configuration.programmer,
			preload_physical_playback_actions: configuration.physicalPlaybacks,
			preload_virtual_playback_actions: configuration.virtualPlaybacks,
		});
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
				name: PRELOAD_MASK_LABELS[0],
			}),
		).toBeVisible();
		return new PreloadSettings(this.api, this.page, this.desk);
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
		const scope = { showId: this.showId(), userId: session.user.id };
		const transport = new HttpProgrammerPreloadValuesTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: session.token,
			authenticatedUserId: session.user.id,
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
		}).loadSnapshot({ showId: this.showId(), userId: session.user.id });
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
			playbackActions: programmer.preload_playback_pending.map((entry) => [
				entry.surface === "virtual" && entry.page != null
					? {
							page: entry.page,
							playbackNumber: entry.playback_number,
						}
					: entry.playback_number,
				entry.action,
				entry.surface,
			]),
		};
	}

	private async virtualRuntime(
		identity: VirtualPlaybackIdentity,
	): Promise<any> {
		const session = this.session();
		const snapshot = await this.api.request<any>(
			"POST",
			"/api/v2/playback-runtime/snapshot",
			{
				identities: [
					{
						kind: "virtual",
						page: identity.page,
						playback_number: identity.playbackNumber,
					},
				],
			},
			true,
			undefined,
			{ showId: this.showId(), deskId: session.desk.id },
		);
		return snapshot.projections?.[0];
	}

	private async programmer(): Promise<ProgrammerPreloadProjection> {
		const sessionId = this.session().session_id;
		const programmers = await this.api.request<ProgrammerPreloadProjection[]>(
			"GET",
			"/api/v2/programmers",
		);
		const programmer =
			programmers.find((candidate) => candidate.session_id === sessionId) ??
			programmers[0];
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

const PRELOAD_MASK_LABELS = [
	"Preload programmer changes",
	"Preload physical playback actions",
	"Preload virtual playback actions",
] as const;

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
