import type { SessionRole } from "../session/ownership";
import type { ServerState } from "./useServerState";

export type ServerController = ServerState & {
	sessionRole: SessionRole;
	loadShowObjects: (
		showId: string | null,
		userId: string | null,
	) => Promise<void>;
	refresh: () => Promise<void>;
	persistCommandLine: (value: string) => Promise<unknown>;
	setCommandLine: (value: string, pristine?: boolean) => void;
	resetCommandLine: () => void;
	dismissCommandChoice: () => void;
	cancelCommandChoice: () => void;
	fileRoots: ServerState["client"]["fileRoots"];
	fileEntries: ServerState["client"]["fileEntries"];
};
