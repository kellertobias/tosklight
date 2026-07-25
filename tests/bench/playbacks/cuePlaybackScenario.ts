import { expect, type Page } from "@playwright/test";
import { HttpCueTransferTransport } from "../../../apps/light-desktop/src/api/CueTransferTransport";
import type { CueMoveCopyChoice } from "../../../apps/light-desktop/src/api/generated/light-wire";
import { HttpPlaybackTopologyTransport } from "../../../apps/light-desktop/src/api/PlaybackTopologyTransport";
import type {
	Cue,
	CueList,
	PlaybackDefinition,
} from "../../../apps/light-desktop/src/api/types/playback";
import type { BrowserCommands } from "../command-selection/commandScenario";
import type { ApiDriver } from "../core/api";
import { type ClockDuration, parseClockDuration } from "../core/clockScenario";
import type { DeskDriver } from "../core/desk";

export enum CueRecordMode {
	Overwrite = "overwrite",
	Merge = "merge",
	Subtract = "subtract",
}

export interface CueTiming {
	fade?: string;
}

export interface CueListConfiguration {
	priority?: number;
}

type CommandRoute = "ui" | "api";
type CueTriggerLabel = "GO" | "FOLLOW" | "TIME";

export interface CueEditorValues {
	name?: string;
	fade?: string;
	delay?: string;
	trigger?: CueTriggerLabel;
	triggerTime?: string;
}

export interface CueListSettingsValues {
	mode?: "Sequence" | "Chaser";
	priority?: number;
	intensityPriority?: "HTP" | "LTP";
	wrap?: "Off" | "Tracking" | "Reset";
	restart?: "First Cue" | "Continue Current Cue";
	forceCueTiming?: boolean;
	disableCueTiming?: boolean;
	speedMultiplier?: number;
	chaserXfade?: number;
}

class CueListSettings {
	constructor(
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly cueListName: string,
	) {}

	async expectDefaults() {
		const dialog = this.dialog();
		await expect(dialog).toContainText(this.cueListName);
		await expect(
			dialog.getByRole("button", { name: /Mode\s*\(Sequence\)/ }),
		).toBeVisible();
		await expect(this.selectField("Intensity priority mode")).toContainText(
			"HTP",
		);
		await expect(this.selectField("Wrap Around")).toContainText("Off");
		await expect(this.selectField("Restart mode")).toContainText("First Cue");
		await expect(dialog.getByLabel("Force Cue Timing")).not.toBeChecked();
		await expect(dialog.getByLabel("Disable Cue Timing")).not.toBeChecked();
	}

	async configure(values: CueListSettingsValues) {
		const dialog = this.dialog();
		await this.desk.recordStep(
			"CONFIGURE CUELIST SETTINGS",
			"Configure Sequence or Chaser behavior through the visible settings dialog.",
		);
		if (values.mode != null) {
			await dialog
				.getByRole("button", { name: /Mode\s*\((Sequence|Chaser)\)/ })
				.click();
			await dialog
				.getByRole("menuitemradio", { name: values.mode, exact: true })
				.click();
		}
		if (values.priority != null)
			await dialog.getByLabel("Numeric priority").fill(String(values.priority));
		if (values.intensityPriority != null)
			await this.choose("Intensity priority mode", values.intensityPriority);
		if (values.wrap != null) await this.choose("Wrap Around", values.wrap);
		if (values.restart != null)
			await this.choose("Restart mode", values.restart);
		if (values.forceCueTiming != null)
			await this.setSwitch("Force Cue Timing", values.forceCueTiming);
		if (values.disableCueTiming != null)
			await this.setSwitch("Disable Cue Timing", values.disableCueTiming);
		if (values.speedMultiplier != null)
			await dialog
				.getByLabel("Speed multiplier")
				.fill(String(values.speedMultiplier));
		if (values.chaserXfade != null) {
			const xfade = dialog.getByRole("slider", { name: "Chaser X-fade" });
			await expect(xfade).toHaveAttribute("min", "0");
			await expect(xfade).toHaveAttribute("max", "100");
			await xfade.evaluate((input: HTMLInputElement, value) => {
				input.value = String(value);
				input.dispatchEvent(new Event("input", { bubbles: true }));
				input.dispatchEvent(new Event("change", { bubbles: true }));
			}, values.chaserXfade);
		}
	}

