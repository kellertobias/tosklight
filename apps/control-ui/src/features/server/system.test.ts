import { describe, expect, it, vi } from "vitest";
import type { ServerController } from "./model";
import { createSystemActions } from "./system";

describe("Programmer lifecycle system actions", () => {
	it("clears through scoped events without reloading bootstrap", async () => {
		const client = {
			clearProgrammer: vi.fn().mockResolvedValue(undefined),
			bootstrap: vi.fn(),
		};
		const setSelectedFixtures = vi.fn();
		const setSelectedGroupId = vi.fn();
		const setCommandLineState = vi.fn();
		const setCommandLinePristine = vi.fn();
		const setError = vi.fn();
		const actions = createSystemActions({
			client,
			setError,
			bootstrap: null,
			session: { session_id: "session-a" },
			patch: null,
			playbacks: null,
			commandTargetModeRef: { current: "FIXTURE" },
			setCommandLineState,
			setCommandLinePristine,
			setSelectedFixtures,
			setSelectedGroupId,
		} as unknown as ServerController);

		await actions.clearProgrammer("session-a");

		expect(client.clearProgrammer).toHaveBeenCalledWith("session-a");
		expect(client.bootstrap).not.toHaveBeenCalled();
		expect(setSelectedFixtures).toHaveBeenCalledWith([]);
		expect(setSelectedGroupId).toHaveBeenCalledWith(null);
		expect(setCommandLineState).toHaveBeenCalledWith("FIXTURE");
		expect(setCommandLinePristine).toHaveBeenCalledWith(true);
		expect(setError).toHaveBeenCalledWith(null);
	});
});
