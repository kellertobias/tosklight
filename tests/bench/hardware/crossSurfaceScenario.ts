import { expect, type Locator, type Page } from "@playwright/test";
import { controlSurfaceOscPaths } from "@tosklight/ui/control-surface-contracts";
import { HttpGroupRecordingTransport } from "../../../apps/light-desktop/src/api/GroupRecordingTransport";
import type { BrowserCommands } from "../command-selection/commandScenario";
import { replaceProgrammingSelection } from "../command-selection/programmingSelection";
import type { ApiDriver, Session } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { LightBench } from "../core/lightBench";
import { setProgrammerGroupValue } from "../programmer/programmerValues";

/** Semantic operations whose subject is reconciliation between control surfaces. */
export class BrowserCrossSurface {
	constructor(
		private readonly api: ApiDriver,
		private readonly bench: LightBench,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly commands: BrowserCommands,
		private readonly showId: () => string,
	) {}

	async applyGroupOneAtFiftyViaApi(): Promise<void> {
		const session = this.requiredSession();
		const artnetMark = this.bench.artnet.mark();
		const sacnMark = this.bench.sacn.mark();
		const outcome = await setProgrammerGroupValue(this.api, {
			surface: "api",
			showId: this.showId(),
			groupId: "1",
			attribute: "intensity",
			value: { kind: "normalized", value: 0.5 },
			timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
		});
		expect(outcome).toMatchObject({ status: "changed", replayed: false });
		if (outcome.status !== "changed")
			throw new Error("Typed Group value must produce one authoritative event");
		expect(outcome.eventSequence).toBeGreaterThan(0);
		await this.bench.waitForGroupProgrammer("1", 0.5, session.token);
		await this.expectGroupOneAtFiftyOutput(artnetMark, sacnMark);
	}

	async applyGroupOneAtFiftyViaOsc(): Promise<void> {
		const session = this.requiredSession();
		const hardware = await this.bench.osc();
		const clientId = `cross-001-osc-${crypto.randomUUID()}`;
		try {
			await hardware.subscribe(clientId, session.desk.osc_alias);
			const auditBefore = await this.auditSequence();
			const artnetMark = this.bench.artnet.mark();
			const sacnMark = this.bench.sacn.mark();
			for (const action of [
				"grp",
				"digit-1",
				"at",
				"digit-5",
				"digit-0",
				"enter",
			] as const)
				await hardware.send(
					`/light/${session.desk.osc_alias}/${controlSurfaceOscPaths.programmer(action)}`,
					[true],
				);
			await this.bench.waitForGroupProgrammer("1", 0.5, session.token);
			const completed = (
				await this.api.request<Array<{ kind: string }>>(
					"GET",
					`/api/v2/audit?after=${auditBefore}`,
				)
			).filter((event) => event.kind === "command_applied");
			expect(completed).toHaveLength(1);
			await this.expectGroupOneAtFiftyOutput(artnetMark, sacnMark);
		} finally {
			await hardware
				.send("/light/unsubscribe", [clientId])
				.catch(() => undefined);
			await hardware.close();
		}
	}

