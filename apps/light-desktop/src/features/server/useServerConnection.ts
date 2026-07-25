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
	const { api, setError, setStatus } = state;
	const stateRef = useRef(state);
	const loadShowObjectsRef = useRef(loadShowObjects);
	const handoffRef = useRef(handoff);
	const attemptGeneration = useRef(0);
	stateRef.current = state;
	loadShowObjectsRef.current = loadShowObjects;
	handoffRef.current = handoff;
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
			handoffRef.current.release(
				generation,
				api.runtime.currentSession?.session_id ?? null,
			);
			try {
				unsubscribe();
				api.runtime.disconnectEvents();
				const session = await bootstrapConnection(
					stateRef.current,
					loadShowObjectsRef.current,
					() => cancelled || generation !== attemptGeneration.current,
					role,
				);
				if (
					!session ||
					cancelled ||
					generation !== attemptGeneration.current
				)
					return;
				handoffRef.current.capture(generation, session);
				unsubscribe = api.runtime.onEvent(
					createServerEventRouter(
						() => stateRef.current,
						session,
						loadShowObjectsRef.current,
					),
				);
				await api.runtime.connectEvents(retry);
				if (!cancelled) setStatus("connected");
			} catch (reason) {
				if (cancelled) return;
				handoffRef.current.release(
					generation,
					api.runtime.currentSession?.session_id ?? null,
				);
				setError(reason instanceof Error ? reason.message : String(reason));
				setStatus(reason instanceof TypeError ? "offline" : "error");
				retry();
			}
		};
		void start();
		return () => {
			cancelled = true;
			const generation = ++attemptGeneration.current;
			handoffRef.current.release(
				generation,
				api.runtime.currentSession?.session_id ?? null,
			);
			window.clearTimeout(retryTimer);
			unsubscribe();
			api.runtime.disconnectEvents();
			closeOwnedSession(role, () => void api.runtime.closeSession());
		};
	}, [api, role, setError, setStatus]);
}
