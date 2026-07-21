import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createServerSnapshotValue(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "status"
	| "error"
	| "dismissError"
	| "simulateError"
	| "readServerLogs"
	| "bootstrap"
	| "session"
	| "deskLock"
	| "outputRoutes"
	| "patchLayers"
	| "screens"
	| "shows"
	| "matter"
	| "fixtureLibrary"
	| "fixtureProfiles"
	| "fixtureProfileWarnings"
	| "mediaServers"
	| "mediaPreviewUrls"
	| "cueObjects"
	| "deskLayout"
	| "deskLayoutScope"
	| "unresolvedMvrFixtures"
	| "commandLine"
	| "commandTargetMode"
	| "commandLinePristine"
	| "commandHistory"
	| "pendingCommandChoice"
	| "selectedFixtures"
	| "selectedGroupId"
	| "highlight"
	| "highlightError"
	| "dismissHighlightError"
> {
	const {
		client,
		status,
		error,
		setError,
		bootstrap,
		session,
		deskLock,
		outputRoutes,
		patchLayers,
		screens,
		shows,
		matter,
		fixtureLibrary,
		fixtureProfiles,
		fixtureProfileWarnings,
		mediaServers,
		mediaPreviewUrls,
		cueObjects,
		deskLayout,
		deskLayoutScope,
		unresolvedMvrFixtures,
		commandTargetMode,
		commandLine,
		commandLinePristine,
		commandHistory,
		pendingCommandChoice,
		selectedFixtures,
		selectedGroupId,
		highlight,
		highlightError,
		setHighlightError,
		highlightErrorSticky,
	} = model;
	return {
		status,
		error,
		dismissError: () => setError(null),
		simulateError: (message) => setError(message),
		readServerLogs: () => client.auditEvents(),
		bootstrap,
		session,
		deskLock,
		outputRoutes,
		patchLayers,
		screens,
		shows,
		matter,
		fixtureLibrary,
		fixtureProfiles,
		fixtureProfileWarnings,
		mediaServers,
		mediaPreviewUrls,
		cueObjects,
		deskLayout,
		deskLayoutScope,
		unresolvedMvrFixtures,
		commandLine,
		commandTargetMode,
		commandLinePristine,
		commandHistory,
		pendingCommandChoice,
		selectedFixtures,
		selectedGroupId,
		highlight,
		highlightError,
		dismissHighlightError: () => {
			highlightErrorSticky.current = false;
			setHighlightError(null);
		},
	};
}