	async expectCompleteFeedbackForPage(pageName: string): Promise<void> {
		const session = this.requiredSession();
		const hardware = await this.bench.osc();
		const clientId = `semantic-osc-001-${crypto.randomUUID()}`;
		try {
			await hardware.subscribe(clientId, session.desk.osc_alias);
			await expect.poll(() => this.hardwareConnected()).toBe(true);
			await hardware.expectAfter(
				0,
				`/light/${session.desk.osc_alias}/feedback/speed-group/5`,
			);
			const pageNumber = Number(pageName.match(/\d+/)?.[0]);
			if (!Number.isSafeInteger(pageNumber))
				throw new Error(`Page name "${pageName}" has no page number`);
			await this.api.request(
				"POST",
				`/api/v2/control-desks/${session.desk.id}/actions`,
				{
					request_id: crypto.randomUUID(),
					action: {
						type: "set_page",
						page: pageNumber,
						existing_only: true,
					},
				},
				true,
				undefined,
				{ showId: this.showId(), deskId: session.desk.id },
			);
			const mark = hardware.mark();
			await this.bench.tick(0);
			const requiredAddresses = [
				"page",
				"locked",
				"command-line",
				controlSurfaceOscPaths.pagePlaybackControl(1, "fader"),
				controlSurfaceOscPaths.pagePlaybackControl(1, "button/1"),
				controlSurfaceOscPaths.pagePlaybackControl(1, "current-cue"),
				"speed-group/1",
				"speed-group/5",
				...[
					"group",
					"at",
					"thru",
					"plus",
					"minus",
					"time",
					"delay",
					"link",
					"cue",
					"record",
					"clear",
					"enter",
					"preload",
				].map((key) => `programmer/${key}`),
			];
			for (const address of requiredAddresses)
				await hardware.expectAfter(
					mark,
					`/light/${session.desk.osc_alias}/feedback/${address}`,
				);
			await waitForFeedbackToSettle(hardware.messages);
			const cycle = hardware.messages.slice(mark);
			expect(new Set(cycle.map((message) => message.address)).size).toBe(
				cycle.length,
			);
			const quietMark = hardware.mark();
			await new Promise((resolve) => setTimeout(resolve, 100));
			expect(hardware.messages.slice(quietMark)).toHaveLength(0);
			await hardware.send("/light/unsubscribe", [clientId]);
			await expect.poll(() => this.hardwareConnected()).toBe(false);
		} finally {
			await this.close(hardware, clientId);
		}
	}

	async executeOscGroupCommandAndVerifyOutput(): Promise<void> {
		const session = this.requiredSession();
		const hardware = await this.bench.osc();
		const clientId = `semantic-osc-002-${crypto.randomUUID()}`;
		try {
			await hardware.subscribe(clientId, session.desk.osc_alias);
			const feedbackMark = hardware.mark();
			for (const action of [
				"group",
				"digit-1",
				"at",
				"digit-2",
				"digit-5",
				"enter",
			])
				await hardware.send(
					`/light/${session.desk.osc_alias}/programmer/${action}`,
					[true],
				);
			await expect
				.poll(async () => {
					const value = (await this.programmer(session)).group_values["1"]
						?.intensity;
					return this.normalizedValue(value);
				})
				.toBeCloseTo(0.25, 4);
			await hardware.expectAfter(
				feedbackMark,
				`/light/${session.desk.osc_alias}/feedback/command-line`,
			);
			const artnetMark = this.bench.artnet.mark();
			const sacnMark = this.bench.sacn.mark();
			await this.bench.tick(3_000);
			expect(
				Array.from(
					(
						await this.bench.artnet.nextAfter(artnetMark, "artnet", 1)
					).slots.slice(0, 12),
				),
			).toEqual(Array(12).fill(64));
			expect(
				Array.from(
					(await this.bench.sacn.nextAfter(sacnMark, "sacn", 101)).slots.slice(
						0,
						12,
					),
				),
			).toEqual(Array(12).fill(64));
		} finally {
			await this.close(hardware, clientId);
		}
	}

	async rejectInvalidGroupCommand(): Promise<void> {
		await this.desk.recordStep(
			"REJECT INVALID COMMAND",
			"Enter an invalid Group number and verify that programmer state and output stay unchanged.",
		);
		for (const key of ["GRP", "9", "9", "9", "AT", "5", "0", "ENT"])
			await this.desk.click(
				this.page.getByRole("button", { name: key, exact: true }),
			);
		await expect(
			this.page.getByRole("textbox", {
				name: "Command line",
				exact: true,
			}),
		).toHaveClass(/error/);
		const states = await this.api.request<any[]>("GET", "/api/v2/programmers");
		expect(
			states.every(
				(state) =>
					state.values.length === 0 &&
					Object.keys(state.group_values).length === 0,
			),
		).toBe(true);
		expect(
			(await this.bench.tick(0)).universes
				.find((entry) => entry.universe === 1)
				?.slots.slice(0, 12),
		).toEqual(Array(12).fill(0));
	}

