import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ServerState } from "../features/server/useServerState";
import { useOutputRuntimeBoundaries } from "./useOutputRuntimeBoundaries";

vi.mock("./client/serverLocation", () => ({
	configuredServerUrl: () => "http://127.0.0.1:5000",
}));
vi.mock("./PatchTransport", () => ({ browserDeskBoundaryToken: () => "" }));

const SESSION = {
	session_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
	client_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
	token: "session-token",
	user: { id: "cccccccc-cccc-4ccc-8ccc-cccccccccccc" },
	desk: { id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd" },
};

function state(session = SESSION, connectionGeneration = 1) {
	return {
		session,
		connectionGeneration,
		setError: vi.fn(),
	} as unknown as ServerState;
}

describe("useOutputRuntimeBoundaries", () => {
	it("replaces transport credentials without discarding the same data authority", () => {
		const firstState = state();
		const rendered = renderHook(
			({ current }: { current: ServerState }) =>
				useOutputRuntimeBoundaries(current),
			{ initialProps: { current: firstState } },
		);
		const first = rendered.result.current.outputRuntimeTransport;
		const firstAuthority = rendered.result.current.outputRuntimeAuthorityKey;
		expect(first).not.toBeNull();

		rendered.rerender({ current: state(SESSION, 2) });
		expect(rendered.result.current.outputRuntimeTransport).toBe(first);
		expect(rendered.result.current.outputRuntimeAuthorityKey).toBe(
			firstAuthority,
		);

		const replacement = { ...SESSION, token: "replacement-token" };
		rendered.rerender({ current: state(replacement, 2) });
		expect(rendered.result.current.outputRuntimeTransport).not.toBe(first);
		expect(rendered.result.current.outputRuntimeAuthorityKey).toBe(
			firstAuthority,
		);

		rendered.rerender({
			current: state({
				...replacement,
				desk: { id: "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee" },
			}),
		});
		expect(rendered.result.current.outputRuntimeAuthorityKey).not.toBe(
			firstAuthority,
		);
	});
});
