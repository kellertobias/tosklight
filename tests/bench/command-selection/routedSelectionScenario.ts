import type { Page } from "@playwright/test";
import { oscProgrammerActionForKey } from "@tosklight/ui/programmer-keypad";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import type {
	BrowserCommands,
	BrowserKeypad,
	KeypadKey,
} from "./commandScenario";
import {
	fixture,
	fixtureRange,
	group,
	groupRange,
	type SelectionPoint,
	type SelectionTarget,
	selectionRange,
} from "./selectionContract";
import {
	SeededSelectionRouteChoice,
	type SelectionMutation,
	type SelectionRouteAdapter,
	type SelectionRouteChoiceReport,
} from "./selectionRouteChoice";
import type { BrowserSelection } from "./selectionScenario";
import {
	oscSelectionEvents,
	type SelectionTransportAuthority,
	selectionKeys,
} from "./selectionTransportScenario";
import { BrowserVisibleSelection } from "./selectionVisibleScenario";

type ItemKind = "fixture" | "group";

interface SelectionItemRoute {
	item(number: number): Promise<unknown>;
	items(...numbers: number[]): Promise<unknown>;
	range(first: number, last: number): Promise<unknown>;
}

class RoutedItems implements SelectionItemRoute {
	constructor(
		private readonly kind: ItemKind,
		private readonly mutate: (
			mutation: SelectionMutation,
			targets: readonly SelectionTarget[],
		) => Promise<unknown>,
		readonly via: Record<string, SelectionItemRoute>,
	) {}

	item(number: number) {
		return this.mutate("replace", [this.point(number)]);
	}

	items(...numbers: number[]) {
		return this.mutate(
			"replace",
			numbers.map((number) => this.point(number)),
		);
	}

	range(first: number, last: number) {
		return this.mutate("replace", [
			this.kind === "fixture"
				? fixtureRange(first, last)
				: groupRange(first, last),
		]);
	}

	private point(number: number) {
		return this.kind === "fixture" ? fixture(number) : group(number);
	}
}

function routeItems(
	kind: ItemKind,
	mutate: (
		mutation: SelectionMutation,
		targets: readonly SelectionTarget[],
	) => Promise<unknown>,
): SelectionItemRoute {
	return new RoutedItems(kind, mutate, {});
}

export class BrowserRoutedSelection {
	readonly fixtures: RoutedItems;
	readonly groups: RoutedItems;
	readonly routeChoice: SeededSelectionRouteChoice;

	constructor(
		private readonly core: BrowserSelection,
		private readonly api: ApiDriver,
		private readonly command: BrowserCommands,
		private readonly keypad: BrowserKeypad,
		private readonly hardware: SimulatedHardware,
		page: Page,
		desk: DeskDriver,
		seed: string,
	) {
		const visible = new BrowserVisibleSelection(page, desk, api, () =>
			core.observe(),
		);
		const apiMutation = this.apiMutation.bind(this);
		const keypadMutation = this.keypadMutation.bind(this);
		const oscMutation = this.oscMutation.bind(this);
		const fixtureSheet = visible.fixtures.via.fixtureSheet;
		const pool = visible.groups.via.pool;
		this.routeChoice = new SeededSelectionRouteChoice(seed, [
			adapter("api", apiMutation),
			adapter("keypad", keypadMutation),
		]);
		this.fixtures = new RoutedItems(
			"fixture",
			(mutation, targets) => this.unqualified(mutation, targets),
			{
				api: routeItems("fixture", apiMutation),
				keypad: routeItems("fixture", keypadMutation),
				osc: routeItems("fixture", oscMutation),
				ui: fixtureSheet,
				touch: fixtureSheet.via.touch,
				fixtureSheet,
				click: fixtureSheet.via.click,
			},
		);
		this.groups = new RoutedItems(
			"group",
			(mutation, targets) => this.unqualified(mutation, targets),
			{
				api: routeItems("group", apiMutation),
				keypad: routeItems("group", keypadMutation),
				osc: routeItems("group", oscMutation),
				ui: pool,
				pool,
			},
		);
	}

	get routeReports(): readonly SelectionRouteChoiceReport[] {
		return this.routeChoice.reports;
	}

	targets(...targets: SelectionTarget[]) {
		return this.unqualified("replace", targets);
	}

