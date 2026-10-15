import { useEffect } from "react";
import type { ServerState } from "../server/useServerState";
/** One authenticated snapshot per connection; subsequent volatile values arrive on the shared event stream. */
export function useTimecodeClockBoundary(state: ServerState) {
	const { api, session, status, connectionGeneration, deskSnapshotStore } =
		state;
	const sessionId = session?.session_id;
	useEffect(() => {
		const scope = deskSnapshotStore.resetClock();
		if (!sessionId || status !== "connected") return;
		let disposed = false;
		void Promise.resolve()
			.then(() => (disposed ? null : api.runtime.timecodeClock()))
			.then((next) => {
				if (!disposed && next) deskSnapshotStore.installClock(next, scope);
			})
			.catch(() => {
				if (!disposed) deskSnapshotStore.invalidateClock(scope, true);
			});
		return () => {
			disposed = true;
		};
	}, [api, sessionId, status, connectionGeneration, deskSnapshotStore]);
}
