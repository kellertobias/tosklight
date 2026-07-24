import type { Page } from "@playwright/test";
import { oscProgrammerActionForKey } from "../../../shared/programmerKeypad";
import type { ApiDriver } from "./api";
import type {
	BrowserCommands,
	BrowserKeypad,
	KeypadKey,
} from "./commandScenario";
import type { DeskDriver } from "./desk";
import type { SimulatedHardware } from "./hardwareScenario";
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

function singleItemRoute(
	kind: ItemKind,
	mutate: (
		mutation: SelectionMutation,
		targets: readonly SelectionTarget[],
	) => Promise<unknown>,
	label: string,
): SelectionItemRoute {
	return {
		item: (number) =>
			mutate("replace", [kind === "fixture" ? fixture(number) : group(number)]),
		items: async () => {
			throw new Error(
				`${label} ordered multi-target selection is pending route verification`,
			);
		},
		range: async () => {
			throw new Error(`${label} range selection is pending route verification`);
		},
	};
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
		const visible = new BrowserVisibleSelection(page, desk, api);
		const apiMutation = this.apiMutation.bind(this);
		const keypadMutation = this.keypadMutation.bind(this);
		const oscMutation = this.oscMutation.bind(this);
		const fixtureSheet = visible.fixtures.via.fixtureSheet;
		const stage = visible.fixtures.via.stage;
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
				osc: singleItemRoute("fixture", oscMutation, "OSC"),
				ui: fixtureSheet,
				touch: fixtureSheet.via.touch,
				fixtureSheet,
				stage,
				click: stage.via.click,
				shiftClick: stage.via.shiftClick as SelectionItemRoute,
			},
		);
		this.groups = new RoutedItems(
			"group",
			(mutation, targets) => this.unqualified(mutation, targets),
			{
				api: routeItems("group", apiMutation),
				keypad: routeItems("group", keypadMutation),
				osc: singleItemRoute("group", oscMutation, "OSC"),
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
		const escapeMark = this.hardware.mark();
		await this.sendOscKey(alias, "ESC", true);
		await this.hardware.expectAfter(
			escapeMark,
			`/light/${alias}/feedback/command-line`,
		);
		const escapeReleaseMark = this.hardware.mark();
		await this.sendOscKey(alias, "ESC", false);
		await this.hardware.expectAfter(
			escapeReleaseMark,
			`/light/${alias}/feedback/command-line`,
		);
		for (const event of oscSelectionEvents(alias, action, targets, authority)) {
			const mark = this.hardware.mark();
			await this.hardware.send(event.address, [...event.arguments]);
			await this.hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/command-line`,
			);
		}
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

	private sendOscKey(alias: string, key: "ESC", pressed: boolean) {
		return this.hardware.send(
			`/light/${alias}/programmer/${oscProgrammerActionForKey(key)}`,
			[pressed],
		);
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
