import { describe, expect, it, vi } from "vitest";
import type { PlaybackRuntimeActions } from "../playbackRuntime/actionWriter";

import { HeldDemoButtons } from "./heldDemoButtons";

function deferredActions() {
	const settle: Array<() => void> = [];
	const calls: Array<{
		playbackNumber: number;
		action: string;
		input: unknown;
	}> = [];
	const actions: PlaybackRuntimeActions = {
		setActivePage: vi.fn(async () => true),
		poolPlaybackAction: vi.fn(async (playbackNumber, action, input) => {
			calls.push({ playbackNumber, action, input });
			await new Promise<void>((resolve) => settle.push(resolve));
			return null;
		}),
	};
	return { actions, calls, settle };
}

describe("HeldDemoButtons", () => {
	it("sends a physical press for the mapped Playback number and button", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(2, 17, 3);
		await flush();
		expect(calls).toEqual([
			{
				playbackNumber: 17,
				action: "button",
				input: { button: 3, pressed: true, surface: "physical" },
			},
		]);
		settle.shift()?.();
	});

	it("releases the originally pressed Playback number after a topology change", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(1, 11, 1);
		await flush();
		// The desk Page remaps slot 1 while the operator still holds the button.
		held.releaseButton(1, 1);
		settle.shift()?.();
		await flush();
		expect(calls.map((call) => [call.playbackNumber, call.input])).toEqual([
			[11, { button: 1, pressed: true, surface: "physical" }],
			[11, { button: 1, pressed: false, surface: "physical" }],
		]);
		settle.shift()?.();
	});

	it("holds the release until the press it belongs to has settled", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(1, 11, 1);
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

	it("ignores a repeated press of the same held Playback number", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(1, 11, 1);
		held.press(1, 11, 1);
		await flush();
		expect(calls).toHaveLength(1);
		settle.shift()?.();
	});

	it("releases a superseded Playback number before pressing the new one", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(1, 11, 1);
		await flush();
		held.press(1, 12, 1);
		settle.shift()?.();
		await drain(settle, calls, 3);
		expect(
			calls.map((call) => [call.playbackNumber, call.input]),
		).toEqual([
			[11, { button: 1, pressed: true, surface: "physical" }],
			[11, { button: 1, pressed: false, surface: "physical" }],
			[12, { button: 1, pressed: true, surface: "physical" }],
		]);
	});

	it("releases every held button exactly once", async () => {
		const { actions, calls, settle } = deferredActions();
		const held = new HeldDemoButtons(actions);
		held.press(1, 11, 1);
		held.press(1, 11, 2);
		held.press(21, 31, 1);
		await drain(settle, calls, 3);
		held.releaseAll();
		held.releaseAll();
		await drain(settle, calls, 6);
		expect(calls.filter((call) => (call.input as { pressed: boolean }).pressed))
			.toHaveLength(3);
		expect(
			calls
				.filter((call) => !(call.input as { pressed: boolean }).pressed)
				.map((call) => call.playbackNumber),
		).toEqual([11, 11, 31]);
	});

	it("stays inert without an authoritative action layer", async () => {
		const held = new HeldDemoButtons(null);
		expect(() => {
			held.press(1, 11, 1);
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
