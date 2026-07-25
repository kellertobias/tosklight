import { expect, type Page } from "@playwright/test";
import { HttpCueTransferTransport } from "../../../src/api/CueTransferTransport";
import type { CueMoveCopyChoice } from "../../../src/api/generated/light-wire";
import type {
	Cue,
	CueList,
	PlaybackDefinition,
} from "../../../src/api/types/playback";
import type { ApiDriver } from "../core/api";
import type { BrowserCommands } from "../command-selection/commandScenario";
import type { DeskDriver } from "../core/desk";

export enum CueRecordMode {
	Overwrite = "overwrite",
	Merge = "merge",
	Subtract = "subtract",
}

export interface CueTiming {
	fade?: string;
}

type CommandRoute = "ui" | "api";

class RecordingSurface {
	constructor(
		private readonly owner: BrowserRecording,
		private readonly route: CommandRoute,
	) {}

	playback(slot: number) {
		return this.owner.playbackVia(this.route, slot);
	}

	cue(intent: RecordCueIntent) {
		return this.owner.cueVia(this.route, intent);
	}
}

export interface RecordCueIntent {
	playback: number;
	cue: number;
	mode?: CueRecordMode;
	timing?: CueTiming;
}

export class BrowserRecording {
	readonly via = {
		ui: new RecordingSurface(this, "ui"),
		api: new RecordingSurface(this, "api"),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly commands: BrowserCommands,
		private readonly showId: () => string,
	) {}

	playback(slot: number) {
		return this.playbackVia("ui", slot);
	}

	cue(intent: RecordCueIntent) {
		return this.cueVia("ui", intent);
	}

	async append(playback: number) {
		const location = await playbackLocation(
			this.api,
			this.showId(),
			validInteger(playback, "Playback"),
		);
		const before = await this.cueListForPlayback(playback);
		const target = this.page.getByRole("button", {
			name: `Playback representation page ${location.page} playback ${location.slot}`,
			exact: true,
		});
		if (!(await target.isVisible())) {
			await this.desk.click(this.page.locator(".mode-toggle"));
			await expect(target).toBeVisible();
		}
		await this.desk.click(
			this.page.locator(".global-store-button:visible").first(),
		);
		await this.desk.click(target);
		await expect
			.poll(async () => (await this.cueListForPlayback(playback)).revision)
			.toBeGreaterThan(before.revision);
	}

	async cueOnly(checked: boolean) {
		await this.ensureCommandSurface();
		await this.desk.recordStep(
			"RECORD SETTINGS",
			`${checked ? "Enable" : "Disable"} Cue only through the visible Record settings.`,
		);
		const record = this.page.getByRole("button", {
			name: /REC(?: ARMED)?/,
			exact: true,
		});
		await record.hover();
		await this.page.mouse.down();
		await this.page.waitForTimeout(700);
		await this.page.mouse.up();
		const dialog = this.page.locator(".store-settings-modal");
		await expect(dialog).toBeVisible();
		const cueOnly = dialog.getByLabel("Cue only");
		if ((await cueOnly.isChecked()) !== checked)
			await this.desk.click(
				dialog.locator("label").filter({ hasText: "Cue only" }),
			);
		if (checked) await expect(cueOnly).toBeChecked();
		else await expect(cueOnly).not.toBeChecked();
		await this.desk.click(
			dialog.getByRole("button", { name: "Done", exact: true }),
		);
		await expect(dialog).toBeHidden();
		await this.page.waitForTimeout(1_000);
	}

	async playbackVia(route: CommandRoute, slot: number) {
		validInteger(slot, "Playback slot");
		if (route === "api")
			throw new Error(
				"Creating an assigned Playback has no truthful API command; use the visible UI route",
			);
		const before = await this.pageObject(1);
		const target = this.page.getByRole("button", {
			name: `Playback representation page 1 playback ${slot}`,
			exact: true,
		});
		if (!(await target.isVisible())) {
			await this.desk.click(this.page.locator(".mode-toggle"));
			await expect(target).toBeVisible();
		}
		await this.desk.click(
			this.page.locator(".global-store-button:visible").first(),
		);
		await this.desk.click(target);
		let playbackNumber: number | undefined;
		await expect
			.poll(async () => {
				const page = await this.pageObject(1);
				playbackNumber = page?.body.slots[String(slot)];
				return {
					revision: page?.revision ?? 0,
					playbackNumber,
				};
			})
			.toMatchObject({
				revision: expect.any(Number),
				playbackNumber: expect.any(Number),
			});
		expect((await this.pageObject(1))?.revision ?? 0).toBeGreaterThan(
			before?.revision ?? 0,
		);
		return playbackNumber!;
	}