	async save() {
		await this.desk.click(
			this.dialog().getByRole("button", { name: "Save", exact: true }),
		);
		await expect(this.dialog()).toBeHidden();
	}

	async close() {
		await this.desk.click(
			this.dialog().getByRole("button", {
				name: "Close Cuelist Settings",
				exact: true,
			}),
		);
		await expect(this.dialog()).toBeHidden();
	}

	private dialog() {
		return this.page.getByRole("dialog", { name: "Cuelist Settings" });
	}

	private selectField(label: string) {
		return this.dialog()
			.locator(".ui-form-field")
			.filter({ hasText: label })
			.getByRole("button")
			.first();
	}

	private async choose(label: string, next: string) {
		await this.selectField(label).click();
		await this.page.getByRole("option", { name: next, exact: true }).click();
	}

	private async setSwitch(label: string, checked: boolean) {
		const input = this.dialog().getByLabel(label, { exact: true });
		if ((await input.isChecked()) !== checked)
			await input
				.locator("xpath=ancestor::label[contains(@class, 'ui-switch-control')]")
				.click();
	}
}

class CueEditorExpectation {
	constructor(private readonly owner: CueEditor) {}

	async structure() {
		await expect(this.owner.window()).toBeVisible();
		await expect(this.owner.page.locator(".cue-table thead th")).toHaveText([
			"Preview",
			"No.",
			"Name",
			"Trigger",
			"Fade",
		]);
		await expect(this.owner.page.locator(".cue-table thead")).not.toContainText(
			"Status",
		);
		await expect(
			this.owner.page.locator(".cue-settings-compact-fallback"),
		).toBeHidden();
	}

	async selected(cue: number, values: CueEditorValues = {}) {
		const row = this.owner.row(cue);
		await expect(row).toHaveClass(/selected/);
		await expect(
			this.owner.page.getByText(`Selected Cue · ${formatCueNumber(cue)}`, {
				exact: true,
			}),
		).toBeVisible();
		if (values.name != null)
			await expect(this.owner.field("Title")).toHaveValue(values.name);
		if (values.fade != null)
			await expect(this.owner.field("Fade")).toHaveValue(values.fade);
		if (values.delay != null)
			await expect(this.owner.field("Delay")).toHaveValue(values.delay);
		if (values.triggerTime != null)
			await expect(this.owner.field("Trigger time")).toHaveValue(
				values.triggerTime,
			);
	}
}

export class CueEditor {
	readonly expect = new CueEditorExpectation(this);

	constructor(
		readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly api: ApiDriver,
		private readonly showId: string,
		private readonly cueListId: string,
		private readonly cueListName: string,
	) {}

	async select(cue: number) {
		await this.desk.recordStep(
			`SELECT CUE ${formatCueNumber(cue)} IN CUELIST VIEW`,
			"Select the Cue row without changing playback output.",
		);
		await this.desk.click(this.row(cue));
		await this.expect.selected(cue);
	}

	async edit(cue: number, values: CueEditorValues) {
		await this.select(cue);
		await this.desk.recordStep(
			`EDIT CUE ${formatCueNumber(cue)}`,
			"Edit the selected Cue through the visible Cuelist View fields.",
		);
		if (values.name != null) await this.commit("Title", values.name);
		if (values.fade != null) await this.commit("Fade", values.fade);
		if (values.delay != null) await this.commit("Delay", values.delay);
		if (values.trigger != null) await this.chooseTrigger(values.trigger);
		if (values.triggerTime != null)
			await this.commit("Trigger time", values.triggerTime);
	}

