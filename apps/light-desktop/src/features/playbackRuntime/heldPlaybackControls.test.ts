import { describe, expect, it, vi } from "vitest";
import { HeldPlaybackControls } from "./heldPlaybackControls";

describe("shared playback hold ownership", () => {
	it("releases the original exact page and control after the surface changes page", async () => {
		const send = vi.fn().mockResolvedValue(undefined);
		const controls = new HeldPlaybackControls();
		const address = { page: 1, position: 401 };
		const token = controls.press(
			"pane:pointer1",
			address,
			{ surface_id: "pane:one", control_id: 1 },
			send,
		);
		address.page = 2;
		controls.release("pane:pointer1");
		await controls.settled();
		expect(send.mock.calls).toEqual([
			[
				{
					address: { page: 1, position: 401 },
					controlSource: { surface_id: "pane:one", control_id: 1 },
					holdToken: token,
				},
				true,
			],
			[
				{
					address: { page: 1, position: 401 },
					controlSource: { surface_id: "pane:one", control_id: 1 },
					holdToken: token,
				},
				false,
			],
		]);
	});
	it("preserves simultaneous surface holds and releases every hold on unmount", async () => {
		const send = vi.fn().mockResolvedValue(undefined);
		const controls = new HeldPlaybackControls();
		const first = controls.press(
			"main:1",
			{ page: 1, position: 1 },
			{ surface_id: "main", control_id: 1 },
			send,
		);
		const second = controls.press(
			"screen:1",
			{ page: 2, position: 201 },
			{ surface_id: "screen:one", control_id: 1 },
			send,
		);
		expect(second).not.toBe(first);
		controls.releaseAll();
		await controls.settled();
		expect(
			send.mock.calls.map(([hold, pressed]) => [hold.address, pressed]),
		).toEqual([
			[{ page: 1, position: 1 }, true],
			[{ page: 2, position: 201 }, true],
			[{ page: 1, position: 1 }, false],
			[{ page: 2, position: 201 }, false],
		]);
	});
	it("still sends the safety release when the press rejects", async () => {
		const send = vi
			.fn()
			.mockRejectedValueOnce(new Error("connection closed"))
			.mockResolvedValue(undefined);
		const controls = new HeldPlaybackControls();
		controls.press(
			"main:1",
			{ page: 1, position: 1 },
			{ surface_id: "main", control_id: 1 },
			send,
		);
		controls.releaseAll();
		await controls.settled();
		expect(send.mock.calls.map(([, pressed]) => pressed)).toEqual([
			true,
			false,
		]);
	});
});