	async cueVia(route: CommandRoute, intent: RecordCueIntent) {
		if (route === "ui") await this.ensureCommandSurface();
		const address = await this.address(intent.playback);
		const before = await this.cueListForPlayback(intent.playback);
		const command = [
			"RECORD",
			intent.mode === CueRecordMode.Merge
				? "+"
				: intent.mode === CueRecordMode.Subtract
					? "-"
					: "",
			address,
			"CUE",
			formatCueNumber(intent.cue),
			intent.timing?.fade ? `TIME ${intent.timing.fade}` : "",
		]
			.filter(Boolean)
			.join(" ");
		await this.commands.via[route].execute(command);
		await expect
			.poll(
				async () => (await this.cueListForPlayback(intent.playback)).revision,
			)
			.toBeGreaterThan(before.revision);
	}

	private async address(playback: number) {
		const location = await playbackLocation(this.api, this.showId(), playback);
		return `SET ${location.page} . ${location.slot}`;
	}

	private cueListForPlayback(playback: number) {
		return cueListForPlayback(this.api, this.showId(), playback);
	}

	private pageObject(number: number) {
		return this.api.showObject<any>(
			this.showId(),
			"playback_page",
			String(number),
		);
	}

	private async ensureCommandSurface() {
		const set = this.page.locator(
			'.programmer-number-block [data-keypad-key="SET"]:visible',
		);
		if (!(await set.isVisible()))
			await this.desk.click(this.page.locator(".mode-toggle"));
		await expect(set).toBeVisible();
	}
}

class CueSurface {
	constructor(
		private readonly owner: BrowserCues,
		private readonly route: CommandRoute,
	) {}

	update(playback: number, cue: number) {
		return this.owner.updateVia(this.route, playback, cue);
	}

	delete(playback: number, cue: number) {
		return this.owner.deleteVia(this.route, playback, cue);
	}

	move(playback: number, cue: number, destination: number) {
		return this.owner.transferVia(
			this.route,
			"MOVE",
			playback,
			cue,
			destination,
		);
	}

	copy(playback: number, cue: number, destination: number) {
		return this.owner.transferVia(
			this.route,
			"COPY",
			playback,
			cue,
			destination,
		);
	}

	goto(playback: number, cue: number) {
		return this.owner.gotoVia(this.route, playback, cue);
	}

	load(playback: number, cue: number) {
		return this.owner.loadVia(this.route, playback, cue);
	}

	select(playback: number, cue: number) {
		return this.owner.selectVia(this.route, playback, cue);
	}
}

class CueExpectation {
	constructor(
		private readonly owner: BrowserCues,
		private readonly playback: number,
		private readonly cue: number,
	) {}

	async present() {
		expect(await this.owner.cue(this.playback, this.cue)).toBeDefined();
	}

	async absent() {
		expect(await this.owner.cue(this.playback, this.cue)).toBeUndefined();
	}

	async metadata(
		expected: Partial<Pick<Cue, "name" | "fade_millis" | "delay_millis">>,
	) {
		expect(await this.owner.cue(this.playback, this.cue)).toMatchObject(
			expected,
		);
	}
}

export class BrowserCues {
	readonly via = {
		ui: new CueSurface(this, "ui"),
		api: new CueSurface(this, "api"),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly commands: BrowserCommands,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly showId: () => string,
	) {}

	update(playback: number, cue: number) {
		return this.updateVia("ui", playback, cue);
	}

	delete(playback: number, cue: number) {
		return this.deleteVia("ui", playback, cue);
	}

	move(playback: number, cue: number, destination: number) {
		return this.transferVia("ui", "MOVE", playback, cue, destination);
	}

	copy(playback: number, cue: number, destination: number) {
		return this.transferVia("ui", "COPY", playback, cue, destination);
	}

	goto(playback: number, cue: number) {
		return this.gotoVia("ui", playback, cue);
	}

	load(playback: number, cue: number) {
		return this.loadVia("ui", playback, cue);
	}

	select(playback: number, cue: number) {
		return this.selectVia("ui", playback, cue);
	}

	expect(playback: number, cue: number) {
		return new CueExpectation(this, playback, cue);
	}

	async updateVia(route: CommandRoute, playback: number, cue: number) {
		await this.mutate(
			route,
			playback,
			`UPDATE ${await this.address(playback)} CUE ${formatCueNumber(cue)}`,
		);
	}

	async deleteVia(route: CommandRoute, playback: number, cue: number) {
		await this.mutate(
			route,
			playback,
			`DELETE ${await this.address(playback)} CUE ${formatCueNumber(cue)}`,
		);
	}

