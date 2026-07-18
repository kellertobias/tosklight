import type { ServerCoreContext } from "./ServerCoreContext";
import type { ServerFixtureContext } from "./ServerFixtureContext";
import type { ServerPlaybackContext } from "./ServerPlaybackContext";
import type { ServerProgrammingContext } from "./ServerProgrammingContext";
import type { ServerShowContext } from "./ServerShowContext";

export type ServerContextValue = ServerCoreContext &
	ServerProgrammingContext &
	ServerPlaybackContext &
	ServerShowContext &
	ServerFixtureContext;
