import { afterEach, describe, expect, it, vi } from "vitest";
import type { ServerEvent, SessionResponse } from "../../api/types";
import { routeOperatorEvent } from "./operatorEventRouting";
import { routeStateEvent } from "./stateEventRouting";
import type { ServerState } from "./useServerState";

const session = {
	session_id: "session-1",
	user: { id: "user-1", name: "Operator", enabled: true },
	desk: { id: "desk-1", osc_alias: "main" },
} as SessionResponse;

function event(kind: string, payload: Record<string, unknown>): ServerEvent {
	return { revision: 1, kind, payload };
}

afterEach(() => vi.restoreAllMocks());

describe("server event routing", () => {
	it("routes desk actions only to their matching desk", () => {
		const received: string[] = [];
		window.addEventListener(
			"light:desk-action",
			((incoming: CustomEvent<string>) => {
				received.push(incoming.detail);
			}) as EventListener,
			{ once: true },
		);
		routeOperatorEvent(
			event("desk_action", { action: "clear", desk_id: "another-desk" }),
			session,
			{} as ServerState,
		);
		routeOperatorEvent(
			event("desk_action", { action: "go", desk_id: session.desk.id }),
			session,
			{} as ServerState,
		);
		expect(received).toEqual(["go"]);
	});

	it("routes Update requests through the desk-scoped UI event", () => {
		const received: unknown[] = [];
		window.addEventListener(
			"light:update-target",
			((incoming: CustomEvent) => {
				received.push(incoming.detail);
			}) as EventListener,
			{ once: true },
		);
		const target = { family: { type: "cue" }, object_id: "cue-list-1" };
		routeOperatorEvent(
			event("update_target_requested", { desk_id: session.desk.id, target }),
			session,
			{} as ServerState,
		);
		expect(received).toEqual([target]);
	});

	it("refreshes playback state for playback events", async () => {
		const playbacks = { active: [] };
		const setPlaybacks = vi.fn();
		const state = {
			client: { playbacks: vi.fn().mockResolvedValue(playbacks) },
			setPlaybacks,
		} as unknown as ServerState;
		routeStateEvent(event("playback_changed", {}), session, state, vi.fn());
		await vi.waitFor(() =>
			expect(setPlaybacks).toHaveBeenCalledWith(playbacks),
		);
	});
});