	async transferVia(
		route: CommandRoute,
		operation: "MOVE" | "COPY",
		playback: number,
		cue: number,
		destination: number,
	) {
		const address = await this.address(playback);
		const command = `${operation} ${address} CUE ${formatCueNumber(cue)} AT ${address} CUE ${formatCueNumber(destination)}`;
		const before = await cueListForPlayback(this.api, this.showId(), playback);
		if (route === "ui") {
			await this.ensureCommandSurface();
			await this.commands.via.ui.execute(command);
			await this.desk.click(
				this.page.getByRole("button", {
					name: `Plain ${operation === "COPY" ? "Copy" : "Move"}`,
					exact: true,
				}),
			);
		} else {
			const response = await this.api.executeCommandLineRaw(command);
			if (response.outcome !== "choice_required")
				throw new Error(`Cue ${operation} did not produce its typed choice`);
			const choice = response.pending_choice as CueMoveCopyChoice;
			const session = this.session();
			await new HttpCueTransferTransport({
				baseUrl: this.api.baseUrl,
				sessionToken: session.token,
			}).apply(this.showId(), choice.show_revision, {
				requestId: crypto.randomUUID(),
				choiceId: choice.choice_id,
				mode: "plain",
				expectedCommandLineRevision: response.command_line.revision,
			});
		}
		await expect
			.poll(
				async () =>
					(await cueListForPlayback(this.api, this.showId(), playback))
						.revision,
			)
			.toBeGreaterThan(before.revision);
	}

	async gotoVia(route: CommandRoute, playback: number, cue: number) {
		return this.commands.via[route].execute(
			`CUE ${await this.address(playback)} CUE ${formatCueNumber(cue)}`,
		);
	}

	async loadVia(route: CommandRoute, playback: number, cue: number) {
		return this.commands.via[route].execute(
			`CUE CUE ${await this.address(playback)} CUE ${formatCueNumber(cue)}`,
		);
	}

	async selectVia(route: CommandRoute, playback: number, cue: number) {
		return this.commands.via[route].execute(
			`${await this.address(playback)} CUE ${formatCueNumber(cue)}`,
		);
	}

	async cue(playback: number, cue: number) {
		const list = await cueListForPlayback(this.api, this.showId(), playback);
		return list.body.cues.find((candidate) => candidate.number === cue);
	}

	private async mutate(route: CommandRoute, playback: number, command: string) {
		const before = await cueListForPlayback(this.api, this.showId(), playback);
		if (route === "ui") await this.ensureCommandSurface();
		await this.commands.via[route].execute(command);
		await expect
			.poll(
				async () =>
					(await cueListForPlayback(this.api, this.showId(), playback))
						.revision,
			)
			.toBeGreaterThan(before.revision);
	}

	private async address(playback: number) {
		const location = await playbackLocation(this.api, this.showId(), playback);
		return `SET ${location.page} . ${location.slot}`;
	}

	private async ensureCommandSurface() {
		const set = this.page.locator(
			'.programmer-number-block [data-keypad-key="SET"]:visible',
		);
		if (!(await set.isVisible()))
			await this.desk.click(this.page.locator(".mode-toggle"));
		await expect(set).toBeVisible();
	}

	private session() {
		if (!this.api.session)
			throw new Error("Cue helper requires an API session");
		return this.api.session;
	}
}

export async function playbackLocation(
	api: ApiDriver,
	showId: string,
	number: number,
) {
	validInteger(number, "Playback");
	const pages = await api.showObjects<any>(showId, "playback_page");
	for (const page of pages)
		for (const [slot, playback] of Object.entries(page.body.slots))
			if (playback === number)
				return { page: page.body.number as number, slot: Number(slot) };
	throw new Error(`Playback ${number} is not mapped to a page`);
}

export async function cueListForPlayback(
	api: ApiDriver,
	showId: string,
	number: number,
) {
	const playback = await api.showObject<PlaybackDefinition>(
		showId,
		"playback",
		String(number),
	);
	if (!playback) throw new Error(`Playback ${number} is absent`);
	if (playback.body.target.type !== "cue_list")
		throw new Error(`Playback ${number} does not target a Cuelist`);
	const cueList = await api.showObject<CueList>(
		showId,
		"cue_list",
		playback.body.target.cue_list_id,
	);
	if (!cueList) throw new Error(`Playback ${number}'s Cuelist is absent`);
	return cueList;
}

export function formatCueNumber(number: number) {
	if (!Number.isFinite(number) || number <= 0)
		throw new Error("Cue numbers must be greater than zero");
	return String(number);
}

export function validInteger(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 1)
		throw new Error(`${label} numbers start at 1`);
	return value;
}
