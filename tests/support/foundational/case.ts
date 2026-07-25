import type { BenchUiContext } from "../../../apps/control-ui/e2e/bench/core/fixtures";

export interface FoundationalCase {
	title: string;
	run: (context: BenchUiContext) => Promise<void>;
}
