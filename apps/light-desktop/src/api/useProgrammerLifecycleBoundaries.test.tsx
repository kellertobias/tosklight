import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ServerState } from "../features/server/useServerState";
import { useProgrammerLifecycleBoundaries } from "./useProgrammerLifecycleBoundaries";

vi.mock("./client/serverLocation", () => ({
	configuredServerUrl: () => "http://127.0.0.1:5000",
}));
vi.mock("./PatchTransport", () => ({ browserDeskBoundaryToken: () => "" }));

const SESSION = {
	token: "session-token",
};

function state(connectionGeneration: number) {
	return {
		session: SESSION,
		connectionGeneration,
		setError: vi.fn(),
	} as unknown as ServerState;
}

describe("useProgrammerLifecycleBoundaries", () => {
	it("changes lifecycle authority when a replacement Headless bootstraps", () => {
		const rendered = renderHook(
			({ current }: { current: ServerState }) =>
				useProgrammerLifecycleBoundaries(current),
			{ initialProps: { current: state(1) } },
		);
		const firstAuthority =
			rendered.result.current.programmerLifecycleAuthorityKey;

		rendered.rerender({ current: state(2) });

		expect(rendered.result.current.programmerLifecycleAuthorityKey).not.toBe(
			firstAuthority,
		);
		expect(rendered.result.current.programmerLifecycleAuthorityKey).toBe(
			"http://127.0.0.1:5000|2",
		);
	});
});
