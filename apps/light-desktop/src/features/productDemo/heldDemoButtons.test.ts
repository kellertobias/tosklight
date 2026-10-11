import { describe, expect, it, vi } from "vitest";
import type { PlaybackRuntimeActions } from "../playbackRuntime/actionWriter";

import { HeldDemoButtons } from "./heldDemoButtons";

function deferredActions() {
	const settle: Array<() => void> = [];
	const calls: Array<{
		address: { page: number; position: number };
		action: string;
		input: unknown;
	}> = [];
	const actions: PlaybackRuntimeActions = {
		setActivePage: vi.fn(async () => true),
		releaseCueListSource: vi.fn(async () => null),
		setGroupMaster: vi.fn(async () => null),
		setGroupFlash: vi.fn(async () => null),
		playbackAction: vi.fn(async (address, action, input) => {
			calls.push({ address, action, input });
			await new Promise<void>((resolve) => settle.push(resolve));
			return null;
		}),
	};
	return { actions, calls, settle };
}

describe("HeldDemoButtons", () => {
	it("sends a exact press for the assigned address and button", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(2, { page: 1, position: 17 }, 3);
		await flush();
		expect(calls).toEqual([
			{
				address: { page: 1, position: 17 },
				action: "button",
				input: {
					button: 3,
					pressed: true,
					control_source: { surface_id: "demo", control_id: 23 },
				},
			},
		]);
		settle.shift()?.();
	});

	it("releases the originally pressed address after a topology change", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(1, { page: 1, position: 11 }, 1);
		await flush();
		// The desk Page remaps slot 1 while the operator still holds the button.
		held.releaseButton(1, 1);
		settle.shift()?.();
		await flush();
		expect(calls.map((call) => [call.address.position, call.input])).toEqual([
			[
				11,
				{
					button: 1,
					pressed: true,
					control_source: { surface_id: "demo", control_id: 11 },
				},
			],
			[
				11,
				{
					button: 1,
					pressed: false,
					control_source: { surface_id: "demo", control_id: 11 },
				},
			],
		]);
		settle.shift()?.();
	});

	it("holds the release until the press it belongs to has settled", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(1, { page: 1, position: 11 }, 1);
		await flush();
		expect(calls).toHaveLength(1);

		held.releaseButton(1, 1);
		await flush();
		expect(calls).toHaveLength(1);

		settle.shift()?.();
		await flush();
		expect(calls).toHaveLength(2);
		expect(calls[1].input).toMatchObject({ pressed: false });
		settle.shift()?.();
	});

	it("ignores a repeated press of the same held address", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(1, { page: 1, position: 11 }, 1);
		held.press(1, { page: 1, position: 11 }, 1);
		await flush();
		expect(calls).toHaveLength(1);
		settle.shift()?.();
	});

	it("releases a superseded address before pressing the new one", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(1, { page: 1, position: 11 }, 1);
		await flush();
		held.press(1, { page: 1, position: 12 }, 1);
		settle.shift()?.();
		await drain(settle, calls, 3);
		expect(calls.map((call) => [call.address.position, call.input])).toEqual([
			[
				11,
				{
					button: 1,
					pressed: true,
					control_source: { surface_id: "demo", control_id: 11 },
				},
			],
			[
				11,
				{
					button: 1,
					pressed: false,
					control_source: { surface_id: "demo", control_id: 11 },
				},
			],
			[
				12,
				{
					button: 1,
					pressed: true,
					control_source: { surface_id: "demo", control_id: 11 },
				},
			],
		]);
	});

	it("releases every held button exactly once", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(1, { page: 1, position: 11 }, 1);
		held.press(1, { page: 1, position: 11 }, 2);
		held.press(21, { page: 1, position: 31 }, 1);
		await drain(settle, calls, 3);
		held.releaseAll();
		held.releaseAll();
		await drain(settle, calls, 6);
		expect(
			calls.filter((call) => (call.input as { pressed: boolean }).pressed),
		).toHaveLength(3);
		expect(
			calls
				.filter((call) => !(call.input as { pressed: boolean }).pressed)
				.map((call) => call.address.position),
		).toEqual([11, 11, 31]);
	});

	it("stays inert without an authoritative action layer", async () => {
		const held = new HeldDemoButtons(null);
		expect(() => {
			held.press(1, { page: 1, position: 11 }, 1);
			held.releaseAll();
			held.releaseButton(1, 1);
		}).not.toThrow();
		await flush();
	});
});

function flush() {
	return new Promise<void>((resolve) => setTimeout(resolve, 0));
}

async function drain(
	settle: Array<() => void>,
	calls: unknown[],
	expected: number,
) {
	for (let attempt = 0; attempt < 40 && calls.length < expected; attempt += 1) {
		settle.shift()?.();
		await flush();
	}
	settle.shift()?.();
}
