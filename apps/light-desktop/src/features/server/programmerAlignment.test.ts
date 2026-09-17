import { afterEach, describe, expect, it, vi } from "vitest";
import { DESK_NOTICE_EVENT } from "../deskNotice/deskNotice";
import type { ServerController } from "./model";
import {
	ALIGN_NO_SELECTION_NOTICE,
	createProgrammerAlignmentActions,
} from "./programmerAlignment";

function setup(align: (mode: string) => Promise<unknown>) {
	const setError = vi.fn();
	const notices: string[] = [];
	const listener = (event: Event) =>
		notices.push((event as CustomEvent<string>).detail);
	window.addEventListener(DESK_NOTICE_EVENT, listener);
	cleanups.push(() => window.removeEventListener(DESK_NOTICE_EVENT, listener));
	const model = {
		api: { programming: { align: vi.fn(align) } },
		setError,
	} as unknown as ServerController;
	return {
		actions: createProgrammerAlignmentActions(model),
		setError,
		notices,
	};
}

const cleanups: Array<() => void> = [];
afterEach(() => {
	for (const cleanup of cleanups.splice(0)) cleanup();
});

describe("Programmer Align feedback", () => {
	it("reports a no-selection activation as a quiet notice, never as a desk error", async () => {
		const { actions, setError, notices } = setup(async () => "off");

		await expect(actions.alignSelection("left")).resolves.toBe("off");

		expect(notices).toEqual([ALIGN_NO_SELECTION_NOTICE]);
		expect(setError).toHaveBeenCalledTimes(1);
		expect(setError).toHaveBeenCalledWith(null);
	});

	it("stays silent when Align actually changes", async () => {
		const { actions, notices } = setup(async () => "left");
		await expect(actions.alignSelection("left")).resolves.toBe("left");
		const off = setup(async () => "off");
		await expect(off.actions.alignSelection("off")).resolves.toBe("off");
		expect(notices).toEqual([]);
		expect(off.notices).toEqual([]);
	});

	it("keeps the actionable error treatment for a validation refusal", async () => {
		const refusal = new Error(
			"Programmer Align changed during the encoder action",
		);
		const { actions, setError, notices } = setup(async () => {
			throw refusal;
		});

		await expect(actions.alignSelection("out")).rejects.toBe(refusal);

		expect(setError).toHaveBeenCalledWith(refusal.message);
		expect(notices).toEqual([]);
	});

	it("keeps the actionable error treatment for a genuine desk failure", async () => {
		const failure = new Error("Live server connection is not ready");
		const { actions, setError, notices } = setup(async () => {
			throw failure;
		});

		await expect(actions.alignSelection("left")).rejects.toBe(failure);

		expect(setError).toHaveBeenCalledWith(failure.message);
		expect(notices).toEqual([]);
	});
});
