import type { BrowserScenarioWorld } from "./browserScenario";
import type { PaneHandle, PanePlacement } from "../window-system/desktopScenario";
import { Show, type DefinedShow } from "../show/showCatalog";
import type { OperatorPaneType } from "../window-system/paneTypes";
import type { SelectionTarget } from "../command-selection/selectionContract";

export type TestRecipeRoute = "api" | "keypad" | "osc" | "touch" | "ui";

interface TestRecipeTypes {
	"use-registered-desktop": {
		options: { name: string };
		result: void;
	};
	"use-canonical-show": {
		options: { show: Show | DefinedShow };
		result: void;
	};
	"select-representative-fixtures": {
		options: { selection: readonly SelectionTarget[] };
		result: void;
	};
	"record-two-cue-dimmer-show": {
		options: {
			playback: number;
			selection: readonly SelectionTarget[];
		low?: number;
			high?: number;
		};
		result: void;
	};
	"use-representative-output-show": {
		options: Record<never, never>;
		result: void;
	};
	"open-typed-pane": {
		options: {
			desktop: string;
			type: OperatorPaneType;
			placement: PanePlacement;
		};
		result: PaneHandle<OperatorPaneType>;
	};
}

export type TestRecipeName = keyof TestRecipeTypes;
export type TestRecipeOptions<N extends TestRecipeName> =
	TestRecipeTypes[N]["options"];
export type TestRecipeResult<N extends TestRecipeName> =
	TestRecipeTypes[N]["result"];

export interface TestRecipeReport {
	name: TestRecipeName;
	route: TestRecipeRoute;
	steps: Array<{ title: string; description: string }>;
}

type RecipeWorld = Pick<
	BrowserScenarioWorld,
	"desktop" | "encoder" | "record" | "selection" | "show"
>;

interface TestRecipeDefinition<N extends TestRecipeName> {
	readonly routes: readonly TestRecipeRoute[];
	readonly execute: (
		world: RecipeWorld,
		options: TestRecipeOptions<N>,
	) => Promise<TestRecipeResult<N>>;
}

const registry = new Map<
	TestRecipeName,
	TestRecipeDefinition<TestRecipeName>
>();

export function defineTestRecipe<N extends TestRecipeName>(
	name: N,
	routes: readonly TestRecipeRoute[],
	execute: TestRecipeDefinition<N>["execute"],
): void {
	if (registry.has(name)) throw new Error(`Test recipe "${name}" is already registered`);
	if (routes.length === 0) throw new Error(`Test recipe "${name}" must support at least one route`);
	registry.set(name, { routes, execute } as TestRecipeDefinition<TestRecipeName>);
}

export class BrowserRecipes {
	readonly reports: TestRecipeReport[] = [];

	constructor(
		private readonly world: RecipeWorld,
		private readonly observeSteps: (
			observer: (step: { title: string; description: string }) => void,
		) => () => void,
	) {}

	async run<N extends TestRecipeName>(
		name: N,
		options: TestRecipeOptions<N>,
		route: TestRecipeRoute = "ui",
	): Promise<TestRecipeResult<N>> {
		const definition = registry.get(name);
		if (!definition) throw new Error(`Unknown test recipe "${name}"`);
		if (!definition.routes.includes(route)) {
			throw new Error(
				`Test recipe "${name}" does not support route "${route}"; supported routes: ${definition.routes.join(", ")}`,
			);
		}
		const report: TestRecipeReport = { name, route, steps: [] };
		const stop = this.observeSteps((step) => report.steps.push(step));
		try {
			const result = await definition.execute(
				this.world,
				options as TestRecipeOptions<TestRecipeName>,
			);
			this.reports.push(report);
			return result as TestRecipeResult<N>;
		} catch (reason) {
			this.reports.push(report);
			throw reason;
		} finally {
			stop();
		}
	}
}

defineTestRecipe("use-registered-desktop", ["ui"], async (t, { name }) => {
	await t.desktop.use(name);
});

defineTestRecipe("use-canonical-show", ["ui"], async (t, { show }) => {
	await t.show.use(show);
});

defineTestRecipe(
	"select-representative-fixtures",
	["api", "keypad", "ui"],
	async (t, { selection }) => {
		await t.selection.targets(...selection);
	},
);

defineTestRecipe(
	"record-two-cue-dimmer-show",
	["ui"],
	async (t, { playback, selection, low = 25, high = 100 }) => {
		await t.selection.targets(...selection);
		await t.encoder.intensity.dimmer.set(low);
		await t.record.cue({ playback, cue: 1 });
		await t.encoder.intensity.dimmer.set(high);
		await t.record.cue({ playback, cue: 2 });
	},
);

defineTestRecipe("use-representative-output-show", ["ui"], async (t) => {
	await t.show.use(Show.TwelveDimmers);
});

defineTestRecipe(
	"open-typed-pane",
	["ui"],
	async (t, { desktop, type, placement }) => {
		const configuration = t.desktop.configure(desktop);
		const handle = configuration.addPane(type, placement);
		await configuration.apply();
		return handle;
	},
);