	add(...targets: SelectionTarget[]) {
		return this.unqualified("add", targets);
	}

	remove(...targets: SelectionTarget[]) {
		return this.unqualified("remove", targets);
	}

	range(first: SelectionPoint, last: SelectionPoint) {
		return this.targets(selectionRange(first, last));
	}

	clear() {
		return this.core.clear();
	}

	previous() {
		return this.core.previous();
	}

	next() {
		return this.core.next();
	}

	all() {
		return this.core.all();
	}

	observe() {
		return this.core.observe();
	}

	expectSelection(...targets: SelectionTarget[]) {
		return this.core.expectSelection(...targets);
	}

	private unqualified(
		action: SelectionMutation,
		targets: readonly SelectionTarget[],
	) {
		return this.routeChoice.execute({ action, targets });
	}

	private apiMutation(
		action: SelectionMutation,
		targets: readonly SelectionTarget[],
	) {
		if (action === "replace") return this.core.targets(...targets);
		if (action === "add") return this.core.add(...targets);
		return this.core.remove(...targets);
	}

	private async keypadMutation(
		action: SelectionMutation,
		targets: readonly SelectionTarget[],
	) {
		await this.command.clear();
		const authority = await this.transportAuthority();
		await this.keypad.press(
			selectionKeys(action, targets, authority) as KeypadKey[],
		);
	}

	private async oscMutation(
		action: SelectionMutation,
		targets: readonly SelectionTarget[],
	) {
		if (!this.api.session) throw new Error("API session is not initialized");
		const alias = this.api.session.desk.osc_alias;
		const authority = await this.transportAuthority();
		const commandLine = (await this.api.getCommandLine()).commandLine;
		if (!commandLine.pristine)
			throw new Error(
				"OSC semantic selection requires a pristine command line because OSC Escape is a desk action, not a Programmer edit",
			);
		for (const event of oscSelectionEvents(alias, action, targets, authority)) {
			await this.sendOscEvent(
				alias,
				event.key,
				event.phase === "press",
				event.address,
			);
		}
	}

	private async sendOscEvent(
		alias: string,
		key: KeypadKey,
		pressed: boolean,
		address = `/light/${alias}/programmer/${oscProgrammerActionForKey(key)}`,
	): Promise<void> {
		if (!pressed) {
			await this.hardware.send(address, [false]);
			return;
		}
		const before = (await this.api.getCommandLine()).commandLine.revision;
		const mark = this.hardware.mark();
		await this.hardware.send(address, [true]);
		await this.hardware.expectAfter(
			mark,
			`/light/${alias}/feedback/command-line`,
		);
		await this.waitForCommandLineRevision(before, key);
	}

	private async waitForCommandLineRevision(
		before: number,
		key: KeypadKey,
	): Promise<void> {
		const deadline = Date.now() + 2_000;
		do {
			if ((await this.api.getCommandLine()).commandLine.revision > before)
				return;
			await new Promise<void>((resolve) => setTimeout(resolve, 5));
		} while (Date.now() < deadline);
		throw new Error(
			`Timed out waiting for OSC Programmer key ${key} to advance command-line revision ${before}`,
		);
	}

	private async transportAuthority(): Promise<SelectionTransportAuthority> {
		const [commandLine, bootstrap] = await Promise.all([
			this.api.getCommandLine(),
			this.api.request<{ active_show: { id: string } | null }>(
				"GET",
				"/api/v2/bootstrap",
			),
		]);
		if (!bootstrap.active_show) throw new Error("No active Show");
		const groups = await this.api.showObjects(
			bootstrap.active_show.id,
			"group",
		);
		return {
			defaultTarget: commandLine.commandLine.target,
			groupNumbers: groups.map((candidate) => Number(candidate.id)),
		};
	}
}

function adapter(
	name: "api" | "keypad",
	mutate: (
		action: SelectionMutation,
		targets: readonly SelectionTarget[],
	) => Promise<unknown>,
): SelectionRouteAdapter {
	return {
		name,
		capabilities: {
			actions: ["replace", "add", "remove"],
			targets: [
				"fixture",
				"fixture_range",
				"group",
				"group_range",
				"dereferenced_group",
			],
		},
		mutate: ({ action, targets }) => mutate(action, targets),
	};
}
