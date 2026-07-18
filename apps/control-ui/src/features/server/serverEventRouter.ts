import type { ServerEvent, SessionResponse } from "../../api/types";
import { routeOperatorEvent } from "./operatorEventRouting";
import { type LoadShowObjects, routeStateEvent } from "./stateEventRouting";
import type { ServerState } from "./useServerState";

export function createServerEventRouter(
	state: ServerState,
	session: SessionResponse,
	loadShowObjects: LoadShowObjects,
) {
	return (event: ServerEvent) => {
		routeOperatorEvent(event, session, state);
		routeStateEvent(event, session, state, loadShowObjects);
	};
}
