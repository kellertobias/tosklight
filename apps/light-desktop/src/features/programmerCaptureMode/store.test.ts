import { describe, expect, it, vi } from "vitest";
import { capturesProgrammerWrites } from "./contracts";
import { ProgrammerCaptureModeStore } from "./store";
import {
	captureModeProjection,
	captureModeSnapshot,
	OTHER_USER_ID,
	SHOW_ID,
	USER_ID,
} from "./testFixtures";

describe("ProgrammerCaptureModeStore", () => {
	it("publishes canonical immutable capture authority", () => {
		const store = new ProgrammerCaptureModeStore();
		store.reset(SHOW_ID, USER_ID);
		const projection = captureModeProjection({
			blind: true,
			preloadCaptureProgrammer: true,
		});

		expect(store.installSnapshot({ cursor: 10, projection })).toBe(true);
		const installed = store.getSnapshot().projection;
		expect(installed).not.toBe(projection);
		expect(Object.isFrozen(installed)).toBe(true);
		expect(capturesProgrammerWrites(installed)).toBe(true);
		expect(capturesProgrammerWrites({ ...projection, blind: false })).toBe(
			false,
		);
	});

	it("rejects foreign users and stale scopes without publishing", () => {
		const store = new ProgrammerCaptureModeStore();
		store.reset(SHOW_ID, USER_ID);
		const scope = store.captureScope();
		const listener = vi.fn();
		store.subscribe(listener);

		expect(
			store.installSnapshot(
				captureModeSnapshot({ userId: OTHER_USER_ID }),
				scope,
			),
		).toBe(false);
		store.reset("new-show", USER_ID);
		expect(store.installSnapshot(captureModeSnapshot(), scope)).toBe(false);
		expect(store.getSnapshot().projection).toBeNull();
		expect(listener).toHaveBeenCalledOnce();
	});

	it("suppresses duplicate events and requires repair for divergence", () => {
		const store = new ProgrammerCaptureModeStore();
		store.reset(SHOW_ID, USER_ID);
		store.installSnapshot(captureModeSnapshot());
		const listener = vi.fn();
		store.subscribe(listener);

		expect(store.applyProjection(captureModeProjection(), 10)).toBe(true);
		expect(listener).not.toHaveBeenCalled();
		expect(() =>
			store.applyProjection(captureModeProjection({ blind: true }), 10),
		).toThrow("Conflicting Programmer capture mode events");
		expect(store.getSnapshot().repairRequired).toBe(true);
	});
});
