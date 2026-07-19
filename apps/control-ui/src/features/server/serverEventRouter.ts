import type { ServerEvent, SessionResponse } from "../../api/types";
import { routeOperatorEvent } from "./operatorEventRouting";
import {
	createStateEventRouter,
	type LoadShowObjects,
} from "./stateEventRouting";
import type { ServerState } from "./useServerState";

export function createServerEventRouter(
	getState: () => ServerState,
	session: SessionResponse,
	loadShowObjects: LoadShowObjects,
) {
	const routeStateEvent = createStateEventRouter(
		getState,
		session,
		loadShowObjects,
	);
	return (event: ServerEvent) => {
		const state = getState();
		routeOperatorEvent(event, session, state);
		routeStateEvent(event);
	};
}