	async expectDeskSubscriberIsolation(): Promise<void> {
		const first = this.requiredSession();
		const second = await this.createSession();
		await this.withSession(second, () =>
			this.api.setCommandLineText("GROUP 2 +"),
		);
		await this.commands.type("GROUP 1 +");
		const firstHardware = await this.bench.osc();
		const secondHardware = await this.bench.osc();
		try {
			const firstMark = firstHardware.mark();
			await firstHardware.subscribe("semantic-osc-003-a", first.desk.osc_alias);
			await firstHardware.expectAfter(
				firstMark,
				`/light/${first.desk.osc_alias}/feedback/speed-group/5`,
			);
			const secondMark = secondHardware.mark();
			await secondHardware.subscribe(
				"semantic-osc-003-b",
				second.desk.osc_alias,
			);
			await secondHardware.expectAfter(
				secondMark,
				`/light/${second.desk.osc_alias}/feedback/speed-group/5`,
			);
			expect(await this.commandFor(first)).toMatch(/^(?:G ?1|GROUP 1) \+$/);
			expect(await this.commandFor(second)).toMatch(/^(?:G ?2|GROUP 2) \+$/);
			const feedback = await firstHardware.expectAfter(
				firstMark,
				`/light/${first.desk.osc_alias}/feedback/command-line`,
			);
			expect(String(feedback.arguments[0])).toMatch(/^(?:G ?1|GROUP 1) \+$/);
			expect(
				secondHardware.messages
					.slice(secondMark)
					.some((message) =>
						String(message.arguments[0]).includes("GROUP 1 +"),
					),
			).toBe(false);

			await firstHardware.send("/light/unsubscribe", ["semantic-osc-003-a"]);
			// OSC unsubscribe has no acknowledgement frame. Let the server process it and
			// drain the subscriber's initial full feedback cycle before marking the
			// isolation window for the other desk's next action.
			await new Promise((resolve) => setTimeout(resolve, 75));
			const disconnected = firstHardware.mark();
			const secondAction = secondHardware.mark();
			await secondHardware.send(
				`/light/${second.desk.osc_alias}/programmer/digit-3`,
				[true],
			);
			await secondHardware.expectAfter(
				secondAction,
				`/light/${second.desk.osc_alias}/feedback/command-line`,
			);
			await new Promise((resolve) => setTimeout(resolve, 50));
			expect(firstHardware.messages.slice(disconnected)).toHaveLength(0);
			expect(
				(
					await this.api.request<{ hardware_connected: boolean }>(
						"GET",
						"/api/v2/bootstrap",
						undefined,
						false,
					)
				).hardware_connected,
			).toBe(true);
			await secondHardware.send("/light/unsubscribe", ["semantic-osc-003-b"]);
			await expect.poll(() => this.hardwareConnected()).toBe(false);
		} finally {
			await this.close(firstHardware, "semantic-osc-003-a");
			await this.close(secondHardware, "semantic-osc-003-b");
		}
	}

