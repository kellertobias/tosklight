import type { BenchUiContext } from "../../bench/core/fixtures";

export interface FoundationalCase {
	title: string;
	run: (context: BenchUiContext) => Promise<void>;
}
