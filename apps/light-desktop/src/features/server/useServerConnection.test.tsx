import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SessionHandoff } from "../session/sessionHandoff";
import { useServerConnection } from "./useServerConnection";
import type { ServerState } from "./useServerState";

const bootstrapConnection = vi.hoisted(() => vi.fn());

vi.mock("./connectionBootstrap", () => ({ bootstrapConnection }));
vi.mock("./serverEventRouter", () => ({ createServerEventRouter: vi.fn() }));

describe("useServerConnection", () => {
	it("keeps one session owner when app-derived callbacks change", async () => {
		const session = {
			session_id: "session-a",
			client_id: "client-a",
			token: "token-a",
			desk: { id: "desk-a" },
		};
		bootstrapConnection.mockResolvedValue(session);
		const unsubscribe = vi.fn();
		const runtime = {
			currentSession: session,
			disconnectEvents: vi.fn(),
			onEvent: vi.fn(() => unsubscribe),
			connectEvents: vi.fn().mockResolvedValue(undefined),
			closeSession: vi.fn().mockResolvedValue(undefined),
		};
		const state = {
			api: { runtime },
			setError: vi.fn(),
			setStatus: vi.fn(),
		} as unknown as ServerState;
		const firstLoad = vi.fn().mockResolvedValue(undefined);
		const secondLoad = vi.fn().mockResolvedValue(undefined);
		const firstHandoff = {
			capture: vi.fn(),
			release: vi.fn(),
		} as unknown as SessionHandoff;
		const secondHandoff = {
			capture: vi.fn(),
			release: vi.fn(),
		} as unknown as SessionHandoff;
		const rendered = renderHook(
			({ load, handoff }) =>
				useServerConnection(state, load, "secondary", handoff),
			{ initialProps: { load: firstLoad, handoff: firstHandoff } },
		);

		await waitFor(() => expect(runtime.connectEvents).toHaveBeenCalledOnce());
		rendered.rerender({ load: secondLoad, handoff: secondHandoff });

		expect(bootstrapConnection).toHaveBeenCalledOnce();
		expect(runtime.disconnectEvents).toHaveBeenCalledOnce();
		expect(unsubscribe).not.toHaveBeenCalled();

		rendered.unmount();
		expect(unsubscribe).toHaveBeenCalledOnce();
		expect(runtime.disconnectEvents).toHaveBeenCalledTimes(2);
	});
});
