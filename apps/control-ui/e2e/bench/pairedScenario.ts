import { test, type BenchContractContext, type BenchUiContext } from "./fixtures";

export type ScenarioSurface = "api" | "ui";

export interface PairedScenario<State> {
  id: string;
  title: string;
  arrange: (context: BenchContractContext, surface: ScenarioSurface) => Promise<State> | State;
  api: (context: BenchContractContext, state: State) => Promise<void>;
  ui: (context: BenchUiContext, state: State) => Promise<void>;
  assert: (context: BenchContractContext, state: State, surface: ScenarioSurface) => Promise<void>;
}

/**
 * Registers an API contract test and a UI adapter test with the same stable ID,
 * fixture arrangement, and oracle. The two tests intentionally use fresh shows.
 */
export function pairedScenario<State>(scenario: PairedScenario<State>): void {
  test(`${scenario.id} @api › ${scenario.title}`, async ({ bench, api, show, request }) => {
    const context: BenchContractContext = { bench, api, show, request };
    const state = await scenario.arrange(context, "api");
    await scenario.api(context, state);
    await scenario.assert(context, state, "api");
  });
  test(`${scenario.id} @ui › ${scenario.title}`, async ({ bench, api, desk, show, page, request }) => {
    const context: BenchUiContext = { bench, api, desk, show, page, request };
    const state = await scenario.arrange(context, "ui");
    await scenario.ui(context, state);
    await scenario.assert(context, state, "ui");
  });
}
