import { describe, expect, it, vi } from "vitest";
import type { HighlightState } from "../../../apps/light-desktop/src/api/types/desk";
import { ApiDriver } from "../core/api";
import {
	type HighlightActionPort,
	HighlightControl,
	highlightApiPort,
	highlightOscPort,
} from "./highlightScenario";

describe("Highlight acceptance intents", () => {
	it("makes explicit on and off idempotent", async () => {
		const fixture = port(false);
		const control = new HighlightControl(fixture.port);

		await expect(control.off()).resolves.toMatchObject({ active: false });
		await expect(control.on()).resolves.toMatchObject({ active: true });
		await expect(control.on()).resolves.toMatchObject({ active: true });
		await expect(control.off()).resolves.toMatchObject({ active: false });

		expect(fixture.act).toHaveBeenCalledTimes(2);
		expect(fixture.act.mock.calls.map(([action]) => action)).toEqual([
			"on",
			"off",
		]);
	});

	it("toggles across the authoritative state", async () => {
		const fixture = port(false);
		const control = new HighlightControl(fixture.port);

		await expect(control.toggle()).resolves.toMatchObject({ active: true });
		await expect(control.toggle()).resolves.toMatchObject({ active: false });

		expect(fixture.act.mock.calls.map(([action]) => action)).toEqual([
			"toggle",
			"toggle",
		]);
	});

	it("fails when the requested state never becomes authoritative", async () => {
		const fixture = port(false, false);
		await expect(new HighlightControl(fixture.port, 0).on()).rejects.toThrow(
			/Timed out waiting for Highlight on/,
		);
	});

	it("uses the authenticated typed HTTP action with explicit desk authority", async () => {
		const api = apiDriver();
		const request = vi
			.spyOn(api, "request")
			.mockResolvedValue(highlightState(false));
		const port = highlightApiPort(api);

		await port.act("on");

		expect(request).toHaveBeenCalledWith(
			"POST",
			"/api/v2/output/highlight/actions",
			{ request_id: expect.any(String), action: "on" },
			true,
			undefined,
			{ deskId: "desk-a" },
		);
	});

	it("uses the documented desk-qualified OSC Highlight route", async () => {
		let state = highlightState(false);
		const send = vi.fn(async (_address: string, arguments_: unknown[]) => {
			if (arguments_[0] === true) state = highlightState(true);
		});
		const api = apiDriver();
		vi.spyOn(api, "request").mockImplementation(async () => state);
		const port = highlightOscPort({ send }, "front-desk", api);

		await port.act("toggle");

		expect(send.mock.calls).toEqual([
			["/light/front-desk/highlight/toggle", [true]],
			["/light/front-desk/highlight/toggle", [false]],
		]);
	});

	it("resends one lost OSC toggle only while authority is unchanged", async () => {
		let state = highlightState(true);
		let presses = 0;
		const send = vi.fn(async (_address: string, arguments_: unknown[]) => {
			if (arguments_[0] !== true) return;
			presses += 1;
			if (presses === 2) state = highlightState(false);
		});
		const api = apiDriver();
		vi.spyOn(api, "request").mockImplementation(async () => state);
		const port = highlightOscPort({ send }, "front-desk", api, {
			deliveryTimeoutMillis: 0,
			debounceMillis: 0,
		});

		await expect(port.act("toggle")).resolves.toMatchObject({ active: false });
		expect(send.mock.calls).toEqual([
			["/light/front-desk/highlight/toggle", [true]],
			["/light/front-desk/highlight/toggle", [false]],
			["/light/front-desk/highlight/toggle", [true]],
			["/light/front-desk/highlight/toggle", [false]],
		]);
	});
});

function port(initial: boolean, mutate = true) {
	let state = highlightState(initial);
	const act = vi.fn<HighlightActionPort["act"]>(async (action) => {
		if (!mutate) return;
		state = highlightState(
			action === "toggle" ? !state.active : action === "on",
		);
	});
	return {
		act,
		port: {
			read: async () => state,
			act,
		} satisfies HighlightActionPort,
	};
}

function highlightState(active: boolean): HighlightState {
	return {
		active,
		mode: "selection",
		output_enabled: active,
		capture_only: false,
		remembered: [],
		active_index: null,
		active_fixture: null,
		can_previous: false,
		can_next: false,
		owner_user_id: null,
	};
}

function apiDriver(): ApiDriver {
	const api = new ApiDriver("http://127.0.0.1:5000");
	api.session = {
		session_id: "session-a",
		client_id: "client-a",
		token: "token-a",
		user: { id: "user-a", name: "Operator" },
		desk: { id: "desk-a", osc_alias: "front-desk" },
	};
	return api;
}
