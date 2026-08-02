import { expect, type Locator, type Page } from "@playwright/test";
import { controlSurfaceOscPaths } from "@tosklight/ui/control-surface-contracts";
import type { BrowserCommands } from "../command-selection/commandScenario";
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

	async expectCompleteFeedbackForPage(pageName: string): Promise<void> {
		const session = this.requiredSession();
		const hardware = await this.bench.osc();
		const clientId = `semantic-osc-001-${crypto.randomUUID()}`;
		try {
			await hardware.subscribe(clientId, session.desk.osc_alias);
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
			for (const address of [
				"page",
				"command-line",
				"programmer/group",
				controlSurfaceOscPaths.pagePlaybackControl(1, "fader"),
				controlSurfaceOscPaths.pagePlaybackControl(1, "button/1"),
				"speed-group/1",
				"speed-group/5",
			])
				await hardware.expectAfter(
					mark,
					`/light/${session.desk.osc_alias}/feedback/${address}`,
				);
			await waitForFeedbackToSettle(hardware.messages);
			const quietMark = hardware.mark();
			await new Promise((resolve) => setTimeout(resolve, 100));
			expect(hardware.messages.slice(quietMark)).toHaveLength(0);
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
		} finally {
			await this.close(firstHardware, "semantic-osc-003-a");
			await this.close(secondHardware, "semantic-osc-003-b");
		}
	}

	async completeSharedValueWhilePeerDraftStaysLocal(): Promise<void> {
		const first = this.requiredSession();
		const second = await this.createSession();
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
		await this.withSession(second, () =>
			this.api.setCommandLineText("GROUP 1 +"),
		);
		await this.commands.execute("GROUP 7 + 8 AT 50");
		const firstState = await this.programmer(first);
		const secondState = await this.programmer(second);
		expect(secondState.command_line).toMatch(/^(?:G ?1|GROUP 1) \+$/);
		for (const number of [1, 2, 3, 4, 8]) {
			expect(
				this.normalizedFixtureValue(firstState, fixtures[number]),
			).toBeCloseTo(0.5, 4);
			expect(
				this.normalizedFixtureValue(secondState, fixtures[number]),
			).toBeCloseTo(0.5, 4);
		}
		expect(
			(await this.bench.tick(3_000)).universes
				.find((entry) => entry.universe === 1)
				?.slots.slice(0, 12),
		).toEqual([128, 128, 128, 128, 0, 0, 0, 128, 0, 0, 0, 0]);
	}

	async reconcileExternalGroupMutation(): Promise<void> {
		await this.openCrossSurfaceLayout();
		const groupCard = this.page
			.locator(".group-pool-window .group-card")
			.filter({ hasText: "Front Dimmers" });
		await this.openGroupContext(groupCard);
		const order = this.page.locator(".group-context-menu .group-order");
		await expect(order).not.toContainText("Fixture 5");
		const group = await this.requiredGroup(3);
		const fixtures = await this.fixtureIds();
		await this.api.seedShowObject(
			this.showId(),
			"group",
			"3",
			{ ...group.body, fixtures: [...group.body.fixtures, fixtures[5]] },
			group.revision,
		);
		await expect(groupCard).toContainText("5 fixtures");
		await expect(order).toContainText("5. Fixture 5");
		await this.desk.click(
			this.page
				.locator(".group-context-menu")
				.getByRole("button", { name: "Select live group", exact: true }),
		);
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

	private async openGroupContext(card: Locator): Promise<void> {
		await card.scrollIntoViewIfNeeded();
		const box = await card.boundingBox();
		if (!box) throw new Error("Group card is not visible");
		await this.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await this.page.mouse.down();
		await this.page.waitForTimeout(700);
		await this.page.mouse.up();
		await expect(this.page.locator(".group-context-menu")).toBeVisible();
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
