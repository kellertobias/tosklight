import { useEffect, useRef } from "react";
import { bootstrapConnection } from "./connectionBootstrap";
import { createServerEventRouter } from "./serverEventRouter";
import type { LoadShowObjects } from "./stateEventRouting";
import type { ServerState } from "./useServerState";

export function useServerConnection(
	state: ServerState,
	loadShowObjects: LoadShowObjects,
) {
	const { client, setError, setStatus } = state;
	const stateRef = useRef(state);
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
			try {
				unsubscribe();
				client.disconnectEvents();
				const session = await bootstrapConnection(
					stateRef.current,
					loadShowObjects,
					() => cancelled,
				);
				if (!session || cancelled) return;
				unsubscribe = client.onEvent(
					createServerEventRouter(stateRef.current, session, loadShowObjects),
				);
				await client.connectEvents(retry);
				if (!cancelled) setStatus("connected");
			} catch (reason) {
				if (cancelled) return;
				setError(reason instanceof Error ? reason.message : String(reason));
				setStatus(reason instanceof TypeError ? "offline" : "error");
				retry();
			}
		};
		void start();
		return () => {
			cancelled = true;
			window.clearTimeout(retryTimer);
			unsubscribe();
			client.disconnectEvents();
			void client.closeSession();
		};
	}, [client, loadShowObjects, setError, setStatus]);
}
