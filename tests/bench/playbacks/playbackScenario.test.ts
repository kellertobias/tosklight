import { describe, expect, it, vi } from "vitest";
import type { Page } from "@playwright/test";
import { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { SimulatedHardware } from "../hardware/hardwareScenario";
import {
	BrowserPlaybacks,
	currentPagePlayback,
	PlaybackButton,
} from "./playbackScenario";

function rig() {
	const api = new ApiDriver("http://desk.local");
	const commandPage = vi.spyOn(api, "playbackCommandPage").mockResolvedValue(4);
	const action = vi
		.spyOn(api, "playbackAction")
		.mockResolvedValue({} as Awaited<ReturnType<ApiDriver["playbackAction"]>>);
	const playbacks = new BrowserPlaybacks(
		api,
		{} as Page,
		{} as DeskDriver,
		{} as SimulatedHardware,
		() => "show",
	);
	return { api, commandPage, action, playbacks };
}

describe("Playback bench original-address holds", () => {
	it("captures the current command page once and releases its original exact address", async () => {
		const { commandPage, action, playbacks } = rig();
		const held = playbacks.via.api.flash(currentPagePlayback(305));
		await held.press();
		commandPage.mockResolvedValue(7);
		await held.release();
		expect(commandPage).toHaveBeenCalledTimes(1);
		expect(action.mock.calls).toEqual([
			[{ page: 4, position: 305 }, "flash", { pressed: true }],
			[{ page: 4, position: 305 }, "flash", { pressed: false }],
		]);
	});

	it("releases the requested assigned Playback without substituting its direct Cuelist", async () => {
		const { action, playbacks } = rig();
		await playbacks.actionVia(
			"api",
			{ page: 4, position: 305 },
			PlaybackButton.Release,
		);
		expect(action).toHaveBeenCalledWith({ page: 4, position: 305 }, "release", {
			pressed: true,
		});
	});
});