	async completeSharedValueWhilePeerDraftStaysLocal(): Promise<void> {
		const first = this.requiredSession();
		const fixtures = await this.fixtureIds();
		const existing = await this.api.showObject<any>(
			this.showId(),
			"group",
			"7",
		);
		await this.api.seedShowObject(
			this.showId(),
			"group",
			"7",
			{
				id: "7",
				number: 7,
				name: "Shared Group",
				fixtures: [fixtures[1], fixtures[2], fixtures[3], fixtures[4]],
				derived_from: null,
				frozen_from: null,
				programming: {},
			},
			existing?.revision ?? 0,
		);
		const browser = this.page.context().browser();
		if (!browser) throw new Error("OSC-005 requires a browser context");
		const secondContext = await browser.newContext();
		const secondPage = await secondContext.newPage();
		const second = await this.desk.openPeer(secondPage, this.bench.baseUrl);
		const firstHardware = await this.bench.osc();
		const secondHardware = await this.bench.osc();
		try {
			await this.commands.type("GROUP 7 +");
			for (const key of ["GRP", "1", "+"])
				await secondPage
					.getByRole("button", { name: key, exact: true })
					.click();
			await firstHardware.subscribe(
				"semantic-osc-005-first",
				first.desk.osc_alias,
			);
			await secondHardware.subscribe(
				"semantic-osc-005-second",
				second.desk.osc_alias,
			);
			await secondHardware.send(
				`/light/${second.desk.osc_alias}/programmer/digit-2`,
				[true],
			);
			await expect(secondPage.getByLabel("Command line")).toHaveValue(
				"G1 + F2",
			);
			await expect(this.page.getByLabel("Command line")).toHaveValue("G7 +");
			const firstFeedbackMark = firstHardware.mark();
			await firstHardware.send(
				`/light/${first.desk.osc_alias}/programmer/digit-8`,
				[true],
			);
			expect(
				(
					await firstHardware.expectAfter(
						firstFeedbackMark,
						`/light/${first.desk.osc_alias}/feedback/command-line`,
					)
				).arguments,
			).toEqual(["G7 + F8"]);
			await expect(this.page.getByLabel("Command line")).toHaveValue("G7 + F8");
			await expect(secondPage.getByLabel("Command line")).toHaveValue(
				"G1 + F2",
			);

			const auditBefore = await this.auditSequence();
			for (const action of ["at", "digit-5", "digit-0", "enter"])
				await firstHardware.send(
					`/light/${first.desk.osc_alias}/programmer/${action}`,
					[true],
				);
			for (const number of [1, 2, 3, 4, 8])
				await expect
					.poll(async () =>
						this.normalizedFixtureValue(
							await this.programmer(first),
							fixtures[number],
						),
					)
					.toBeCloseTo(0.5, 4);
			const firstState = await this.programmer(first);
			const secondState = await this.programmer(second);
			for (const number of [1, 2, 3, 4, 8])
				expect(
					this.normalizedFixtureValue(secondState, fixtures[number]),
				).toBeCloseTo(
					this.normalizedFixtureValue(firstState, fixtures[number]),
					4,
				);
			expect(secondState.command_line).toBe("G1 + F2");
			const completed = (
				await this.api.request<any[]>(
					"GET",
					`/api/v2/audit?after=${auditBefore}`,
				)
			).filter((event) => event.kind === "command_applied");
			expect(completed).toHaveLength(1);
			const artnetMark = this.bench.artnet.mark();
			await this.bench.tick(3_000);
			expect(
				Array.from(
					(
						await this.bench.artnet.nextAfter(artnetMark, "artnet", 1)
					).slots.slice(0, 12),
				),
			).toEqual([128, 128, 128, 128, 0, 0, 0, 128, 0, 0, 0, 0]);

			await firstHardware.send("/light/unsubscribe", [
				"semantic-osc-005-first",
			]);
			await secondHardware.send("/light/unsubscribe", [
				"semantic-osc-005-second",
			]);
			await expect.poll(() => this.hardwareConnected()).toBe(false);

			await secondPage.getByLabel("Command line").fill("");
			await secondPage
				.getByRole("button", { name: "GRP", exact: true })
				.click();
			await secondPage
				.getByRole("button", { name: "ENT", exact: true })
				.click();
			const groupModeMark = secondHardware.mark();
			await secondHardware.send("/light/subscribe", [
				"semantic-osc-005-second",
				second.desk.osc_alias,
				secondHardware.feedbackPort,
			]);
			await secondHardware.expectAfter(
				groupModeMark,
				`/light/${second.desk.osc_alias}/feedback/page`,
			);
			await secondHardware.send(
				`/light/${second.desk.osc_alias}/programmer/digit-3`,
				[true],
			);
			await expect(secondPage.getByLabel("Command line")).toHaveValue("G3");

			await this.page.getByLabel("Command line").fill("G7 +");
			const reconnectMark = firstHardware.mark();
			await firstHardware.send("/light/subscribe", [
				"semantic-osc-005-first",
				first.desk.osc_alias,
				firstHardware.feedbackPort,
			]);
			expect(
				(
					await firstHardware.expectAfter(
						reconnectMark,
						`/light/${first.desk.osc_alias}/feedback/command-line`,
					)
				).arguments,
			).toEqual(["G7 +"]);
			await secondPage.getByLabel("Command line").fill("G1 + F2");
			const reattachMark = firstHardware.mark();
			await firstHardware.send("/light/subscribe", [
				"semantic-osc-005-first",
				second.desk.osc_alias,
				firstHardware.feedbackPort,
			]);
			await firstHardware.expectAfter(
				reattachMark,
				`/light/${second.desk.osc_alias}/feedback/page`,
			);
			await firstHardware.send(
				`/light/${second.desk.osc_alias}/programmer/digit-9`,
				[true],
			);
			await expect(secondPage.getByLabel("Command line")).toHaveValue(
				"G1 + F29",
			);
			await expect(this.page.getByLabel("Command line")).toHaveValue("G7 +");
		} finally {
			await this.close(firstHardware, "semantic-osc-005-first");
			await this.close(secondHardware, "semantic-osc-005-second");
			await secondContext.close();
		}
	}

