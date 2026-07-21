import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createServerSnapshotValue(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "dismissError"
	| "simulateError"
	| "readServerLogs"
	| "bootstrap"
	| "session"
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
		setError,
		bootstrap,
		session,
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
		dismissError: () => setError(null),
		simulateError: (message) => setError(message),
		readServerLogs: () => client.auditEvents(),
		bootstrap,
		session,
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
