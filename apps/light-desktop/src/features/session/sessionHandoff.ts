import type { SessionResponse } from "../../api/types";

export const SESSION_HANDOFF_RECEIVER = "__lightSessionHandoffReceiver";

export type SessionHandoffPublication =
	| {
			type: "captured";
			generation: number;
			session: SessionResponse;
	  }
	| {
			type: "released";
			generation: number;
			session_id: string | null;
	  };

export interface SessionHandoff {
	capture(generation: number, session: SessionResponse): void;
	release(generation: number, sessionId: string | null): void;
}

type SessionHandoffReceiver = (
	publication: SessionHandoffPublication,
) => void | Promise<void>;

type SessionHandoffWindow = Window & {
	[SESSION_HANDOFF_RECEIVER]?: SessionHandoffReceiver;
};

export const inactiveSessionHandoff: SessionHandoff = {
	capture: () => undefined,
	release: () => undefined,
};

export function createSessionHandoff(
	runtime: SessionHandoffWindow | undefined = browserWindow(),
): SessionHandoff {
	const receiver = runtime?.[SESSION_HANDOFF_RECEIVER];
	if (typeof receiver !== "function") return inactiveSessionHandoff;
	let latestGeneration = -1;
	let sessionId: string | null = null;
	const publish = (publication: SessionHandoffPublication) => {
		void Promise.resolve(receiver(publication)).catch(() => undefined);
	};
	return {
		capture(generation, session) {
			if (generation < latestGeneration) return;
			latestGeneration = generation;
			sessionId = session.session_id;
			publish({ type: "captured", generation, session });
		},
		release(generation, expectedSessionId) {
			if (generation < latestGeneration) return;
			if (expectedSessionId && sessionId && expectedSessionId !== sessionId)
				return;
			latestGeneration = generation;
			sessionId = null;
			publish({
				type: "released",
				generation,
				session_id: expectedSessionId,
			});
		},
	};
}

function browserWindow(): SessionHandoffWindow | undefined {
	return typeof window === "undefined"
		? undefined
		: (window as SessionHandoffWindow);
}