	async verifyCurrentAndExplicitPageOscAddressing(): Promise<void> {
		const first = this.requiredSession();
		const second = await this.createSession();
		await this.setDeskPage(second, 2);
		await this.selectPlaybackPage("Page 1");
		const firstHardware = await this.bench.osc();
		const secondHardware = await this.bench.osc();
		const firstId = "semantic-osc-006-first";
		const secondId = "semantic-osc-006-second";
		try {
			await firstHardware.subscribe(firstId, first.desk.osc_alias);
			await secondHardware.subscribe(secondId, second.desk.osc_alias);
			expect(
				(
					await firstHardware.expectAfter(
						0,
						`/light/${first.desk.osc_alias}/feedback/page`,
					)
				).arguments,
			).toEqual([1]);
			expect(
				(
					await secondHardware.expectAfter(
						0,
						`/light/${second.desk.osc_alias}/feedback/page`,
					)
				).arguments,
			).toEqual([2]);

			const firstAddress = `/light/${first.desk.osc_alias}/page-playback/1/button/1`;
			await firstHardware.send(firstAddress, [true]);
			await expect
				.poll(async () => (await this.activePlayback(1))?.current_cue_number)
				.toBe(1);
			await firstHardware.send(firstAddress, [false]);
			await new Promise((resolve) => setTimeout(resolve, 50));
			expect(await this.activePlayback(1)).toMatchObject({
				current_cue_number: 1,
			});
			await secondHardware.send(
				`/light/${second.desk.osc_alias}/page-playback/1/button/1`,
				[true],
			);
			await expect
				.poll(async () => (await this.activePlayback(2))?.current_cue_number)
				.toBe(1);
			await this.api.playbackNumberAction(2, "off", {});
			await expect
				.poll(async () => (await this.activePlayback(2))?.enabled)
				.toBe(false);

			const pageFeedbackMark = firstHardware.mark();
			await this.selectPlaybackPage("Page 2");
			await expect.poll(() => this.deskPage(first)).toBe(2);
			expect(
				(
					await firstHardware.expectAfter(
						pageFeedbackMark,
						`/light/${first.desk.osc_alias}/feedback/page`,
					)
				).arguments,
			).toEqual([2]);
			await firstHardware.send(firstAddress, [true]);
			await expect
				.poll(async () => (await this.activePlayback(2))?.enabled)
				.toBe(true);
			expect(await this.deskPage(second)).toBe(2);

			await this.selectPlaybackPage("Page 1");
			await expect.poll(() => this.deskPage(first)).toBe(1);
			for (const level of [0, 0.5, 1]) {
				const feedbackMark = firstHardware.mark();
				await firstHardware.sendFloat(
					`/light/${first.desk.osc_alias}/page-playback/1/fader`,
					level,
				);
				await expect
					.poll(async () => (await this.activePlayback(1))?.fader_position)
					.toBeCloseTo(level, 4);
				await firstHardware.expectAfter(
					feedbackMark,
					`/light/${first.desk.osc_alias}/feedback/page-playback/1/fader`,
				);
			}
			for (const level of [0, 0.5, 1]) {
				await firstHardware.sendFloat("/light/playback/2/1/fader", level);
				await expect
					.poll(async () => (await this.activePlayback(2))?.fader_position)
					.toBeCloseTo(level, 4);
				expect(await this.deskPage(first)).toBe(1);
			}
			await secondHardware.sendFloat("/light/playback/2/1/fader", 0.5);
			await expect
				.poll(async () => (await this.activePlayback(2))?.fader_position)
				.toBeCloseTo(0.5, 4);

			const beforeNoOp = await this.playbackOperationalState();
			await firstHardware.send(
				`/light/${first.desk.osc_alias}/page-playback/2/button/1`,
				[true],
			);
			await firstHardware.send("/light/playback/1/2/button/1", [true]);
			await new Promise((resolve) => setTimeout(resolve, 50));
			expect(await this.playbackOperationalState()).toEqual(beforeNoOp);
		} finally {
			await this.close(firstHardware, firstId);
			await this.close(secondHardware, secondId);
		}
	}

