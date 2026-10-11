import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SessionHandoff } from "../session/sessionHandoff";
import { useServerConnection } from "./useServerConnection";
import type { ServerState } from "./useServerState";

const bootstrapConnection = vi.hoisted(() => vi.fn());

vi.mock("./connectionBootstrap", () => ({ bootstrapConnection }));
vi.mock("./serverEventRouter", () => ({ createServerEventRouter: vi.fn(() => vi.fn()) }));

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
			api: { runtime, playback: { screens: vi.fn().mockResolvedValue({ screens: [] }) } },
			setScreens: vi.fn(),
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

	it("repairs a Screen deleted between hydration and its event subscription", async () => {
		const session = { session_id: "desk-session", desk: { id: "desk" } };
		bootstrapConnection.mockResolvedValue(session);
		let ready!: () => void;
		const subscribed = new Promise<void>((resolve) => {
			ready = resolve;
		});
		const snapshot = { screens: [], surfaces: {}, programmer_control_surface: {} };
		const screens = vi.fn().mockResolvedValue(snapshot);
		const setScreens = vi.fn();
		const runtime = {
			currentSession: session,
			disconnectEvents: vi.fn(),
			onEvent: vi.fn(() => () => undefined),
			connectEvents: vi.fn(() => subscribed),
			closeSession: vi.fn(),
		};
		const state = {
			api: { runtime, playback: { screens } },
			setScreens,
			setError: vi.fn(),
			setStatus: vi.fn(),
		} as unknown as ServerState;
		const rendered = renderHook(() =>
			useServerConnection(state, vi.fn(), "secondary", {
				capture: vi.fn(),
				release: vi.fn(),
			}),
		);
		await waitFor(() => expect(runtime.connectEvents).toHaveBeenCalledOnce());
		expect(screens).not.toHaveBeenCalled();
		await act(async () => {
			ready();
			await subscribed;
		});
		await waitFor(() => expect(setScreens).toHaveBeenCalledWith(snapshot));
		expect(screens).toHaveBeenCalledOnce();
		expect(runtime.closeSession).not.toHaveBeenCalled();
		rendered.unmount();
	});

	it.each(["unmount", "newer-event", "superseded-attempt"] as const)(
		"does not install a stale Screen catch-up after %s",
		async (reason) => {
			const session = { session_id: "session-a", desk: { id: "desk-a" } };
			bootstrapConnection.mockResolvedValue(session);
			let resolveSnapshot!: (snapshot: { screens: [] }) => void;
			const pending = new Promise<{ screens: [] }>((resolve) => {
				resolveSnapshot = resolve;
			});
			const screens = vi.fn().mockReturnValue(pending);
			const setScreens = vi.fn();
			let eventListener!: (event: { type: "screens_changed" }) => void;
			const runtime = {
				currentSession: session,
				disconnectEvents: vi.fn(),
				closeSession: vi.fn(),
				onEvent: vi.fn((listener) => {
					eventListener = listener;
					return vi.fn();
				}),
				connectEvents: vi.fn().mockResolvedValue(undefined),
			};
			const state = {
				api: { runtime, playback: { screens } },
				setScreens,
				setError: vi.fn(),
				setStatus: vi.fn(),
			} as unknown as ServerState;
			const rendered = renderHook(() =>
				useServerConnection(state, vi.fn(), "secondary", {
					capture: vi.fn(),
					release: vi.fn(),
				}),
			);
			await waitFor(() => expect(screens).toHaveBeenCalledOnce());
			if (reason === "unmount") rendered.unmount();
			if (reason === "newer-event")
				act(() => eventListener({ type: "screens_changed" }));
			if (reason === "superseded-attempt") {
				// A manual reconnect supersedes the read; its new generation reconciles afresh.
				screens.mockResolvedValueOnce({ screens: [] });
				act(() => rendered.result.current());
				await waitFor(() => expect(setScreens).toHaveBeenCalledOnce());
				setScreens.mockClear();
			}
			await act(async () => {
				resolveSnapshot({ screens: [] });
				await pending;
			});
			expect(setScreens).not.toHaveBeenCalled();
			expect(runtime.closeSession).not.toHaveBeenCalled();
			rendered.unmount();
		},
	);

	it("explains a failed screen join and retries it on demand without closing the desk session", async () => {
		bootstrapConnection.mockReset();
		bootstrapConnection.mockRejectedValue(new TypeError("Load failed"));
		const runtime = {
			currentSession: null,
			serverUrl: "http://127.0.0.1:5471",
			disconnectEvents: vi.fn(),
			onEvent: vi.fn(() => () => undefined),
			connectEvents: vi.fn(),
			closeSession: vi.fn(),
		};
		const setError = vi.fn();
		const setStatus = vi.fn();
		const state = {
			api: { runtime },
			setError,
			setStatus,
		} as unknown as ServerState;
		const handoff = { capture: vi.fn(), release: vi.fn() };
		const rendered = renderHook(() =>
			useServerConnection(state, vi.fn(), "secondary", handoff),
		);
		await waitFor(() =>
			expect(setError).toHaveBeenCalledWith(
				expect.stringContaining("http://127.0.0.1:5471 is not reachable"),
			),
		);
		expect(setStatus).toHaveBeenCalledWith("offline");
		act(() => rendered.result.current());
		await waitFor(() => expect(bootstrapConnection).toHaveBeenCalledTimes(2));
		rendered.unmount();
		expect(runtime.closeSession).not.toHaveBeenCalled();
		act(() => rendered.result.current());
		expect(bootstrapConnection).toHaveBeenCalledTimes(2);
	});
});
