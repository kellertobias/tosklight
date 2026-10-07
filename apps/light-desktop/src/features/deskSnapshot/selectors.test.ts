import { describe, expect, it } from "vitest";
import type { BootstrapSnapshot } from "../../api/types";
import { runningShowId, selectActiveShowId } from "./selectors";

const show = { id: "failed-show", name: "Default Stage Show" };

function bootstrap(
	active_show_error: string | null,
): Pick<BootstrapSnapshot, "active_show" | "active_show_error"> {
	return { active_show: show as BootstrapSnapshot["active_show"], active_show_error };
}

describe("running Show", () => {
	it("is no Show while the active Show is in recovery", () => {
		const recovery = bootstrap("Show 'Default Stage Show' could not be loaded");
		// No surface loads objects or derives Group/Playback identities from a Show the engine
		// does not hold.
		expect(runningShowId(recovery)).toBeNull();
		// The recovery dialog still names the damaged Show, to keep it out of its alternatives.
		expect(
			selectActiveShowId({
				bootstrap: recovery as BootstrapSnapshot,
				session: null,
			}),
		).toBe("failed-show");
	});

	it("is the active Show once it runs", () => {
		expect(runningShowId(bootstrap(null))).toBe("failed-show");
		expect(runningShowId({ active_show: null, active_show_error: null })).toBeNull();
		expect(runningShowId(null)).toBeNull();
	});
});