	async reconcileExternalGroupMutation(): Promise<void> {
		await this.openCrossSurfaceLayout();
		const groupCard = this.page
			.locator(".group-pool-window .group-card")
			.filter({ hasText: "Front Dimmers" });
		await this.openGroupSettingsByHold(groupCard);
		const dialog = this.page.getByRole("dialog", {
			name: "Group 3 settings",
			exact: true,
		});
		await expect(dialog).toBeVisible();
		await this.desk.click(dialog.getByRole("tab", { name: "Projection" }));
		const group = await this.requiredGroup(3);
		const fixtures = await this.fixtureIds();
		await replaceProgrammingSelection(this.api, {
			surface: "api",
			showId: this.showId(),
			fixtures: [fixtures[5]],
		});
		const session = this.requiredSession();
		const outcome = await new HttpGroupRecordingTransport({
			baseUrl: this.api.baseUrl,
			sessionToken: session.token,
		}).record(this.showId(), {
			requestId: "cross-002-external-group-merge",
			groupId: "3",
			operation: "merge",
			expectedObjectRevision: group.revision,
		});
		expect(outcome).toMatchObject({
			status: "changed",
			group: { state: "stored", id: "3", revision: group.revision + 1 },
		});
		await expect(groupCard).toContainText("5 fixtures");
		await this.desk.click(
			dialog.getByRole("button", { name: "Close settings" }),
		);
		await this.openGroupSettingsByHold(groupCard);
		await expect(dialog).toBeVisible();
		await this.desk.click(dialog.getByRole("tab", { name: "Projection" }));
		await expect(dialog.getByLabel("Projection preview")).toBeVisible();
		await this.desk.click(
			dialog.getByRole("button", { name: "Close settings" }),
		);
		await this.desk.click(groupCard);
		await expect(groupCard).toHaveClass(/selected/);
		await setProgrammerGroupValue(this.api, {
			surface: "api",
			showId: this.showId(),
			groupId: "3",
			attribute: "intensity",
			value: { kind: "normalized", value: 0.5 },
			timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
		});
		await expect(
			this.page.getByRole("group", { name: "Enc 1 · Dimmer", exact: true }),
		).toContainText("50%");
		await this.bench.tick(3_000);
		for (const number of [1, 5]) {
			const value = this.fixtureRow(number).locator(".source-value").first();
			await expect(value).toHaveClass(/source-programmer/);
			await expect(value).toContainText("50%");
		}
		await this.page.waitForTimeout(700);
		await this.page.reload();
		await expect(this.page.locator(".connection-cover")).toBeHidden({
			timeout: 10_000,
		});
		await expect(
			this.page
				.locator(".group-pool-window .group-card")
				.filter({ hasText: "Front Dimmers" }),
		).toContainText("5 fixtures");
		for (const number of [1, 5])
			await expect(
				this.fixtureRow(number).locator(".source-value").first(),
			).toContainText("50%");
	}

