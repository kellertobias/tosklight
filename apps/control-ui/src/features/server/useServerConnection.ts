import { useEffect, useRef } from "react";
import { closeOwnedSession, type SessionRole } from "../session/ownership";
import type { SessionHandoff } from "../session/sessionHandoff";
import { bootstrapConnection } from "./connectionBootstrap";
import { createServerEventRouter } from "./serverEventRouter";
import type { LoadShowObjects } from "./stateEventRouting";
import type { ServerState } from "./useServerState";

export function useServerConnection(
	state: ServerState,
	loadShowObjects: LoadShowObjects,
	role: SessionRole,
	handoff: SessionHandoff,
) {
	const { client, setError, setStatus } = state;
	const stateRef = useRef(state);
	const attemptGeneration = useRef(0);
	stateRef.current = state;
	useEffect(() => {
		let cancelled = false;
		let unsubscribe = () => {};
		let retryTimer: number | undefined;
		const retry = () => {
			if (cancelled) return;
			window.clearTimeout(retryTimer);
			setStatus("connecting");
			retryTimer = window.setTimeout(() => void start(), 1_500);
		};
		const start = async () => {
			const generation = ++attemptGeneration.current;
			handoff.release(generation, client.currentSession?.session_id ?? null);
			try {
				unsubscribe();
				client.disconnectEvents();
				const session = await bootstrapConnection(
					stateRef.current,
					loadShowObjects,
					() => cancelled || generation !== attemptGeneration.current,
					role,
				);
				if (
					!session ||
					cancelled ||
					generation !== attemptGeneration.current
				)
					return;
				handoff.capture(generation, session);
				unsubscribe = client.onEvent(
					createServerEventRouter(
						() => stateRef.current,
						session,
						loadShowObjects,
					),
				);
				await client.connectEvents(retry);
				if (!cancelled) setStatus("connected");
			} catch (reason) {
				if (cancelled) return;
				handoff.release(generation, client.currentSession?.session_id ?? null);
				setError(reason instanceof Error ? reason.message : String(reason));
				setStatus(reason instanceof TypeError ? "offline" : "error");
				retry();
			}
		};
		void start();
		return () => {
			cancelled = true;
			const generation = ++attemptGeneration.current;
			handoff.release(generation, client.currentSession?.session_id ?? null);
			window.clearTimeout(retryTimer);
			unsubscribe();
			client.disconnectEvents();
			closeOwnedSession(role, () => void client.closeSession());
		};
	}, [client, handoff, loadShowObjects, role, setError, setStatus]);
}
