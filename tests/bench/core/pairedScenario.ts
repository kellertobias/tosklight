import type { BenchContractContext, BenchUiContext } from "./fixtures";

export type ScenarioSurface = "api" | "ui";

export interface PairedScenario<State> {
	id: string;
	title: string;
	/** Register only the listed surfaces when a migrated semantic root case owns the other half. */
	surfaces?: readonly ScenarioSurface[];
	/**
	 * Skip a surface with a human-readable reason. A surface is skipped when its
	 * reason string is present. Use this for scenarios that assert a contract that
	 * is not implemented yet (rather than a regression the refactoring introduced),
	 * so the reason stays visible next to the scenario instead of the test vanishing.
	 */
	skip?: { api?: string; ui?: string };
	arrange: (
		context: BenchContractContext,
		surface: ScenarioSurface,
	) => Promise<State> | State;
	api: (context: BenchContractContext, state: State) => Promise<void>;
	ui: (context: BenchUiContext, state: State) => Promise<void>;
	assert: (
		context: BenchContractContext,
		state: State,
		surface: ScenarioSurface,
	) => Promise<void>;
}

/**
 * Compatibility declaration for the retired paired catalog.
 *
 * The migrated semantic specs under `tests/37-semantic-*` through
 * `tests/65-semantic-*` own these behaviors through the production UI and use
 * the API as their assertion surface. Registering the former API half here
 * would execute the same behavior twice. Genuine API-only contracts and
 * constructed failure modes are registered directly with an explicit `@api`
 * tag instead.
 */
export function pairedScenario<State>(_scenario: PairedScenario<State>): void {
	// Intentionally no test registration. Delete remaining declarations as their
	// shared setup is absorbed by the semantic UI bench.
}