	async reject(cue: number, values: CueEditorValues) {
		await this.select(cue);
		if (values.fade != null) {
			await this.field("Fade").fill(values.fade);
			await this.field("Fade").press("Enter");
		}
		await expect(
			this.page
				.getByRole("alert")
				.filter({ hasText: "Cue edit was not saved" }),
		).toBeVisible();
	}

	async inspectSettings() {
		const settings = await this.openSettings();
		await settings.close();
	}

	async openSettings() {
		await this.desk.click(
			this.page.getByRole("button", {
				name: "Cuelist Settings",
				exact: true,
			}),
		);
		const dialog = this.page.getByRole("dialog", {
			name: "Cuelist Settings",
		});
		await expect(dialog).toContainText(this.cueListName);
		return new CueListSettings(this.page, this.desk, this.cueListName);
	}

	window() {
		return this.page.locator(".cue-table");
	}

	row(cue: number) {
		return this.page.locator(".cue-table tbody tr").filter({
			has: this.page
				.locator("td")
				.nth(1)
				.getByText(formatCueNumber(cue), { exact: true }),
		});
	}

	field(label: string) {
		return this.page.getByLabel(label, { exact: true });
	}

	private async commit(label: string, value: string) {
		const before = await this.revision();
		const field = this.field(label);
		await field.fill(value);
		await field.press("Enter");
		await expect.poll(() => this.revision()).toBeGreaterThan(before);
	}

	private async chooseTrigger(next: CueTriggerLabel) {
		const before = await this.revision();
		const scope = this.page.locator(".cue-properties");
		await scope
			.getByRole("button", { name: /^(GO|FOLLOW|TIME)$/, exact: true })
			.click();
		await this.page.getByRole("option", { name: next, exact: true }).click();
		await expect.poll(() => this.revision()).toBeGreaterThan(before);
	}

	private async revision() {
		const object = await this.api.showObject<CueList>(
			this.showId,
			"cue_list",
			this.cueListId,
		);
		if (!object) throw new Error(`Cuelist ${this.cueListId} is absent`);
		return object.revision;
	}
}

class CueTransferChoice {
	constructor(
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly operation: "COPY" | "MOVE",
	) {}

	async available() {
		const dialog = this.dialog();
		const title = this.operation === "COPY" ? "Copy" : "Move";
		await expect(dialog).toBeVisible();
		await expect(
			dialog.getByRole("button", { name: `Plain ${title}`, exact: true }),
		).toBeVisible();
		await expect(
			dialog.getByRole("button", { name: `Status ${title}`, exact: true }),
		).toBeVisible();
		await expect(
			dialog.getByRole("button", { name: "Cancel", exact: true }),
		).toBeVisible();
	}

	async cancel() {
		const dialog = this.dialog();
		await this.available();
		await this.desk.click(
			dialog.getByRole("button", { name: "Cancel", exact: true }),
		);
		await expect(dialog).toBeHidden();
	}

	private dialog() {
		const title = this.operation === "COPY" ? "Copy" : "Move";
		return this.page.getByRole("dialog", { name: `Cue ${title} choice` });
	}
}

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

	async trigger(expected: Cue["trigger"]) {
		expect((await this.owner.cue(this.playback, this.cue))?.trigger).toEqual(
			expected,
		);
	}

	async metadata(
		expected: Partial<Pick<Cue, "name" | "fade_millis" | "delay_millis">>,
	) {
		expect(await this.owner.cue(this.playback, this.cue)).toMatchObject(
			expected,
		);
	}

	async groupValueTiming(
		group: number,
		attribute: string,
		expected: { fade: ClockDuration; delay?: ClockDuration | null },
	) {
		const change = await this.groupChange(group, attribute);
		expect(change?.fade_millis).toBe(parseClockDuration(expected.fade));
		expect(change?.delay_millis).toBe(
			expected.delay == null
				? expected.delay
				: parseClockDuration(expected.delay),
		);
	}

	async groupValue(group: number, attribute: string, expected: number) {
		const change = await this.groupChange(group, attribute);
		expect(change?.value).toEqual({ kind: "normalized", value: expected });
	}

	private async groupChange(group: number, attribute: string) {
		const cue = await this.owner.cue(this.playback, this.cue);
		expect(cue).toBeDefined();
		const change = (cue?.group_changes ?? []).find(
			(candidate) =>
				candidate.group_id === String(validInteger(group, "Group")) &&
				candidate.attribute === attribute,
		);
		expect(change).toBeDefined();
		return change;
	}
}

