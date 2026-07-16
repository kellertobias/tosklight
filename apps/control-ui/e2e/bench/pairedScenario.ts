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
    const state = await test.step("Arrange an independent working show", () => scenario.arrange(context, "api"));
    await test.step("Perform the authenticated API action", () => scenario.api(context, state));
    await test.step("Verify authoritative application and output state", () => scenario.assert(context, state, "api"));
  });
  test(`${scenario.id} @ui › ${scenario.title}`, async ({ bench, api, desk, show, page, request }) => {
    const context: BenchUiContext = { bench, api, desk, show, page, request };
    await desk.recordStep("ARRANGE", `Preparing a fresh working copy for ${scenario.id}.`);
    const state = await test.step("Arrange an independent working show", () => scenario.arrange(context, "ui"));
    await desk.recordStep("OPERATOR ACTION", scenario.title);
    await test.step("Perform the production UI action", () => scenario.ui(context, state));
    await desk.recordStep("VERIFY", "Comparing UI, server state, and rendered output with the scenario contract.");
    await test.step("Verify authoritative application and output state", () => scenario.assert(context, state, "ui"));
    await desk.recordStep("PASSED", `${scenario.id} completed successfully.`);
  });
}