	private async openGroupSettingsByHold(groupCard: Locator): Promise<void> {
		await groupCard.dispatchEvent("pointerdown", {
			pointerId: 1,
			pointerType: "mouse",
			button: 0,
		});
		await this.page.waitForTimeout(700);
		await groupCard.dispatchEvent("pointerup", {
			pointerId: 1,
			pointerType: "mouse",
			button: 0,
		});
	}

	private async expectGroupOneAtFiftyOutput(
		artnetMark: number,
		sacnMark: number,
	): Promise<void> {
		const frame = await this.bench.tick(3_000);
		expect(
			frame.universes.find((entry) => entry.universe === 1)?.slots.slice(0, 12),
		).toEqual(Array(12).fill(128));
		const artnet = await this.bench.artnet.nextAfter(artnetMark, "artnet", 1);
		const sacn = await this.bench.sacn.nextAfter(sacnMark, "sacn", 101);
		expect(Array.from(artnet.slots.slice(0, 12))).toEqual(Array(12).fill(128));
		expect(Array.from(sacn.slots.slice(0, 12))).toEqual(Array(12).fill(128));
	}

	private createSession(): Promise<Session> {
		return this.api.request(
			"POST",
			"/api/v2/sessions",
			{ username: "Operator", client_id: crypto.randomUUID() },
			false,
		);
	}

	private async programmer(session: Session): Promise<any> {
		const states = await this.api.request<any[]>("GET", "/api/v2/programmers");
		const state = states.find(
			(entry) => entry.session_id === session.session_id,
		);
		if (!state) throw new Error(`Programmer ${session.session_id} is absent`);
		return state;
	}

	private async commandFor(session: Session): Promise<string> {
		return (await this.programmer(session)).command_line;
	}

	private normalizedFixtureValue(state: any, fixtureId: string): number {
		const value = state.values.find(
			(entry: any) =>
				entry.fixture_id === fixtureId && entry.attribute === "intensity",
		)?.value;
		return typeof value === "number" ? value : value?.value;
	}

	private normalizedValue(value: unknown): number | null {
		let current = value;
		while (current && typeof current === "object" && "value" in current)
			current = (current as { value: unknown }).value;
		return typeof current === "number" ? current : null;
	}

	private async fixtureIds(): Promise<Record<number, string>> {
		return Object.fromEntries(
			(await this.api.patch()).fixtures.map((fixture) => [
				fixture.fixture_number,
				fixture.fixture_id,
			]),
		);
	}

	private requiredSession(): Session {
		if (!this.api.session)
			throw new Error(
				"Cross-surface scenario requires an authenticated session",
			);
		return this.api.session;
	}

	private async hardwareConnected(): Promise<boolean> {
		return (
			await this.api.request<{ hardware_connected: boolean }>(
				"GET",
				"/api/v2/bootstrap",
				undefined,
				false,
			)
		).hardware_connected;
	}

	private async auditSequence(): Promise<number> {
		return Math.max(
			0,
			...(
				await this.api.request<Array<{ revision: number }>>(
					"GET",
					"/api/v2/audit?after=0",
				)
			).map((event) => event.revision),
		);
	}

	private playbackOverview(): Promise<any> {
		return this.api.request("GET", "/api/v2/playback-overview");
	}

	private async playbackOperationalState(): Promise<unknown> {
		const overview = await this.playbackOverview();
		return {
			active_page: overview.active_page,
			active: overview.active
				.map((playback: any) => ({
					playback_number: playback.playback_number,
					enabled: playback.enabled,
					current_cue_number: playback.current_cue_number,
					fader_position: playback.fader_position,
					master: playback.master,
					flash: playback.flash,
				}))
				.sort(
					(a: { playback_number: number }, b: { playback_number: number }) =>
						a.playback_number - b.playback_number,
				),
		};
	}