class CueListExpectation {
	constructor(
		private readonly owner: BrowserCues,
		private readonly playback: number,
	) {}

	async configuration(expected: Partial<CueList>) {
		await expect
			.poll(() => this.owner.cueList(this.playback))
			.toMatchObject(expected);
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

	expectList(playback: number) {
		return new CueListExpectation(this, playback);
	}

	transferChoice(operation: "COPY" | "MOVE") {
		return new CueTransferChoice(this.page, this.desk, operation);
	}

	async openEditor(playback: number) {
		const cueList = await cueListForPlayback(this.api, this.showId(), playback);
		const playbackObject = await this.api.showObject<PlaybackDefinition>(
			this.showId(),
			"playback",
			String(playback),
		);
		if (!playbackObject) throw new Error(`Playback ${playback} is absent`);
		await this.page.setViewportSize({ width: 1280, height: 1100 });
		const shift = this.page.getByRole("button", {
			name: "SHIFT",
			exact: true,
		});
		if (!(await shift.isVisible().catch(() => false)))
			await this.desk.click(this.page.locator(".mode-toggle"));
		await this.desk.recordStep(
			`OPEN ${cueList.body.name} IN CUELIST VIEW`,
			"Open the assigned Cuelist from the visible Cuelist Pool.",
		);
		await this.desk.click(shift);
		await this.desk.click(
			this.page.getByRole("button", { name: "4", exact: true }),
		);
		await expect(this.page.locator(".cuelist-pool-window")).toBeVisible();
		await this.desk.click(
			this.page.locator(".cuelist-card").filter({
				hasText: playbackObject.body.name,
			}),
		);
		await expect(this.page.locator(".cue-table")).toBeVisible();
		return new CueEditor(
			this.page,
			this.desk,
			this.api,
			this.showId(),
			cueList.id,
			cueList.body.name,
		);
	}

	async reopenEditor(playback: number) {
		await this.api.openShow(this.showId(), { transition: "hold_current" });
		await this.page.reload();
		await expect(this.page.locator(".connection-cover")).toBeHidden({
			timeout: 10_000,
		});
		return this.openEditor(playback);
	}

	async configure(playback: number, configuration: CueListConfiguration) {
		const current = await cueListForPlayback(this.api, this.showId(), playback);
		const priority = configuration.priority ?? current.body.priority;
		if (
			!Number.isSafeInteger(priority) ||
			priority < -32_768 ||
			priority > 32_767
		)
			throw new Error("Cuelist priority must be a signed 16-bit integer");
		if (!this.api.session)
			throw new Error("Cuelist configuration requires an API session");
		const runtime = await this.api.request<any>(
			"POST",
			"/api/v2/playback-runtime/snapshot",
			{ identities: [] },
			true,
			undefined,
			{
				showId: this.showId(),
				deskId: this.api.session.desk.id,
			},
		);
		await new HttpPlaybackTopologyTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: this.api.session.token,
		}).apply(this.showId(), runtime.desk.scope.show_revision, {
			requestId: crypto.randomUUID(),
			action: {
				type: "save_cue_list",
				cueListId: current.body.id,
				expectedRevision: current.revision,
				expectedObjectId: current.id,
				body: { ...current.body, priority },
			},
		});
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

	async cueList(playback: number) {
		return (await cueListForPlayback(this.api, this.showId(), playback)).body;
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