	private async activePlayback(number: number): Promise<any | undefined> {
		return (await this.playbackOverview()).active.find(
			(playback: any) => playback.playback_number === number,
		);
	}

	private async deskPage(session: Session): Promise<number> {
		return this.withSession(
			session,
			async () => (await this.playbackOverview()).active_page,
		);
	}

	private async setDeskPage(session: Session, page: number): Promise<void> {
		await this.withSession(session, () =>
			this.api.request(
				"POST",
				`/api/v2/control-desks/${session.desk.id}/actions`,
				{
					request_id: crypto.randomUUID(),
					action: { type: "set_page", page, existing_only: true },
				},
				true,
				undefined,
				{ showId: this.showId(), deskId: session.desk.id },
			),
		);
	}

	private async selectPlaybackPage(name: string): Promise<void> {
		const software = this.page.locator(".playback-page-current");
		if (await software.count()) await this.desk.click(software);
		else
			await this.desk.click(
				this.page
					.locator(".hardware-control-summary")
					.getByRole("button", { name: /^Page \d+$/ }),
			);
		const dialog = this.page.locator(".playback-page-modal");
		await expect(dialog).toBeVisible();
		await this.desk.click(dialog.getByRole("button").filter({ hasText: name }));
	}

	private async withSession<T>(
		session: Session,
		action: () => Promise<T>,
	): Promise<T> {
		const original = this.api.session;
		this.api.session = session;
		try {
			return await action();
		} finally {
			this.api.session = original;
		}
	}

	private async close(hardware: any, clientId: string): Promise<void> {
		try {
			await hardware.send("/light/unsubscribe", [clientId]);
		} catch {
			// Cleanup must not hide the contract failure.
		} finally {
			await hardware.close();
		}
	}

	private async requiredGroup(number: number): Promise<any> {
		const group = await this.api.showObject(
			this.showId(),
			"group",
			String(number),
		);
		if (!group) throw new Error(`Group ${number} is absent`);
		return group;
	}

	private async openCrossSurfaceLayout(): Promise<void> {
		const fixtureSheet = this.page
			.locator(".desk-pane")
			.filter({ has: this.page.locator(".fixture-window") });
		const groupPool = this.page
			.locator(".desk-pane")
			.filter({ has: this.page.locator(".group-pool-window") });
		if ((await fixtureSheet.isVisible()) && (await groupPool.isVisible()))
			return;
		const presetPane = this.page
			.locator(".desk-pane")
			.filter({ has: this.page.locator(".preset-pool-window") });
		await presetPane.getByRole("button", { name: "Settings" }).click();
		await this.page
			.getByRole("dialog", { name: "Pane Settings" })
			.getByRole("button", { name: "Remove pane" })
			.click();
		const box = await this.page.locator(".desk-grid").boundingBox();
		if (!box) throw new Error("Desk grid is not visible");
		await this.page.mouse.click(
			box.x + box.width * 0.1,
			box.y + box.height * 0.1,
		);
		await this.page
			.locator(".window-picker")
			.getByRole("button", { name: "Group pool" })
			.click();
		await expect(groupPool).toBeVisible();
	}

	private fixtureRow(number: number): Locator {
		return this.page
			.locator(".fixture-window .ui-data-table-row:not(.header)")
			.filter({
				has: this.page.getByRole("cell", {
					name: String(number),
					exact: true,
				}),
			})
			.first();
	}
}

async function waitForFeedbackToSettle(
	messages: readonly unknown[],
	timeoutMs = 2_000,
): Promise<void> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const count = messages.length;
		await new Promise((resolve) => setTimeout(resolve, 75));
		if (messages.length === count) return;
	}
	throw new Error("OSC feedback did not settle after the page change");
}
